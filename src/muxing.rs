use std::{num::{NonZero, NonZeroUsize}, path::{Path, PathBuf}, process::{Command, Stdio}, sync::atomic::{AtomicU32, Ordering}, thread, time::Duration};
use indicatif::{ProgressBar, ProgressStyle};
use nix::{errno::Errno, fcntl::OFlag, sys::{mman::{MapFlags, ProtFlags, mmap, shm_open, shm_unlink}, stat::Mode}, unistd::{close, ftruncate}};

use crate::errors::MuxingError;

// Could be nice to also implement this multiplexing, but for now we simply use ffmpeg.
// It also gives an opportunity to play with dlsym and shared memory mechanism.

const TUBU_SHMEM_ID: &str = "/tubu_shared";
const SHARED_SIZE: NonZero<usize> = NonZeroUsize::new(size_of::<u32>()).unwrap();

pub fn mux_tracks(video_path: &Path, audio_path: &Path) -> Result<PathBuf, MuxingError> {
    let out_path: PathBuf = PathBuf::from("outputs").join("output.mp4");
    // see the comments in av_log_intercept.c about tearing shm down
    let raw = setup_shmem()?;
    let frame_amount: &AtomicU32 = unsafe { AtomicU32::from_ptr(raw as *mut u32) };
    frame_amount.store(0, Ordering::Relaxed);

    let cmd = get_ffmpeg_cmd(video_path, audio_path, &out_path);
    run_and_track(frame_amount, cmd)?;

    let _ = teardown_shmem() // simply report an error, don't propagate it
        .unwrap_or_else(|err| eprintln!("Error on tearing down shared memory: {}", err));
    Ok(out_path)
}

fn run_and_track(frame_amount: &AtomicU32, mut cmd: Command) -> Result<(), MuxingError> {
    let pb = ProgressBar::new_spinner();
    pb.set_style(ProgressStyle::with_template("Frames processed: {msg}").unwrap());
    let mut proc = cmd.spawn()
        .map_err(|err| MuxingError::FfmpegProcError { err })?;
    loop {
        match proc.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                let n = frame_amount.load(Ordering::Relaxed);
                pb.set_message(n.to_string());
                // at this point there are no ongoing tasks, so we simply block
                thread::sleep(Duration::from_millis(100));
            },
            Err(err) => return Err(MuxingError::FfmpegProcError { err }),
        }
    }
    // only way to break the loop is to read Ok(Some(_)), so unwrapping is safe
    let out_status = proc.wait().unwrap();
    if out_status.success() {
        Ok(())
    } else {
        Err(MuxingError::FfmpegFailed { code: out_status })
    }
}

fn setup_shmem() -> Result<*mut u32, MuxingError> {
    let shm_flags = OFlag::O_CREAT | OFlag::O_EXCL | OFlag::O_RDWR;
    let shm_mode = Mode::S_IRUSR | Mode::S_IWUSR;
    let fd = shm_open(TUBU_SHMEM_ID, shm_flags, shm_mode)?;
    ftruncate(&fd, SHARED_SIZE.get() as i64)?;
    let mmap_prot = ProtFlags::PROT_READ | ProtFlags::PROT_WRITE;
    let ptr_raw = unsafe { 
        mmap(None, SHARED_SIZE, mmap_prot, 
            MapFlags::MAP_SHARED, &fd,0)
    }?;
    close(fd)?;
    Ok(ptr_raw.as_ptr() as *mut u32)
}

fn teardown_shmem() -> Result<(), Errno> {
    shm_unlink(TUBU_SHMEM_ID)
}

fn get_ffmpeg_cmd(video_path: &Path, audio_path: &Path, out_path: &Path) -> Command { 
    // let args = ["-i", &video_path.to_string_lossy(), "-i", &audio_path.to_string_lossy(),
    //                         "-c", "copy", // no further processing
    //                         "-map", "0:v:0", "-map", "1:a:0", // explicitly specify video/audio sources
    //                         "-y", // overwrite existing output file
    //                         &out_path.to_string_lossy()];

    // slow it down to see that the progress bar actually gets updated
    let args = ["-i", &video_path.to_string_lossy(), "-i", &audio_path.to_string_lossy(),
                        "-c:v", "libx264", "-preset", "veryslow", // force real per-frame encoding
                        "-c:a", "copy", // audio stays fast, no need to slow it down too
                        "-map", "0:v:0", "-map", "1:a:0", // explicitly specify video/audio sources
                        "-y", // overwrite existing output file
                        &out_path.to_string_lossy()];
                            
    let mut cmd = std::process::Command::new("ffmpeg");
    cmd.args(&args)
       .stdout(Stdio::null())
       .stderr(Stdio::null()) // ffmpeg logs to stderr
       .env("LD_PRELOAD", "./intercept/av_log_intercept.so");
    cmd
}
