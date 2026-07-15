use std::{io::Cursor, path::PathBuf, time::Instant};

use reqwest::Client;
use tokio::{fs::File, io, task::JoinSet};
use url::Url;

use crate::{config::DashLocation, errors::{SegmentDownloadError, reqwest_err_into_sde}, mpd::AdaptationSet};

const NUM_ATTEMPTS: usize = 2;

pub async fn download_set<T>(aset: &AdaptationSet, dash_loc: &DashLocation, cb: T)
    -> Result<(), Vec<SegmentDownloadError>>
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
        match download_set_iter(segs, dash_loc, attempts > 1, cb.clone()).await {
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
async fn download_set_iter<T>(segs: Vec<String>, dash_loc: &DashLocation, forgive_timeouts: bool,
cb: T)
    -> Result<Vec<String>, Vec<SegmentDownloadError>>
    where T: FnOnce() + Clone + Send + 'static
{
    let mut tasks = JoinSet::new();
    let client = reqwest::Client::new();
    for seg in segs {
        let url = dash_loc.segment_url(&seg);
        tasks.spawn(download_segment(url, seg, client.clone(), cb.clone()));
    };

    let results = tasks.join_all().await;
    let all_errors: Vec<_> = results.into_iter()
        .filter(Result::is_err)
        .map(|e| e.unwrap_err())
        .collect();

    if all_errors.is_empty() {
        return Ok(Vec::new())
    };

    // if the only errors are timeouts, we might retry later with timed out segments  
    if all_errors.iter().all(|(_, err)| err.is_timeout()) && forgive_timeouts {
        let segs_left = all_errors.into_iter().map(|(s, _)| s).collect();
        Ok(segs_left)
    } else {
        Err(all_errors.into_iter().map(|(_, e)| e).collect())
    }
}

async fn download_segment<T: FnOnce()>(seg_url: Url, name: String, client: Client, cb: T)
    -> Result<(), (String, SegmentDownloadError)> 
{
    // Separating the actual download function to ease error handling.
    // The name of the failing segment is needed for identifying required retries,
    // as JoinSet does not preserve the order of tasks
    let _ = download_segment_impl(seg_url, &name, client).await
        .map_err(|e| (name, e))?;
    cb();
    Ok(())
}

async fn download_segment_impl(seg_url: Url, name: &String, client: Client)
    -> Result<(), SegmentDownloadError> 
{
    let start = Instant::now();
    let res = client.get(seg_url.clone()).send().await;
    let dur = Instant::now().duration_since(start);    
    let resp = res.and_then(|r| r.error_for_status())
        .map_err(|e| reqwest_err_into_sde(e, dur.as_secs() as usize))?;
    // at that point resp is a 2.. response

    let path = PathBuf::from("outputs").join(name);
    let mut file = File::create(&path).await?;
    let raw = resp.bytes().await
        .map_err(|e| reqwest_err_into_sde(e, dur.as_secs() as usize))?;
    let mut raw_cursor = Cursor::new(raw);
    let _ = io::copy(&mut raw_cursor, &mut file).await?;
    Ok(())
}