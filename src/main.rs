use std::{io::ErrorKind, path::{Path, PathBuf}, process::ExitCode};
use tokio::{fs::File, io, signal};
use tokio_util::sync::CancellationToken;
use tubu::{cancellation::{CancellableResult, unless_cancelled}, config::DashLocation, download::download_set, errors::{ManifestError, ProcessingError, TubuError}, mpd::{AdaptationSet, Mpd}, muxing::{self, mux_tracks}, printer::{self, PrintTx, PrinterMessage}};

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
    if !Path::new(muxing::SO_PATH).exists() { 
        let err = std::io::Error::new(ErrorKind::NotFound, format!("{} cannot be accessed", muxing::SO_PATH));
        return Err(Some(vec![TubuError::OnSetup { err }]))
    }
    let (printer_fut, tx) = printer::create_printer();
    let print_handle = tokio::spawn(printer_fut);

    let (cnc_fut, cnc_tok) = create_ctrl_c_handler(tx.clone());
    let cnc_handle = tokio::spawn(cnc_fut);

    tokio::fs::create_dir_all("outputs").await
        .map_err(|err| vec!(TubuError::OnSetup { err }))?;
        
    let (dash_loc, mpd) = unless_cancelled(fetch_manifest(), &cnc_tok).await        
        .map_err(|oerr| oerr.map(|err| vec!(TubuError::OnReadingManifest { err })))?;
    
    let (video_path, audio_path) = process_video_audio(mpd, dash_loc, cnc_tok, tx.clone()).await?;

    // Cancellation is only accounted for if happened before/during segments download.
    // Everything after is relatively quick, and it's easier to just complete the process
    // TODO: it might change for muxing large files
    let _ = cnc_handle.abort();

    let out_path = mux_tracks(&video_path, &audio_path)
        .map_err(|err| vec!(TubuError::OnMuxing { err }))?;
    
    let msg = format!("Download successful: {}", out_path.to_string_lossy());
    let _ = tx.send(PrinterMessage::Text(msg)).await;
    drop(tx);
    
    let _ = tokio::join!(print_handle);
    Ok(())
}

fn create_ctrl_c_handler(tx: PrintTx) -> (impl Future<Output = Result<(), TubuError>>, CancellationToken) {
    let tok = CancellationToken::new();
    let tok2 = tok.clone();
    let handler = async move {
        match signal::ctrl_c().await {
            Ok(()) => {
                tok2.cancel();
                let _ = tx.send(PrinterMessage::Text("Cancelling the download...".to_string())).await;
                // the finalization message is sent from process_video_audio
                // let _ = tx.send(PrinterMessage::FinalizePB).await;
                Ok(())
            },
            Err(err) => {
                eprintln!("Unable to listen for shutdown signal: {}", err);
                Err(TubuError::OnSetup { err })
            },
        }    
    };
    (handler, tok)
}

async fn fetch_manifest() -> Result<(DashLocation, Mpd), ManifestError> {
    let dash_loc = DashLocation::new(SERVER_URL, DASH_PATH, MPD_NAME)
        .map_err(|err| ManifestError::InvalidUrl {err})?;
    let resp = reqwest::get(dash_loc.mpd_url()).await?;
    let content = resp.text().await?;
    let mpd = Mpd::parse(&content)?;
    Ok((dash_loc, mpd))
}

async fn process_video_audio(mpd: Mpd, dash_loc: DashLocation,
                             cnc_tok: CancellationToken, tx: PrintTx)
    -> CancellableResult<(PathBuf, PathBuf), Vec<TubuError>> 
{
    let total_segments = mpd.video_aset().segment_names_iterator().count() + mpd.audio_aset().segment_names_iterator().count();
    let _ = tx.send(PrinterMessage::SetupPB(total_segments)).await;

    let video_task = tokio::spawn(process_set((*mpd.video_aset()).clone(), dash_loc.clone(), tx.clone(), cnc_tok.clone()));
    let audio_task = tokio::spawn(process_set((*mpd.audio_aset()).clone(), dash_loc, tx.clone(), cnc_tok.clone()));        
    let (rv, ra) = tokio::join!(video_task, audio_task);    
    let _ = tx.send(PrinterMessage::FinalizePB).await;
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


async fn process_set(aset: AdaptationSet, dash_loc: DashLocation, tx: PrintTx, cnc_tok: CancellationToken) 
    -> CancellableResult<PathBuf, TubuError> 
{
    let dl = download_set(&aset, &dash_loc, tx.clone(), cnc_tok).await;
    match dl {        
        Ok(_) => (), // no data upon success; assume all segment files are written to known dir
        Err(None) => return Err(None),
        Err(Some(errs)) => {            
            return Err(Some(TubuError::OnLoadingSegments { aset, errs }))
        }
    };

    match concat_track(&aset).await {
        Ok(track_path) => {
            // it is a bit misleading, as these messages will be displayed above the progress bar
            let msg = format!("Processed {} segment", aset.content_type);
            let _ = tx.send(PrinterMessage::Text(msg)).await;
            Ok(track_path)
        },
        Err(err) => Err(Some(TubuError::OnProcessingSegments { aset, err }))
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