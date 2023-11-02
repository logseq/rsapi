#![deny(clippy::all)]
#![feature(result_flattening)]
#![feature(async_closure)]

use std::time::Instant;
use std::{collections::HashMap, path::PathBuf};

use napi::bindgen_prelude::*;
use napi::threadsafe_function::{
    ErrorStrategy, ThreadSafeCallContext, ThreadsafeFunction, ThreadsafeFunctionCallMode,
};
use napi::Result;
use napi_derive::napi;

use rsapi_impl as implementation;
pub use rsapi_impl::{graph::Metadata, FileMeta, Progress};

use crate::age_task::{DecryptTask, EncryptInput, EncryptTask};

pub mod age_task;

type ProgressCallbackFunction = ThreadsafeFunction<Progress, ErrorStrategy::CalleeHandled>;
static mut PROGRESS_CALLBACK: Option<ProgressCallbackFunction> = None;

static LOGGER: NodeJsLogger = NodeJsLogger;
static mut LOGGING_CALLBACK: Option<
    ThreadsafeFunction<(String, String), ErrorStrategy::CalleeHandled>,
> = None;

pub struct NodeJsLogger;

impl log::Log for NodeJsLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        (metadata.target().starts_with("rsapi") || metadata.target().starts_with("sync"))
            && metadata.level() <= log::Level::Debug
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            if let Some(callback) = unsafe { LOGGING_CALLBACK.as_ref() } {
                callback.call(
                    Ok((record.level().to_string(), format!("{}", record.args()))),
                    ThreadsafeFunctionCallMode::NonBlocking,
                );
            } else {
                eprintln!("W: logger callback not set!");
            }
        }
    }

    fn flush(&self) {}
}

/// Set rsapi Logger
#[napi]
pub fn init_logger(js_logging_fn: JsFunction) -> Result<()> {
    if unsafe { LOGGING_CALLBACK.is_some() } {
        return Ok(());
    }

    eprintln!("(rsapi) init loggers");
    let logging_fn: ThreadsafeFunction<(String, String), ErrorStrategy::CalleeHandled> =
        js_logging_fn.create_threadsafe_function(
            1000,
            |ctx: ThreadSafeCallContext<(String, String)>| {
                Ok(vec![ctx.env.to_js_value(&ctx.value)?.into_unknown()])
            },
        )?;

    unsafe {
        LOGGING_CALLBACK = Some(logging_fn);
    }
    let _ = log::set_logger(&LOGGER);
    log::set_max_level(log::LevelFilter::Debug);

    Ok(())
}

/// Age encryption key generation
#[napi]
pub async fn keygen() -> Result<HashMap<String, String>> {
    let (secret_key, pub_key) = implementation::keygen();

    let mut map = HashMap::new();
    map.insert("publicKey".into(), pub_key);
    map.insert("secretKey".into(), secret_key);
    Ok(map)
}

/// Set dev environment along with encryption key
#[napi]
pub async fn set_env(
    graph_uuid: String,
    env: String,
    secret_key: String,
    public_key: String,
) -> Result<()> {
    implementation::set_env(&graph_uuid, &env, &secret_key, &public_key)?;
    Ok(())
}

#[napi]
pub async fn set_proxy(proxy: Option<String>) -> Result<()> {
    implementation::set_proxy(proxy.as_ref().map(|x| &**x))?;
    Ok(())
}

#[napi]
pub fn set_progress_callback(callback: JsFunction) -> Result<()> {
    // ThreadsafeFunction<Progress, ErrorStrategy::CalleeHandled>
    // queue = 1000, control maximum number of notification in queue
    let progress_fn: ProgressCallbackFunction =
        callback.create_threadsafe_function(1000, |ctx: ThreadSafeCallContext<Progress>| {
            Ok(vec![ctx.env.to_js_value(&ctx.value)?.into_unknown()])
        })?;

    unsafe {
        PROGRESS_CALLBACK = Some(progress_fn);
    }

    fn progress_callback(info: Progress) {
        if let Some(callback) = unsafe { PROGRESS_CALLBACK.as_ref() } {
            callback.call(Ok(info), ThreadsafeFunctionCallMode::NonBlocking);
        } else {
            log::warn!("progress callback not set!");
        }
    }

    implementation::set_progress_callback(progress_callback);

    Ok(())
}

