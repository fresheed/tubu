use tubu::tubu::MPD::Mpd;
use url::Url;

const SERVER_URL: &str ="http://127.0.0.1:8000/";
const MPD_PATH: &str = "dash/manifest.mpd";


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let MPD_URL = Url::parse(SERVER_URL)?.join(MPD_PATH)?;
    let resp = reqwest::get(MPD_URL).await?;
    println!("MPD: {:?}", resp);
    
    // println!("{}", resp.text().await.unwrap());
    let content = resp.text().await?;
    let mpd: Mpd = Mpd::parse(&content)?;
    // println!("{:?}", mpd);

    // let one_path = get_fragment_path(&mpd);
    let init_path = get_init_fragment_path(&mpd);
    let video_aset = mpd.video_aset();
    println!("Path: {}", init_path);
    println!("Video: {:?}", video_aset);

    for seg in video_aset.segment_names_iterator() {
        println!("Video: {}", seg);
    }

    Ok(())
}

fn get_fragment_path(mpd: &Mpd, index: usize) -> &String {
    &mpd.period.adaptation_set[0].representation.segment_template.media
}

fn get_init_fragment_path(mpd: &Mpd) -> &String {
    &mpd.period.adaptation_set[0].representation.segment_template.initialization
}