use std::{path::{Path, PathBuf}, process::{ExitCode, Stdio}};
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use tokio::{fs::File, io, signal};
use tokio_util::sync::CancellationToken;
use tubu::{cancellation::{CancellableResult, unless_cancelled}, config::DashLocation, download::download_set, errors::{ManifestError, MuxingError, ProcessingError, TubuError}, mpd::{AdaptationSet, Mpd}};

const SERVER_URL: &str ="http://127.0.0.1:8000/";
const DASH_PATH: &str = "dash/";
const MPD_NAME: &str = "manifest.mpd";


#[tokio::main]
async fn main() -> ExitCode {
    match tubu_main().await {
        Ok(_) => ExitCode::SUCCESS,
        Err(None) => {
            println!("Download cancelled");
            ExitCode::FAILURE
        },
        Err(Some(errs)) => {
            // TODO: improve error printing
            println!("Errors occured: {:?}", errs);
            ExitCode::FAILURE
        }        
    }
}

// Can produce multiple errors (e.g. failing to download audio and process video simultaneously)
async fn tubu_main() -> CancellableResult<(), Vec<TubuError>> {
    let cnc_tok = CancellationToken::new();

    let cnc_tok2 = cnc_tok.clone();
    let cnc_handle = tokio::spawn(async move {        
        match signal::ctrl_c().await {
            Ok(()) => {
                cnc_tok2.cancel();
                println!("Cancelling the download...");
                Ok(())
            },
            Err(err) => {
                eprintln!("Unable to listen for shutdown signal: {}", err);
                Err(TubuError::OnSetup { err })
            },
        }
    });

    tokio::fs::create_dir_all("outputs").await
        .map_err(|err| vec!(TubuError::OnSetup { err }))?;
        
    let dash_loc = DashLocation::new(SERVER_URL, DASH_PATH, MPD_NAME)
        .map_err(|err| vec!(TubuError::OnReadingManifest { err: ManifestError::InvalidUrl {err} }))?;
    let mpd = unless_cancelled(fetch_manifest(&dash_loc), &cnc_tok).await        
        .map_err(|oerr| oerr.map(|err| vec!(TubuError::OnReadingManifest { err })))?;
    
    let (video_path, audio_path) = process_video_audio(mpd, dash_loc, cnc_tok).await?;

    // Cancellation is only accounted for if happened before/during segments download.
    // Everything after is relatively quick, and it's easier to just complete the process
    // TODO: it might change for muxing large files
    let _ = cnc_handle.abort();

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

async fn process_video_audio(mpd: Mpd, dash_loc: DashLocation, cnc_tok: CancellationToken)
    -> CancellableResult<(PathBuf, PathBuf), Vec<TubuError>> 
{
    // Just use progress bar instead
    // println!("Starting download...");
    let pb = setup_progressbar(&mpd);

    let video_task = tokio::spawn(process_set((*mpd.video_aset()).clone(), dash_loc.clone(), pb.clone(), cnc_tok.clone()));
    let audio_task = tokio::spawn(process_set((*mpd.audio_aset()).clone(), dash_loc, pb.clone(), cnc_tok.clone()));        
    let (rv, ra) = tokio::join!(video_task, audio_task);    
    pb.finish();
    // cancellation is graceful via token, and we don't expect processing to panic
    let results = (rv.unwrap(), ra.unwrap());

    let errors = match results {
        (Ok(path_video), Ok(path_audio)) => return Ok((path_video, path_audio)),
        (Err(None), _) => return Err(None),
        (_, Err(None)) => return Err(None),
        (Ok(_), Err(Some(errs))) => vec!(errs),
        (Err(Some(errs)), Ok(_)) => vec!(errs),
        (Err(Some(errs1)), Err(Some(errs2))) => vec!(errs1, errs2),
    };
    Err(Some(errors))
}

fn setup_progressbar(mpd: &Mpd) -> ProgressBar {
    let total_segments = mpd.video_aset().segment_names_iterator().count() + mpd.audio_aset().segment_names_iterator().count();
    let pb = ProgressBar::new(total_segments as u64);
    pb.set_style(ProgressStyle::with_template("{msg}:{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len}")
        .unwrap()
        .progress_chars("#>-"));
    pb.set_draw_target(ProgressDrawTarget::stdout());
    pb.set_message("Download progress");
    pb
}

async fn process_set(aset: AdaptationSet, dash_loc: DashLocation, pb: ProgressBar, cnc_tok: CancellationToken) 
    -> CancellableResult<PathBuf, TubuError> 
{

    let pbc = pb.clone();
    let dl = download_set(&aset, &dash_loc,
        move || { pbc.inc(1); }, cnc_tok).await;
    match dl {        
        Ok(_) => (), // no data upon success; assume all segment files are written to known dir
        Err(None) => return Err(None),
        Err(Some(errs)) => {            
            return Err(Some(TubuError::OnLoadingSegments { aset, errs }))
        }
    };
    // Now this is replaced by the unified progress bar
    // println!("Downloaded {} segment", aset.content_type);
    match concat_track(&aset).await {
        Ok(track_path) => {
            // it is a bit misleading, as these messages will be displayed above the progress bar
            let msg = format!("Processed {} segment", aset.content_type);
            pb.println(msg);
            Ok(track_path)
        },
        Err(err) => Err(Some(TubuError::OnProcessingSegments { aset, err }))
    }
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