#[napi]
pub async fn cancel_all_requests() -> Result<()> {
    implementation::cancel_all_requests()?;
    Ok(())
}

/// get local files' metadata: file-size, md5
/// (get-local-files-meta [this graph-uuid base-path filepaths] "get local files' metadata")
#[napi]
pub async fn get_local_files_meta(
    graph_uuid: String,
    base_path: String,
    file_paths: Vec<String>,
) -> Result<HashMap<String, FileMeta>> {
    log::trace!("get local files metadata {:?}", file_paths);
    let graph = implementation::get_graph(&graph_uuid)?;
    Ok(graph.get_files_meta(base_path, file_paths).await?)
}

/// (get-local-all-files-meta [this graph-uuid base-path] "get all local files' metadata")
#[napi]
pub async fn get_local_all_files_meta(
    graph_uuid: String,
    base_path: String,
) -> Result<HashMap<String, FileMeta>> {
    let start_time = Instant::now();

    let graph = implementation::get_graph(&graph_uuid)?;
    let files_meta: HashMap<String, FileMeta> = graph
        .get_all_files_meta(&base_path)
        .await?
        .into_iter()
        .map(|meta| (meta.fname.clone(), meta))
        .collect();

    let elapsed = start_time.elapsed();
    log::info!(
        "get file meta of {:?}: {} files in {}ms",
        base_path,
        files_meta.len(),
        elapsed.as_millis()
    );

    Ok(files_meta)
}

/// (rename-local-file [this graph-uuid base-path from to access-token])
#[napi]
pub async fn rename_local_file(
    graph_uuid: String,
    base_path: String,
    from: String,
    to: String,
) -> Result<()> {
    log::info!("rename local file: {:?} => {:?}", from, to);

    let graph = implementation::get_graph(&graph_uuid)?;
    graph.rename_local_file(&base_path, &from, &to).await?;

    Ok(())
}

/// (delete-local-file [this graph-uuid base-path filepath access-token])
/// NOTE: token is not used
#[napi]
pub async fn delete_local_files(
    graph_uuid: String,
    base_path: String,
    file_paths: Vec<String>,
) -> Result<()> {
    log::info!("delete local files: {:?}", file_paths);

    let graph = implementation::get_graph(&graph_uuid)?;
    graph.delete_local_files(base_path, file_paths).await?;

    Ok(())
}

#[napi]
pub async fn fetch_remote_files(
    graph_uuid: String,
    base_path: String,
    file_paths: Vec<String>,
    token: String,
) -> Result<Vec<String>> {
    log::info!("fetch remote files: {:?}", file_paths);

    let graph = implementation::get_graph(&graph_uuid)?;
    match graph
        .fetch_remote_files(base_path, file_paths, &token)
        .await
    {
        Ok(val) => Ok(val),
        Err(e) => {
            log::error!("fetch remote files error: {:?}", e);
            return Err(e.into());
        }
    }
}

/// remote -> local
/// (update-local-file [this graph-uuid base-path filepath access-token] "remote -> local")
#[napi]
pub async fn update_local_files(
    graph_uuid: String,
    base_path: String,
    file_paths: Vec<String>,
    token: String,
) -> Result<()> {
    log::info!("update local files: {:?}", file_paths);

    let graph = implementation::get_graph(&graph_uuid)?;
    if let Err(e) = graph
        .update_local_files(base_path, file_paths, &token)
        .await
    {
        log::error!("update local files error: {:?}", e);
        return Err(e.into());
    }

    Ok(())
}

