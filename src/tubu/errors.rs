use std::{fmt, process::{ExitStatus}};
use tokio::io;

use crate::tubu::mpd::{AdaptationSet, InvalidMpd};



#[derive(Debug)]
pub enum SegmentDownloadError {
    // considering timeout separately, because we might retry upon it
    Timeout { dur_sec: usize, err: reqwest::Error },
    RequestError { err: reqwest::Error, },
    SaveError { err: io::Error, },
}

impl fmt::Display for SegmentDownloadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SegmentDownloadError::Timeout { dur_sec, err } => 
                write!(f, "Timed out ({} seconds elapsed): {}", dur_sec, err),
            SegmentDownloadError::RequestError { err } => 
                write!(f, "Request error: {}", err),
            SegmentDownloadError::SaveError { err } => 
                write!(f, "Error upon saving the fragment to a file: {}", err),
        }
    }
}

impl std::error::Error for SegmentDownloadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SegmentDownloadError::Timeout { err , .. } => Some(err),
            SegmentDownloadError::RequestError { err } => Some(err),
            SegmentDownloadError::SaveError { err } => Some(err),
        }
    }
}

// not using Into trait, because we also want to pass duration
pub fn reqwest_err_into_sde(err: reqwest::Error, dur_sec: usize) -> SegmentDownloadError {
    if err.is_timeout() {
        SegmentDownloadError::Timeout { dur_sec, err }
    } else {
        SegmentDownloadError::RequestError { err }
    }
}

impl From<io::Error> for SegmentDownloadError {
    fn from(err: io::Error) -> Self {
        Self::SaveError { err }
    }
}

#[derive(Debug)]
pub enum ProcessingError {
    ConcatError { err: io::Error, },
}

impl From<io::Error> for ProcessingError {
    fn from(err: io::Error) -> Self {
        Self::ConcatError { err }
    }
}

#[derive(Debug)]
pub enum ManifestError {
    InvalidUrl { err: url::ParseError },
    CannotAccess { err: reqwest::Error },
    InvalidManifest { err: InvalidMpd },
}

#[derive(Debug)]
pub enum MuxingError {
    FfmpegProcError { err: io::Error },
    FfmpegFailed { code: ExitStatus },
}

#[derive(Debug)]
pub enum TubuError {
    OnReadingManifest { err: ManifestError },
    OnLoadingSegments { aset: AdaptationSet, errs: Vec<SegmentDownloadError> },
    OnProcessingSegments { aset: AdaptationSet, err: ProcessingError },
    OnMuxing { err: MuxingError },
}

impl From<MuxingError> for TubuError {
    fn from(err: MuxingError) -> Self {
        Self::OnMuxing { err }
    }
}

impl From<ManifestError> for TubuError {
    fn from(err: ManifestError) -> Self {
        Self::OnReadingManifest { err }
    }
}

impl From<reqwest::Error> for ManifestError {
    fn from(err: reqwest::Error) -> Self {
        Self::CannotAccess { err }       
    }
}

impl From<InvalidMpd> for ManifestError {
    fn from(err: InvalidMpd) -> Self {
        Self::InvalidManifest { err }
    }
}