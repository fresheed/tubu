use std::{fmt, io::Cursor, path::PathBuf, time::{Duration, Instant}};
use reqwest::{Client, Response};
use tokio::{fs::File, io, task::JoinSet};
use tubu::tubu::{MPD::{AdaptationSet, Mpd}, errors::{SegmentDownloadError, reqwest_err_into_sde}};
use url::Url;

const SERVER_URL: &str ="http://127.0.0.1:8000/";
const DASH_PATH: &str = "dash/";
const MPD_NAME: &str = "manifest.mpd";

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

    let video = mpd.video_aset();
    let video_res = process_set(video, &dash_loc).await;
    if video_res.is_ok() {
        println!("AdaptationSet {} ({:?}) downloaded successfully", video.id, video.content_type);
    } else {
        println!("Download of AdaptationSet {} ({:?}) failed", video.id, video.content_type);
    };
    
    Ok(())

}

async fn process_set(aset: &AdaptationSet, dash_loc: &DashLocation) 
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
