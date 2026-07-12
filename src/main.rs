use std::{fmt, io::Cursor, path::{Path, PathBuf}, process::Stdio, time::{Duration, Instant}};
use reqwest::{Client, Response};
use tokio::{fs::File, io, task::{JoinError, JoinSet}};
use tubu::tubu::{MPD::{AdaptationSet, Mpd}, errors::{ProcessingError, SegmentDownloadError, reqwest_err_into_sde}};
use url::Url;

const SERVER_URL: &str ="http://127.0.0.1:8000/";
const DASH_PATH: &str = "dash/";
const MPD_NAME: &str = "manifest.mpd";

#[derive(Clone)]
struct DashLocation {
    dash_url: Url,
    mpd_name: String,
}

impl DashLocation {    
    fn new(server_url: &str, dash_path: &str, mpd_name: &str) -> Result<Self, url::ParseError> {
        let dash_url = Url::parse(server_url)?.join(dash_path)?;
        Ok(Self { dash_url, mpd_name: mpd_name.to_string() })
    }
    
    fn mpd_url(&self) -> Url {
        // assume that no problems can occur at that point
        self.dash_url.join(&self.mpd_name).unwrap()
    }

    fn segment_url(&self, segment_path: &str) -> Url {
        // assume that no problems can occur at that point
        self.dash_url.join(segment_path).unwrap()
    }
}


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dash_loc = DashLocation::new(SERVER_URL, DASH_PATH, MPD_NAME)?;
    let resp = reqwest::get(dash_loc.mpd_url()).await?;
    println!("MPD: {:?}", resp);
    
    // println!("{}", resp.text().await.unwrap());
    let content = resp.text().await?;
    let mpd: Mpd = Mpd::parse(&content)?;
    // println!("{:?}", mpd);

    
    let video_res = tokio::spawn(process_set((*mpd.video_aset()).clone(), dash_loc.clone()));
    let audio_res = tokio::spawn(process_set((*mpd.audio_aset()).clone(), dash_loc));
    
    let (rv, ra) = tokio::join!(video_res, audio_res);
    // so far there is no cancellation implementation, and we don't expect processing to panic
    let results = (rv.unwrap(), ra.unwrap());
    let (Ok(video_path), Ok(audio_path)) = results else {
        panic!("An error occured when generating one of the tracks");
    };
    let out_path = mux_tracks(&video_path, &audio_path);
    println!("Download successful: {}", out_path.to_string_lossy());
    Ok(())

}

fn mux_tracks(video_path: &Path, audio_path: &Path) -> PathBuf {
    let out_path = PathBuf::from("outputs").join("output.mp4");
    // Could be nice to do this multiplexing by hand, 
    // but for now we simply use ffmpeg    
    let args = ["-i", &video_path.to_string_lossy(), "-i", &audio_path.to_string_lossy(),
                           "-c", "copy", // no further processing
                            "-map", "0:v:0", "-map", "1:a:0", // explicitly specify video/audio sources
                            "-y", // overwrite existing output file
                            &out_path.to_string_lossy()];
    let proc = std::process::Command::new("ffmpeg")
                       .args(&args)
                       .stdout(Stdio::null())
                       .stderr(Stdio::null()) // ffmpeg logs to stderr
                       .spawn();

    let Ok(mut proc) = proc else {
        panic!("Error running ffmpeg");
    };
  
    let output = match proc.wait() {
        Ok(output)  => output,
        Err(err)    => panic!("ffmpeg exited with error: {}", err),
    };

    out_path    
}

async fn process_set(aset: AdaptationSet, dash_loc: DashLocation) -> Result<PathBuf, ()> {
    let res = download_set(&aset, &dash_loc).await;
    // so far "processing" is just reporting the result of the download
    if res.is_ok() {
        println!("AdaptationSet {} ({:?}) downloaded successfully", aset.id, aset.content_type);
        let track_path = concat_track(&aset).await;
        let track_path = track_path.unwrap();
        println!("Track for {:?} written successfully at {}", aset.content_type, track_path.to_string_lossy());
        Ok(track_path)
    } else {
        println!("Download of AdaptationSet {} ({:?}) failed", aset.id, aset.content_type);
        Err(())
    }
}

async fn download_set(aset: &AdaptationSet, dash_loc: &DashLocation) 
    // -> Result<(), (String, SegmentDownloadError)> 
    -> Result<(), ()> 
{
    let mut tasks = JoinSet::new();
    let client = reqwest::Client::new();
    for seg in aset.segment_names_iterator() {
        let url = dash_loc.segment_url(&seg);
        tasks.spawn(download_segment(url, seg, client.clone()));
    };

    let results = tasks.join_all().await;
    let errors: Vec<_> = results.into_iter()
        .filter(Result::is_err)
        .map(Result::unwrap_err)
        .collect();

    if errors.is_empty() {
        Ok(())
    } else {
        // simplification until retries are implemented:
        // just list occured errors and return non-informative Err
        for (name, err) in errors {
            println!("An error occured for segment {}: {}", name, err);            
        };        
        Err(())
    }
}

async fn concat_track(aset: &AdaptationSet) -> Result<PathBuf, ProcessingError> {
    let name: String = format!("track_{}.mp4", aset.content_type);
    let path = PathBuf::from("outputs").join(name);
    let mut track = File::create(&path).await?;

    // Segments must be concatenated in the exact order, 
    // so we don't get much benefit from being async,
    // except that audio and video can be processed at the same time
    for seg in aset.segment_names_iterator() {
        let seg_path = PathBuf::from("outputs").join(seg);
        let mut seg_file = File::open(seg_path).await?;        
        let _ = io::copy(&mut seg_file, &mut track).await?;
    };
    Ok(path)
}

async fn download_segment(seg_url: Url, name: String, client: Client) -> Result<(), (String, SegmentDownloadError)> {
    // Separating the actual download function to ease error handling.
    // The name of the failing segment is needed for identifying required retries,
    // as JoinSet does not preserve the order of tasks
    download_segment_impl(seg_url, &name, client).await
        .map_err(|e| (name, e))
}

async fn download_segment_impl(seg_url: Url, name: &String, client: Client) -> Result<(), SegmentDownloadError> {
    let start = Instant::now();
    let res = client.get(seg_url.clone()).send().await;
    let dur = Instant::now().duration_since(start);    
    let resp = res.and_then(|r| r.error_for_status())
        .map_err(|e| reqwest_err_into_sde(e, dur.as_secs() as usize))?;
    // at that point resp is a 2.. response

    let path = PathBuf::from("outputs").join(name);
    let mut file = File::create(&path).await?;
    let raw = resp.bytes().await
        .map_err(|e| reqwest_err_into_sde(e, dur.as_secs() as usize))?;
    let mut raw_cursor = Cursor::new(raw);
    let _ = io::copy(&mut raw_cursor, &mut file).await?;
    Ok(())
}
