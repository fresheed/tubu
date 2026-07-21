# tubu - an async DASH Downloader

A downloader for DASH-streamed video.
This is a project for practicing async Rust w/ tokio, as well as some aspects of systems programming. 

tubu uses asychronicity to improve performance on:
- downloads of individual segments of audio/video tracks
- saving segments to separate files
- less important: concatenating segments into the audio and video tracks (only 2 tasks executing simultaneously)

Moveover, tokio's channels are used to unify the process' outputs. 

## Current status and future work

Overall, upon `cargo run`, tubu:
1. fetches the manifest file
2. downloads the audio and video tracks' segments into separate files
3. combines them into two tracks
4. calls *dlsym-intercepted* (see below) `ffmpeg` to obtain the complete video file

At the moment, a not-so-happy path is working:
- tubu implements a retry logic for timeouts, which especially occur with slow servers such as python's `http.server` (single- or multithreaded). Timeouts might happen either due to TCP handshake being ignored by the server, or because of its slow response. In both cases, we restart the download task for the timed out segments, repeating it a fixed number of times until giving up
- The download process can be gracefully cancelled with Ctrl-C. The behavior depends on when the cancellation occurs:
   - If it happens before the download starts, the process stops there
   - If a given segment is being fetched from the server, the corresponding task is cancelled.
   - If a segment is already fetched, but not saved to the file yet, cancellation is ignored (at least for this specific segment), and it is saved
   - If all segments are saved, the cancellation is ignored, and tubu produces the final file
- The errors occurring at any stage are propagated to the main function, except for timeouts that are treated differently as described above
- DASH server, as well as location of .mpd manifest file in it, is currently hardcoded
- Instead of showing the exact output of `ffmpeg` at the last step, we employ dynamic library interception to intercept the logging calls of `ffmpeg` and pass the relevant information to the tubu process. Specifically:
   1. Before spawning `ffmpeg`, tubu sets up a shared memory object and a pointer into an address in it
   2. `ffmpeg` is spawned with `LD_PRELOAD=...` that points to a shared library obtained by compiling `./intercept/av_log_intercept.c` upon `cargo build` (or separately with `cargo build-so`)
   3. This library sets up the same memory object and a pointer to the same address
   4. The library intercepts the calls to `av_log` which `ffmpeg` sends the log messages into. In particular, it finds the messages containing the number of currently processed frames and writes this number to the pointer
   5. After spawning, tubu repeatedly checks the `ffmpeg` process status, and if it is not terminated yet, it reads the current value under pointer and displays it using `indicatif`'s machinery.
- To make this progress reporting actually meaningful, the final muxing step re-encodes the video track (`libx264`, `veryslow` preset) instead of doing a fast stream copy. This is slower and lossy compared to a plain remux, but gives `ffmpeg` per-frame work to report on

Future work: since the goal of the project is practicing async Rust and systems programming, the corresponding items have higher priority, even despite e.g. proper testing obviously being useful:
- [x] complete environment setup with `docker compose`
- [x] graceful shutdown
- [ ] intercepting the internal logging calls within `ffmpeg` and passing the information into the tubu process
- [ ] final muxing without `ffmpeg`
- [ ] integration tests (at least binary match of downloaded individual segments)
- [ ] making server and manifest location the input arguments
- [ ] resumable downloads: before starting, tubu should check whether (some of) segments have already been downloaded

## Running

### Testing in an isolated environment

The project provides a complete setup with a minimal server and sample video to work with. Assuming your system has Docker Compose, checking this setup amounts to simply cloning the repo and running from the project root:
```
docker compose up -d
docker compose exec downloader /bin/bash
```
As a part of the first command above, `downloader` container simply installs `ffmpeg`. 
The second commands brings you inside the `downloader` container. There, execute
```
cargo run
```
This builds the project if needed (including the interceptor library) and runs the download. The resulting video is stored in `outputs/output.mp4`, which is mapped to the project root, so you can also see it on the host machine. 

The server is a simple Python `http.server` (multithreaded). Upon starting, it preprocesses the sample video by creating the manifest file and the segment files. It serves at `localhost:8000`; it is forwarded to the host machine, and the server includes a simple `index.html` page, so you can see the video in your browser `localhost:8000` *(you might want to turn the audio a bit down)*. 


### Manual setup 

1. Note that the interception stage relies on POSIX `shm` and `LD_PRELOAD`, so the project only runs on Linux (or another `LD_PRELOAD`-capable system)
2. Make sure your system has the prerequisites
   - Rust toolchain supporting edition 2024 (rustc ≥ 1.85)
   - `ffmpeg`
   - `gcc`
3. tubu currently expects a DASH manifest at a hardcoded address:
   `http://127.0.0.1:8000/dash/manifest.mpd`, with segment files alongside it
   under `dash/`. For local testing, place a sample DASH stream (manifest +
   segments) in a `dash/` folder and serve it, e.g.:
   ```
   python -m http.server 8000
   ```   
   
   As mentioned above, parameterizing tubu with manifest location is future work coming soon.
4. Run tubu:
   ```
   cargo run
   ```
   On success, the muxed video is written to `outputs/output.mp4`.

## Stack

- Rust for, well, almost everything
- `tokio` for organizing the async download, saving of segments and logging; plus `tokio-util` for CancellationToken
- `reqwest` for sending async GET requests
- `serde` + `quick-xml` + `xml_schema_generator` for turning a sample `.mpd` file into a Rust type for MPD
- `indicatif` for progress bars
- `ffmpeg` for final muxing
- `nix` for the Rust-side POSIX shared memory interface
- C, `dlsym` + `LD_PRELOAD` mechanism and POSIX shared memory objects on the `ffmpeg` side, for passing information from the `ffmpeg` process to tubu

## License

[MIT](LICENSE)
