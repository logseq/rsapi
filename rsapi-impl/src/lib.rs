#![feature(async_closure)]

#[cfg(feature = "napi")]
use napi_derive::napi;
use serde::{Deserialize, Serialize};

use crate::error::Result;
pub use crate::graph::{cancel_all_requests, set_env, set_proxy, FileMeta};
use crate::graph::{Graph, GRAPHS};

pub mod error;
pub mod graph;

// re-exports
pub use lsq_encryption::keygen;

// Global progress callback
pub(crate) static mut PROGRESS_CALLBACK: Option<Box<dyn Fn(Progress)>> = None;

/// Download/Upload Progress Info
#[cfg_attr(feature = "napi", napi(object))]
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Progress {
    #[serde(rename = "graphUUID")]
    pub graph_uuid: String,
    pub file: String,
    pub r#type: &'static str,
    pub progress: i64,
    pub total: i64,
    pub percent: i32,
}

impl Progress {
    pub fn download(graph_uuid: &str, file: &str, progress: i64, total: i64) -> Self {
        Self {
            graph_uuid: graph_uuid.into(),
            file: file.into(),
            r#type: "download",
            progress,
            total,
            percent: (progress as f64 / total as f64 * 100.0) as i32,
        }
    }

    pub fn upload(graph_uuid: &str, file: &str, progress: i64, total: i64) -> Self {
        Self {
            graph_uuid: graph_uuid.into(),
            file: file.into(),
            r#type: "upload",
            progress,
            total,
            percent: (progress as f64 / total as f64 * 100.0) as i32,
        }
    }
}

pub fn set_progress_callback<F>(cb: F)
where
    F: Fn(Progress) + 'static,
{
    unsafe {
        PROGRESS_CALLBACK = Some(Box::new(cb));
    }
}

pub fn get_graph(graph_uuid: &str) -> Result<&'static Graph> {
    unsafe { GRAPHS.get_graph(graph_uuid) }
}