// Version files are saved in S3 with uuid as file names.
#[napi]
pub async fn update_local_version_files(
    graph_uuid: String,
    base_path: String,
    file_paths: Vec<String>,
    token: String,
) -> Result<()> {
    log::debug!("download version files: {:?}", file_paths);

    let graph = implementation::get_graph(&graph_uuid)?;
    graph
        .update_local_version_files(base_path, file_paths, &token)
        .await?;

    Ok(())
}

#[napi]
pub async fn update_remote_files(
    graph_uuid: String,
    base_path: String,
    file_paths: Vec<String>,
    txid: i64,
    token: String,
    _metadata: Option<Metadata>,
) -> Result<i64> {
    log::info!("update remote files[txid={}]: {:?}", txid, file_paths);

    let graph = implementation::get_graph(&graph_uuid)?;
    let mut retries = 0;
    loop {
        match graph
            .update_remote_files(&base_path, &file_paths, txid, &token, None)
            .await
        {
            Ok(txid) => {
                log::debug!("update remote files success, txid={}", txid);
                return Ok(txid);
            }
            Err(e) => {
                if e.to_string().contains("ExpiredToken") {
                    log::warn!("token expired, retry");
                }
                if retries >= 2 {
                    log::error!("update remote files: {}", e);
                    return Err(e.into());
                }
                log::warn!("update remote files(retry={}): {}", retries, e);
                retries += 1;
            }
        }
    }
}

/// (delete-remote-file [this graph-uuid base-path filepath local-txid access-token]))#[napi]
#[napi]
pub async fn delete_remote_files(
    graph_uuid: String,
    base_path: String,
    file_paths: Vec<String>,
    txid: i64,
    token: String,
) -> Result<i64> {
    log::info!("delete remote files[txid={}]: {:?}", txid, file_paths);

    let graph = implementation::get_graph(&graph_uuid)?;
    let txid = graph
        .delete_remote_files(base_path, file_paths, txid, &token)
        .await?;

    Ok(txid)
}

/// Encryption API

#[napi]
pub fn age_encrypt_with_passphrase(
    passphrase: String,
    data: Uint8Array,
    signal: Option<AbortSignal>,
) -> Result<AsyncTask<EncryptTask>> {
    let task = EncryptTask::new(passphrase, EncryptInput::Bytes(data.to_vec()));
    Ok(AsyncTask::with_optional_signal(task, signal))
}

#[napi]
pub fn age_decrypt_with_passphrase(
    passphrase: String,
    data: Uint8Array,
    signal: Option<AbortSignal>,
) -> Result<AsyncTask<DecryptTask>> {
    let task = DecryptTask::new(passphrase, EncryptInput::Bytes(data.to_vec()));
    Ok(AsyncTask::with_optional_signal(task, signal))
}

#[napi]
pub fn encrypt_fnames(graph_uuid: String, fnames: Vec<String>) -> Result<Vec<String>> {
    use rayon::prelude::*;

    let graph = implementation::get_graph(&graph_uuid)?;
    fnames
        .par_iter()
        .map(|p| Ok(graph.encrypt_filename(p)?))
        .collect()
}

#[napi]
pub fn decrypt_fnames(graph_uuid: String, fnames: Vec<String>) -> Result<Vec<String>> {
    use rayon::prelude::*;

    let graph = implementation::get_graph(&graph_uuid)?;
    fnames
        .par_iter()
        .map(|p| Ok(graph.decrypt_filename(p)?))
        .collect()
}

/// Helper
#[napi]
pub async fn canonicalize_path(file_path: String) -> Result<String> {
    let new_path = std::fs::canonicalize(PathBuf::from(file_path))?;
    let strip_windows_prefix = dunce::canonicalize(new_path)?;
    Ok(strip_windows_prefix.to_str().unwrap().to_string())
}
