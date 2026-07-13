use std::{io::Cursor, path::{Path, PathBuf}, process::Stdio, time::Instant};
use reqwest::Client;
use tokio::{fs::File, io, task::JoinSet};
use tubu::{mpd::{AdaptationSet, Mpd}, errors::{ManifestError, MuxingError, ProcessingError, SegmentDownloadError, TubuError, reqwest_err_into_sde}};
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


// Can produce multiple errors (e.g. failing to download audio and process video simultaneously)
#[tokio::main]
async fn main() -> Result<(), Vec<TubuError>> {
    tokio::fs::create_dir_all("outputs").await
        .map_err(|err| vec!(TubuError::OnSetup { err }))?;

    let dash_loc = DashLocation::new(SERVER_URL, DASH_PATH, MPD_NAME)
        .map_err(|err| vec!(TubuError::OnReadingManifest { err: ManifestError::InvalidUrl {err} }))?;
    let mpd = fetch_manifest(&dash_loc).await        
        .map_err(|err| vec!(TubuError::OnReadingManifest { err }))?;
    // not printing anything here - user won't wait too long to reach this point
    // println!("Manifest found");
    
    let (video_path, audio_path) = process_video_audio(mpd, dash_loc).await?;
    let out_path = mux_tracks(&video_path, &audio_path)
        .map_err(|err| vec!(TubuError::OnMuxing { err }))?;
    println!("Download successful: {}", out_path.to_string_lossy());
    Ok(())
}

async fn fetch_manifest(dash_loc: &DashLocation) -> Result<Mpd, ManifestError> {
    let resp = reqwest::get(dash_loc.mpd_url()).await?;
    let content = resp.text().await?;
    let mpd = Mpd::parse(&content)?;
    Ok(mpd)
}

async fn process_video_audio(mpd: Mpd, dash_loc: DashLocation) -> Result<(PathBuf, PathBuf), Vec<TubuError>> {
    println!("Starting download...");
    let video_task = tokio::spawn(process_set((*mpd.video_aset()).clone(), dash_loc.clone()));
    let audio_task = tokio::spawn(process_set((*mpd.audio_aset()).clone(), dash_loc));
    
    let (rv, ra) = tokio::join!(video_task, audio_task);
    // so far there is no cancellation implementation, and we don't expect processing to panic
    let results = (rv.unwrap(), ra.unwrap());

    let errors = match results {
        (Ok(path_video), Ok(path_audio)) => return Ok((path_video, path_audio)),
        (Ok(_), Err(errs)) => vec!(errs),
        (Err(errs), Ok(_)) => vec!(errs),
        (Err(errs1), Err(errs2)) => vec!(errs1, errs2),
    };
    Err(errors)
}

fn mux_tracks(video_path: &Path, audio_path: &Path) -> Result<PathBuf, MuxingError> {
    let out_path = PathBuf::from("outputs").join("output.mp4");
    // Could be nice to also implement this multiplexing, 
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

    let mut proc = proc.map_err(|err| MuxingError::FfmpegProcError { err })?;
    let out_status = match proc.wait() {
        Ok(status)  => status,
        Err(err)    => return Err(MuxingError::FfmpegProcError { err })
    };
    if out_status.success() {
        Ok(out_path)
    } else {
        Err(MuxingError::FfmpegFailed { code: out_status })
    }
}

async fn process_set(aset: AdaptationSet, dash_loc: DashLocation) -> Result<PathBuf, TubuError> {
    let dl = download_set(&aset, &dash_loc).await;
    match dl {        
        Ok(_) => (), // no data upon success; assume all segment files are written to known dir
        Err(errs) => {            
            return Err(TubuError::OnLoadingSegments { aset, errs })
        }
    };
    println!("Downloaded {} segment", aset.content_type);
    match concat_track(&aset).await {
        Ok(track_path) => {
            println!("Processed {} segment", aset.content_type);
            Ok(track_path)
        },
        Err(err) => Err(TubuError::OnProcessingSegments { aset, err })
    }
}

const NUM_ATTEMPTS: usize = 2;

async fn download_set(aset: &AdaptationSet, dash_loc: &DashLocation)
    -> Result<(), Vec<SegmentDownloadError>>
{    
    let mut segs: Vec<String> = aset.segment_names_iterator().collect();
    let mut attempts = NUM_ATTEMPTS;
    // with the current implementation, it's easier to use "forgive timeouts flag"
    // which is only set to false on the last iteration
    // TODO: make it nicer?
    while !segs.is_empty() && attempts > 0 {        
        if attempts < NUM_ATTEMPTS { // only print it on actual retries
            println!("Trying to download {} segment again, {} attempts left", aset.content_type, attempts);
        };
        match download_set_iter(segs, dash_loc, attempts > 1).await {
            Ok(segs_left) => {
                segs = segs_left;
                attempts -= 1;                
            },
            Err(errs) => return Err(errs),
        }
    };

    // the loop is exited when: 
    // a) no segments are pending, or b) we exhausted retries.
    // In the latter case, due to report_timeouts value, errors should've been raised at the last iteration.
    // So at this point segs is empty.
    assert!(segs.is_empty());
    Ok(())
}

// returns either the list of timed out segments, or non-timeout errors if any
async fn download_set_iter(segs: Vec<String>, dash_loc: &DashLocation, forgive_timeouts: bool)
    -> Result<Vec<String>, Vec<SegmentDownloadError>>
{
    let mut tasks = JoinSet::new();
    let client = reqwest::Client::new();
    for seg in segs {
        let url = dash_loc.segment_url(&seg);
        tasks.spawn(download_segment(url, seg, client.clone()));
    };

    let results = tasks.join_all().await;
    let all_errors: Vec<_> = results.into_iter()
        .filter(Result::is_err)
        .map(|e| e.unwrap_err())
        .collect();

    if all_errors.is_empty() {
        return Ok(Vec::new())
    };

    // if the only errors are timeouts, we might retry later with timed out segments  
    if all_errors.iter().all(|(_, err)| err.is_timeout()) && forgive_timeouts {
        let segs_left = all_errors.into_iter().map(|(s, _)| s).collect();
        Ok(segs_left)
    } else {
        Err(all_errors.into_iter().map(|(_, e)| e).collect())
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
