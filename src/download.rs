use std::{io::Cursor, path::PathBuf, time::{Duration, Instant}};
use tokio_util::bytes::Bytes;
use reqwest::Client;
use tokio::{fs::File, io, task::JoinSet, time::timeout};
use tokio_util::sync::CancellationToken;
use url::Url;

use crate::{cancellation::{CancellableResult, as_cancellable, unless_cancelled}, config::DashLocation, errors::{SegmentDownloadError, reqwest_err_into_sde}, mpd::AdaptationSet};

const NUM_ATTEMPTS: usize = 2;

pub async fn download_set<T>(aset: &AdaptationSet, dash_loc: &DashLocation, cb: T, cnc_tok: CancellationToken)
    -> CancellableResult<(), Vec<SegmentDownloadError>>
    where T: FnOnce() + Clone + Send + 'static
{    
    let mut segs: Vec<String> = aset.segment_names_iterator().collect();
    let mut attempts = NUM_ATTEMPTS;
    // with the current implementation, it's easier to use "forgive timeouts flag"
    // which is only set to false on the last iteration
    // TODO: make it nicer?
    while !segs.is_empty() && attempts > 0 {        
        if attempts < NUM_ATTEMPTS { // only print it on actual retries
            println!("Trying to download {} segment again, {} attempts left", aset.content_type, attempts);
        };
        match download_set_iter(segs, dash_loc, attempts > 1, cb.clone(), cnc_tok.clone()).await {
            Ok(segs_left) => {
                segs = segs_left;
                attempts -= 1;                
            },
            Err(errs) => return Err(errs),
        }
    };

    // the loop is exited when: 
    // a) no segments are pending, or b) we exhausted retries.
    // In the latter case, due to report_timeouts value, errors should've been raised at the last iteration.
    // So at this point segs is empty.
    assert!(segs.is_empty());
    Ok(())
}

// returns either the list of timed out segments, or non-timeout errors if any
async fn download_set_iter<T>(segs: Vec<String>, dash_loc: &DashLocation, 
                              forgive_timeouts: bool, cb: T,
                              cnc_tok: CancellationToken,
                            )
    -> CancellableResult<Vec<String>, Vec<SegmentDownloadError>>
    where T: FnOnce() + Clone + Send + 'static
{
    let mut tasks = JoinSet::new();
    let client = reqwest::Client::new();
    for seg in segs {
        let url = dash_loc.segment_url(&seg);
        tasks.spawn(download_segment(url, seg, client.clone(), cb.clone(), cnc_tok.clone()));
    };

    let results = tasks.join_all().await;

    // If the cancellation happened during downloads, return immediately.
    // Otherwise, there can be no cancellations (i.e. Err(None)) among errors,
    // so it is safe to unwrap the underlying Option.
    // For the non-hierarchical cancellation token that we use,
    // this task will always observe its current status,
    // so we don't have to worry about visibility. 
    if cnc_tok.is_cancelled() {
        return Err(None)
    }
    
    // TODO: simplify types of underlying functions:
    // CancellableResult is not really needed there

    let all_errors: Vec<_> = results.into_iter()
        .filter(Result::is_err)
        .map(|e| e.unwrap_err().unwrap())
        .collect();

    if all_errors.is_empty() {
        return Ok(Vec::new())
    };

    // if the only errors are timeouts, we might retry later with timed out segments  
    if all_errors.iter().all(|(_, err)| err.is_timeout()) && forgive_timeouts {
        let segs_left = all_errors.into_iter().map(|(s, _)| s).collect();
        Ok(segs_left)
    } else {
        Err(all_errors.into_iter().map(|(_, e)| Some(e)).collect())
    }
}

async fn download_segment<T: FnOnce()>(seg_url: Url, name: String, client: Client, cb: T,
                                       cnc_tok: CancellationToken,)
    -> CancellableResult<(), (String, SegmentDownloadError)> 
{
    // Separating the actual download function to ease error handling.
    // The name of the failing segment is needed for identifying required retries,
    // as JoinSet does not preserve the order of tasks
    let _ = download_segment_impl(seg_url, &name, client, cnc_tok).await
        .map_err(|oe| oe.map(|e| (name, e)))
        ?;
    cb();
    Ok(())
}

async fn download_segment_impl(seg_url: Url, name: &String, client: Client, cnc_tok: CancellationToken)
    -> CancellableResult<(), SegmentDownloadError> 
{
    // Only cancel the web request; if it's already retrieved, save the content regardless of cancellation
    let raw = unless_cancelled(fetch_segment(seg_url, client), &cnc_tok).await?;
    as_cancellable(save_segment(raw, name)).await
}

const MAX_FETCH_DUR: Duration = Duration::from_secs(10);

async fn fetch_segment(seg_url: Url, client: Client) -> Result<Bytes, SegmentDownloadError> {
    timeout(MAX_FETCH_DUR, fetch_segment_impl(seg_url, client)).await
        .map_err(|e| SegmentDownloadError::Timeout { err: None })?
}

async fn fetch_segment_impl(seg_url: Url, client: Client) -> Result<Bytes, SegmentDownloadError> {    
    /* There are two kinds of timeouts being accounted for:
       - timeout for TCP handshake occuring upon get(..).send(),
         which is triggered at OS level and presented as reqwest's Err         
       - general timeout configured at the tokio level,
         which limits the total duration of everything 
         involved in fetching the segment's binary data.
         reqwest does not deal with this at all.
       Note that tubu ends up representing both as SegmentDownloadError::Timeout,
       because they are handled in a same way (retrying a fixed number of times). */
       
    let res = client.get(seg_url.clone()).send().await;    
    let resp = res.and_then(|r| r.error_for_status())
        .map_err(reqwest_err_into_sde)?;
    let raw = resp.bytes().await
        .map_err(reqwest_err_into_sde)?;
    Ok(raw)
}

async fn save_segment(raw: Bytes, name: &String)
-> Result<(), SegmentDownloadError> {
    let path = PathBuf::from("outputs").join(name);
    let mut file = File::create(&path).await?;
    let mut raw_cursor = Cursor::new(raw);
    let _ = io::copy(&mut raw_cursor, &mut file).await?;
    Ok(())
}
    
    