# tubu - an async DASH Downloader

A downloader for DASH-streamed video.

This is a project for exploring async Rust w/ tokio. 
Specifically, asynchronicity improves performance on:
- downloads of individual segments of audio/video tracks
- saving segments to separate files
- less important: concatenating segments into the audio and video tracks (only 2 tasks executing simultaneously)

## Current status and future work

Overall, upon `cargo run`, tubu:
1. fetches the manifest file
2. downloads the audio and video tracks' segments into separate files
3. combines them into two tracks
4. calls `ffmpeg` to obtain the complete video file

At the moment, a not-so-happy path is working:
- DASH server, as well as location of .mpd manifest file in it, is hardcoded
- tubu implements a retry logic, which is needed for slow servers, such as python's `http.server` (both single- and multithreaded). It might happen both due to TCP handshake being ignored by the server, or because of its slow response. In both cases, we restart the download task with the remaining segments, repeating it a fixed number of times until giving up
- The download process can be gracefully cancelled with Ctrl-C. The behavior depends on when the cancellation occurs:
   - If it happens before the download starts, the process stops there
   - If a given segment is being fetched from the server, the corresponding task is cancelled.
   - If a segment is already fetched, but not saved to the file yet, cancellation is ignored (at least for this specific segment), and it is saved
   - If all segments are saved, the cancellation is ignored, and tubu produces the final file
- The errors occurring at any stage are propagated to the main function, except for timeouts that are treated differently as described above (might need to clean up the error types to improve it)

Future work (coming in the next few days):
- [x] complete environment setup with `docker compose`
- [ ] making server and manifest location the input arguments
- [ ] integration tests (at least binary match of downloaded individual segments)
- [x] graceful shutdown
- [ ] resumable downloads: before starting, tubu should check whether (some of) segments have already been downloaded
- [ ] final muxing without `ffmpeg`

## Running

### Testing in an isolated environment

The project provides a complete setup with a minimal server and sample video to work with. Assuming your system has Docker Compose, checking this setup amounts to simply cloning the repo and running `docker compose up` from the project root. 

The server is a simple Python `http.server` (multithreaded). Upon starting, it preprocesses the sample video by creating the manifest file and the segment files. It serves at `localhost:8000`; it is forwarded to the host machine, and the server includes a simple `index.html` page, so you can see the video in your browser `localhost:8000` (you might want to turn the audio a bit down). 

The main container installs `ffmpeg`, builds the project and immediately runs it. The resulting video is stored in `outputs/output.mp4`, which is mapped to the working directory, so you can also see it on the host machine. 

### Manual setup 

1. Make sure your system has the prerequisites
- Rust toolchain supporting edition 2024 (rustc ≥ 1.85)
- `ffmpeg` installed and available on `PATH`
2. tubu currently expects a DASH manifest at a hardcoded address:
   `http://127.0.0.1:8000/dash/manifest.mpd`, with segment files alongside it
   under `dash/`. For local testing, place a sample DASH stream (manifest +
   segments) in a `dash/` folder and serve it, e.g.:
   ```
   python -m http.server 8000
   ```   
   
   As mentioned above, parameterizing tubu with manifest location is future work coming soon.
3. Run tubu:
   ```
   cargo run
   ```
   On success, the muxed video is written to `outputs/output.mp4`.

## Stack

- Rust for, well, everything
- `tokio` for organizing the async download and saving of segments; plus `tokio-util` for CancellationToken
- `reqwest` for async GET requests
- `serde` + `quick-xml` + `xml_schema_generator` for turning a sample `.mpd` file into a Rust type for MPD
- `indicatif` for progress bar
- `ffmpeg` for final muxing

## License

[MIT](LICENSE)