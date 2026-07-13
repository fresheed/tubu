# tubu - an async DASH Downloader

A downloader for DASH-streamed video.
This is a project for exploring async Rust w/ tokio. 
Specifically, we use asynchronicity to improve performance on:
- downloads of individual segments of audio/video tracks. Here, we also implement retry logic: if some track's segments are timed out, we restart the download task with the remaining segments, repeating it a fixed number of times until giving up
- saving segments to separate files
- less important: concatenating segments into the audio and video tracks (only 2 tasks executing simultaneously)

## Current status and future work

At the moment, a not-so-happy path is working:
- DASH server, as well as location of .mpd manifest file in it, is hardcoded
- With `cargo run`, tubu fetches the manifest file, downloads the audio and video tracks' segments, combines them into two tracks, and finally calls `ffmpeg` to obtain the complete video file
- Each of these steps might fail, which is accounted for with a custom error type. The implementation is not supposed to panic. 
- tubu implements a retry logic, which is needed for slow servers, such as python's http.server (both single- and multithreaded)

Future work (coming in the next few days):
- complete environment setup with `docker compose`
- making server and manifest location the input arguments
- integration tests (at least binary match of downloaded individual segments)
- graceful shutdown
- resumable downloads: before starting, tubu should check whether (some of) segments have already been downloaded
- final muxing without `ffmpeg`

## Running

**Prerequisites:**
- Rust toolchain supporting edition 2024 (rustc ≥ 1.85)
- `ffmpeg` installed and available on `PATH`
- A DASH source to download from — see below

**Setup:**

1. Create the output directory (not created automatically):
   ```
   mkdir outputs
   ```
2. tubu currently expects a DASH manifest at a hardcoded address:
   `http://127.0.0.1:8000/dash/manifest.mpd`, with segment files alongside it
   under `dash/`. For local testing, place a sample DASH stream (manifest +
   segments) in a `dash/` folder and serve it, e.g.:
   ```
   python -m http.server 8000
   ```   
   
   As mentioned above, completing the testing setup and parameterizing tubu with manifest location is future work coming soon.
3. Run tubu:
   ```
   cargo run
   ```
   On success, the muxed video is written to `outputs/output.mp4`.

## Stack

- Rust for, well, everything
- `tokio` for async download and saving of segments
- `reqwest` for sending async GET requests
- `serde` + `quick-xml` + `xml_schema_generator` for turning a sample `.mpd` file into a Rust type for MPD
- `ffmpeg` for final muxing

## License

MIT — see [LICENSE](LICENSE).