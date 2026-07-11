use std::{io::Cursor, path::PathBuf, time::{Duration, Instant}};
use reqwest::{Client, Response};
use tokio::{fs::File, io, task::JoinSet};
use tubu::tubu::MPD::{AdaptationSet, Mpd};
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

    let tasks = process_set(mpd.video_aset(), &dash_loc);
    let results = tasks.join_all().await;

    for res in results {
        ()
    };

    Ok(())
}

fn process_set(aset: &AdaptationSet, dash_loc: &DashLocation) -> JoinSet<String> {
    // let video_aset = mpd.video_aset();
    // for seg in video_aset.segment_names_iterator() {
    //     println!("Video: {}", seg);
    // }

    let mut tasks = JoinSet::new();
    let client = reqwest::Client::new();
    for seg in aset.segment_names_iterator() {
        let url = dash_loc.segment_url(&seg);
        tasks.spawn(download_segment(url, seg, client.clone()));
    }
    tasks
}

async fn download_segment(seg_url: Url, name: String, client: Client) -> String {
    let start = Instant::now();
    let res = client.get(seg_url.clone()).send().await;
    let end = Instant::now();
    let dur = end.duration_since(start);
    let dl_time = format!("Downloaded {} in {} sec", seg_url.to_string(), dur.as_secs().to_string());
    let out = match res {
        Ok(resp) => if resp.status() != 200 {            
            format!("not 200: {}", resp.text().await.unwrap())
        } else {
            let path = PathBuf::from("outputs").join(name);
            let Ok(mut file) = File::create(&path).await else {
                return format!("error creating file {}", path.display());
            };
            let Ok(raw) = resp.bytes().await else {
                return format!("could not get raw bytes for {}", path.display());
            };
            let mut raw_cursor = Cursor::new(raw);
            io::copy(&mut raw_cursor, &mut file).await;
            format!("200")            
        }
        Err(err) => format!("An error: {:?}", err),
    };
    let msg = format!("{} : {}", dl_time, out);
    println!("{}", msg);
    msg    
}
