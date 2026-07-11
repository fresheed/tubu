use reqwest::Response;
use tokio::task::JoinSet;
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

    let tasks = process_set(mpd.video_aset(), &dash_loc)    ;
    let results = tasks.join_all().await;

    for res in results {
        match res {
            Ok(_resp) => (),
            Err(_err) => println!("An error occured somewhere"),
        }
    };

    Ok(())
}

fn process_set(aset: &AdaptationSet, dash_loc: &DashLocation) -> JoinSet<Result<Response, reqwest::Error>> {
    // let video_aset = mpd.video_aset();
    // for seg in video_aset.segment_names_iterator() {
    //     println!("Video: {}", seg);
    // }

    let mut tasks = JoinSet::new();
    for seg in aset.segment_names_iterator() {
        let url = dash_loc.segment_url(&seg);
        tasks.spawn(download_segment(url));
    }
    tasks
}

async fn download_segment(seg_url: Url) -> Result<Response, reqwest::Error> {
    let res = reqwest::get(seg_url.clone()).await;
    println!("Downloaded {}", seg_url.to_string());
    res
}
