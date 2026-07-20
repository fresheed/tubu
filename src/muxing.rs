use std::{ffi::c_void, num::{NonZero, NonZeroUsize}, path::{Path, PathBuf}, process::Stdio};
use nix::{fcntl::OFlag, sys::{mman::{MapFlags, ProtFlags, mmap, munmap, shm_open, shm_unlink}, stat::Mode}, unistd::ftruncate};

use crate::errors::MuxingError;

const TUBU_SHMEM_ID: &str = "/tubu_shared";
const SHARED_SIZE: NonZero<usize> = NonZeroUsize::new(size_of::<u32>()).unwrap();

fn mux_extra() -> Result<(), MuxingError> {
    let shm_flags = OFlag::O_CREAT | OFlag::O_EXCL | OFlag::O_RDWR;
    let shm_mode = Mode::S_IRUSR | Mode::S_IWUSR;
    let fd = shm_open(TUBU_SHMEM_ID, shm_flags, shm_mode)?;
    ftruncate(&fd, SHARED_SIZE.get() as i64)?;

    let mmap_prot = ProtFlags::PROT_READ | ProtFlags::PROT_WRITE;
    let ptr_raw = unsafe { 
        mmap(None, SHARED_SIZE, mmap_prot, 
            MapFlags::MAP_SHARED, &fd,0)
    }?;

    let frames_amount: &mut u32 = unsafe { &mut *(ptr_raw.as_ptr() as *mut u32) };

    Ok(())
}

pub fn mux_tracks(video_path: &Path, audio_path: &Path) -> Result<PathBuf, MuxingError> {
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
