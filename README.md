# tubu - an async DASH Downloader

A downloader for DASH-streamed video.
This is a project for exploring async Rust w/ tokio. 

## Current status and future work

At the moment, a not-so-happy path is working:
- DASH server, as well as location of .mpd manifest file in it, is hardcoded
- With `cargo run`, tubu fetches the manifest file, downloads the audio and video tracks' segments, combines them into two tracks, and finally calls `ffmpeg` to obtain the complete video file
- Each of these steps might fail, which is accounted for with a custom error type. The implementation is not supposed to panic. 
- tubu implements a retry logic, which is needed for slow servers, such as python's http.server (both single- and multithreaded). If the download of some segments has timed out, and there are no other errors, tubu retries the download with these missing segments a fixed number of times before finally reporting error

Future work (coming in the next few days):
- complete environment setup with `docker compose`
- making server and manifest location the input arguments
- integration tests (at least binary match of downloaded individual segments)
- graceful shutdown
- resumable downloads: before starting, tubu should check whether (some of) segments have already been downloaded
- final muxing without `ffmpeg`

## Stack

- Rust for, well, everything
- `tokio` for async download and saving of segments. Also for concatenating them into tracks, although it doesn't improve performance that much
- `reqwest` for sending async GET requests
- `serde` + `quick-xml` + `xml_schema_generator` for turning a sample `.mpd` file into a Rust type for MPD
- `ffmpeg` for final muxing

## License

MIT — see [LICENSE](LICENSE).