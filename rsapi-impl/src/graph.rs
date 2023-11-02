use std::{
    borrow::Cow,
    collections::HashMap,
    path::Path,
    sync::Arc,
    time::{Duration, SystemTime},
};

#[cfg(feature = "napi")]
use napi_derive::napi;
use once_cell::sync::Lazy;
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::watch;

use futures::prelude::*;
use lsq_encryption::md5_hexdigest;
use sync::SyncClient;
use unicode_normalization::UnicodeNormalization;

use crate::error::{Error, Result};
use crate::{Progress, PROGRESS_CALLBACK};

pub static mut CANCELLACTION_TX: Option<watch::Sender<()>> = None;
pub static mut CANCELLACTION_RX: Option<watch::Receiver<()>> = None;

pub static mut GRAPHS: Lazy<GraphCatalog> = Lazy::new(|| {
    let (cancel_tx, cancel_rx) = watch::channel(());

    unsafe {
        CANCELLACTION_TX = Some(cancel_tx);
        CANCELLACTION_RX = Some(cancel_rx);
    }
    GraphCatalog::default()
});

// Public API implementation

#[derive(Default)]
pub struct GraphCatalog(HashMap<String, Graph>);

impl GraphCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_graph(&mut self, graph: Graph) {
        self.0.insert(graph.uuid.clone(), graph);
    }

    pub fn get_graph(&self, graph_uuid: &str) -> Result<&Graph> {
        self.0.get(graph_uuid).ok_or(Error::GraphNotSet)
    }
}

#[derive(Debug)]
pub struct Graph {
    pub uuid: String,
    // TODO: base_bash should be bonded to the graph
    // pub base_path: PathBuf,
    pub age_public_key: String,
    pub age_secret_key: String,
    pub fname_encryption_key: [u8; 32],
}

pub fn cancel_all_requests() -> Result<()> {
    unsafe { CANCELLACTION_TX.as_ref() }.map(|tx| tx.send(()));
    log::debug!("cancelling all request");
    Ok(())
}

// Set proxy for all sync requests, convert error to impl::Error
pub fn set_proxy(proxy: Option<&str>) -> Result<()> {
    if proxy.is_some() {
        log::info!("setting proxy: {:?}", proxy);
    }
    sync::set_proxy(proxy)?;
    Ok(())
}

pub fn set_env(graph_uuid: &str, env: &str, secret_key: &str, public_key: &str) -> Result<()> {
    log::info!("set sync env {:?} for {}", env, graph_uuid);
    // clean temp credential cache
    sync::reset_user();
    // also cancel any pending requests
    unsafe { CANCELLACTION_TX.as_ref() }.map(|tx| tx.send(()));

    match env {
        "production" | "product" | "prod" => {
            sync::set_prod();
        }
        "development" | "develop" | "dev" => {
            sync::set_dev();
        }
        _ => return Err(Error::InvalidArg),
    }

    let g = Graph {
        uuid: graph_uuid.into(),
        age_public_key: public_key.into(),
        age_secret_key: secret_key.into(),
        fname_encryption_key: lsq_encryption::to_raw_x25519_key(secret_key)?,
    };

    unsafe {
        GRAPHS.add_graph(g);
    }

    Ok(())
}

impl Graph {
    pub fn encrypt_filename(&self, fname: &str) -> Result<String> {
        Ok(lsq_encryption::encrypt_filename(
            &fname,
            &self.fname_encryption_key,
        )?)
    }

    pub fn decrypt_filename(&self, fname: &str) -> Result<String> {
        Ok(lsq_encryption::decrypt_filename(
            &fname,
            &self.fname_encryption_key,
        )?)
    }

    pub fn encrypt_content<'a, 'b>(&'a self, data: &'b [u8]) -> Result<Cow<'b, [u8]>> {
        if data.starts_with(b"-----BEGIN AGE ENCRYPTED FILE-----")
            || data.starts_with(b"age-encryption.org/v1\n")
        {
            return Ok(data.into());
        }

        let encrypted = lsq_encryption::encrypt_with_x25519(&self.age_public_key, data, false)?;
        Ok(encrypted.to_vec().into())
    }

    pub fn decrypt_content<'a, 'b>(&'a self, data: &'b [u8]) -> Result<Cow<'b, [u8]>> {
        if data.starts_with(b"-----BEGIN AGE ENCRYPTED FILE-----")
            || data.starts_with(b"age-encryption.org/v1\n")
        {
            let decrypted = lsq_encryption::decrypt_with_x25519(&self.age_secret_key, data)?;
            return Ok(decrypted.to_vec().into());
        }

        Ok(data.into())
    }

    pub async fn get_files_meta<P0: AsRef<Path>, P1: AsRef<str>, PS>(
        &self,
        base_path: P0,
        file_paths: PS,
    ) -> Result<HashMap<String, FileMeta>>
    where
        PS: IntoIterator<Item = P1>,
    {
        let base_path = dunce::canonicalize(base_path.as_ref())?;

        let futs = file_paths.into_iter().map(|p| {
            let path: String = p.as_ref().to_string();

            let meta = {
                let p = path.clone();
                self.get_file_meta(&base_path, p)
            };
            meta.map(move |meta| (path, meta))
        });

        Ok(future::join_all(futs)
            .await
            .into_iter()
            .filter(|(_, m)| m.is_ok())
            .map(|(p, m)| (p, m.unwrap()))
            .collect())
    }

    pub async fn get_all_files_meta<P: AsRef<Path>>(
        &self,
        base_path: P,
    ) -> Result<impl Iterator<Item = FileMeta>> {
        let base_path = dunce::canonicalize(base_path.as_ref())?;
        let base_path_ref = base_path.clone();

        let futs = {
            walkdir::WalkDir::new(&base_path)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter_map(move |e| {
                    e.path()
                        .strip_prefix(&base_path)
                        .ok()
                        .and_then(|p| p.to_str())
                        .map(|p| p.replace("\\", "/").trim_start_matches('/').to_string())
                })
                .filter(|p| {
                    !(p.starts_with('.')
                        || p.contains("/.")
                        || p.starts_with("logseq/bak/")
                        || p.starts_with("logseq/version-files/"))
                })
                .map(|p| self.get_file_meta(&base_path_ref, p))
        };
        Ok(future::join_all(futs)
            .await
            .into_iter()
            .filter(Result::is_ok)
            .map(Result::unwrap))
    }

    pub async fn rename_local_file<P: AsRef<Path>, S0: AsRef<str>, S1: AsRef<str>>(
        &self,
        base_path: P,
        from: S0,
        to: S1,
    ) -> Result<()> {
        let base_path = base_path.as_ref();
        fs::rename(base_path.join(from.as_ref()), base_path.join(to.as_ref())).await?;
        Ok(())
    }

    // delete local file
    pub async fn delete_local_files<P: AsRef<Path>, S: AsRef<str>>(
        &self,
        base_path: P,
        file_paths: impl IntoIterator<Item = S>,
    ) -> Result<()> {
        let base_path = base_path.as_ref();
        let futs = file_paths
            .into_iter()
            .map(async move |p| fs::remove_file(base_path.join(p.as_ref())).await);
        future::join_all(futs)
            .await
            .into_iter()
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Error::from)
            .map(|_| ())
    }

    // Delete remote file, and local base version, return txid
    pub async fn delete_remote_files<P: AsRef<Path>, S: AsRef<str>>(
        &self,
        base_path: P,
        file_paths: impl IntoIterator<Item = S>,
        txid: i64,
        token: &str,
    ) -> Result<i64> {
        let base_path = base_path.as_ref();
        let mut client = SyncClient::new(&token);
        client.set_graph(&self.uuid, txid);

        let file_paths = file_paths
            .into_iter()
            .map(|s| s.as_ref().to_string())
            .collect::<Vec<_>>();

        let encrypted_file_paths = file_paths
            .iter()
            .map(|p| self.encrypt_filename(&p))
            .collect::<Result<Vec<_>>>()?;

        let ret = client.delete_files(encrypted_file_paths).await?;

        for file_rpath in &file_paths {
            let _ =
                fs::remove_file(base_path.join("logseq/version-files/base").join(file_rpath)).await;
        }

        Ok(ret.txid)
    }

    /// Logseq Sync v2: Fetch remote files to local version DB.
    /// To replace `update_local_files`.
    ///
    /// Return list of local downloaded files, ready to be merged.
    pub async fn fetch_remote_files<P: AsRef<Path>, S: AsRef<str>>(
        &self,
        base_path: P,
        file_paths: impl IntoIterator<Item = S>,
        token: &str,
    ) -> Result<Vec<String>> {
        let mut cancel_notification = unsafe { CANCELLACTION_RX.as_ref().unwrap().clone() };
        let _ = cancel_notification.borrow_and_update();

        let base_path = base_path.as_ref();
        let mut client = SyncClient::new(&token);
        client.set_graph(&self.uuid, 0);
        let client = Arc::new(client);

        // Vec<(encrypted_file_path, file_path)>
        let encrypted_paths = file_paths
            .into_iter()
            .map(|p| {
                self.encrypt_filename(p.as_ref())
                    .map(|ep| (ep, p.as_ref().to_string()))
            })
            .collect::<Result<HashMap<_, _>>>()?;

        // encrypted_file_path => remote_url
        let remote_files = client.get_files(encrypted_paths.keys()).await?;
        log::debug!("get {} remote files", remote_files.len());

        let mut tasks = vec![];
        for (encrypted_file_path, remote_url) in remote_files {
            let file_path = match encrypted_paths.get(&encrypted_file_path) {
                Some(p) => p.to_owned(),
                None => continue,
            };

            let client = client.clone();
            let graph_uuid = self.uuid.clone();

            let target_file_path = if is_page_file(&file_path) {
                base_path
                    .join("logseq/version-files/incoming")
                    .join(&file_path)
            } else {
                base_path.join(&file_path)
            };

            // avoid use of moved value
            let file_path1 = file_path.clone();

            let progress_callback = move |bytes, total| {
                static mut LAST: usize = 0;
                let progress = bytes * 100 / total;
                unsafe {
                    // reduce callback calling frequency
                    if LAST / 10 != progress / 10 || bytes == total {
                        LAST = progress;
                        if let Some(callback) = &PROGRESS_CALLBACK {
                            let progress =
                                Progress::download(&graph_uuid, &file_path, bytes as _, total as _);
                            callback(progress);
                        }
                        log::debug!(
                            "download progress: {}% {}/{} {:?}",
                            progress,
                            bytes,
                            total,
                            file_path
                        );
                    }
                }
            };

            tasks.push(async move {
                let buf = client.download_file(&remote_url, progress_callback).await?;
                let decrypted = self.decrypt_content(&buf)?;

                if let Some(dir) = target_file_path.parent() {
                    fs::create_dir_all(dir).await?;
                }
                let mut file = fs::File::create(&target_file_path).await?;
                file.write_all(&decrypted).await?;
                file.flush().await?;
                log::debug!("write to file: {:?}", target_file_path);

                if is_page_file_path(&file_path1) {
                    Ok::<_, Error>(Some(file_path1.clone()))
                } else {
                    Ok::<_, Error>(None)
                }
            });
        }

        tokio::select! {
            ret = future::join_all(tasks) => {
                ret.into_iter().filter_map(|f| f.transpose()).collect()
            }
            _ = cancel_notification.changed() => {
                log::warn!("downloading remote cancelled");
                Err(Error::Cancelled)
            }
        }
    }

    /// Download files from remote, and update local files.
    pub async fn update_local_files<P: AsRef<Path>, S: AsRef<str>>(
        &self,
        base_path: P,
        file_paths: impl IntoIterator<Item = S>,
        token: &str,
    ) -> Result<()> {
        let mut cancel_notification = unsafe { CANCELLACTION_RX.as_ref().unwrap().clone() };
        let _ = cancel_notification.borrow_and_update();

        let mut client = SyncClient::new(&token);
        client.set_graph(&self.uuid, 0);
        let client = Arc::new(client);

        let base_path = base_path.as_ref();
        // Vec<(encrypted_file_path, file_path)>
        let encrypted_paths = file_paths
            .into_iter()
            .map(|p| {
                self.encrypt_filename(p.as_ref())
                    .map(|ep| (ep, p.as_ref().to_string()))
            })
            .collect::<Result<HashMap<_, _>>>()?;

        // encrypted_file_path => remote_url
        let remote_files = client.get_files(encrypted_paths.keys()).await?;
        log::debug!("get {} remote files", remote_files.len());

        let mut tasks = vec![];
        for (encrypted_file_path, remote_url) in remote_files {
            let file_path = match encrypted_paths.get(&encrypted_file_path) {
                Some(p) => p.to_owned(),
                None => continue,
            };

            let absolute_file_path = base_path.join(&file_path);
            let client = client.clone();
            let graph_uuid = self.uuid.clone();

            let progress_callback = move |bytes, total| {
                static mut LAST: usize = 0;
                let progress = bytes * 100 / total;
                unsafe {
                    // reduce callback calling frequency
                    if LAST / 10 != progress / 10 || bytes == total {
                        LAST = progress;
                        if let Some(callback) = &PROGRESS_CALLBACK {
                            let progress =
                                Progress::download(&graph_uuid, &file_path, bytes as _, total as _);
                            callback(progress);
                        }
                        log::debug!(
                            "download progress: {}% {}/{} {:?}",
                            progress,
                            bytes,
                            total,
                            file_path
                        );
                    }
                }
            };
            tasks.push(async move {
                let buf = client.download_file(&remote_url, progress_callback).await?;
                let decrypted = self.decrypt_content(&buf)?;

                if let Some(dir) = absolute_file_path.parent() {
                    fs::create_dir_all(dir).await?;
                }
                let mut file = fs::File::create(absolute_file_path).await?;
                file.write_all(&decrypted).await?;
                file.flush().await?;
                Ok::<_, Error>(())
            });
        }

        tokio::select! {
            ret = future::join_all(tasks) => {
                ret.into_iter().collect::<Result<Vec<_>>>().map(|_| ())
            }
            _ = cancel_notification.changed() => {
                log::warn!("downloading remote cancelled");
                Err(Error::Cancelled)
            }
        }
    }

    /// Logseq Sync v2, update remote files and save to local version-db.
    pub async fn update_remote_files<P: AsRef<Path>, S: AsRef<str>>(
        &self,
        base_path: P,
        file_paths: impl IntoIterator<Item = S>,
        txid: i64,
        token: &str,
        _metadata: Option<Metadata>, // TODO
    ) -> Result<i64> {
        let mut cancel_notification = unsafe { CANCELLACTION_RX.as_ref().unwrap().clone() };
        let _ = cancel_notification.borrow_and_update();

        let mut client = SyncClient::new(&token);
        client.set_graph(&self.uuid, txid);
        client.refresh_temp_credential().await?;

        let base_path = base_path.as_ref();
        let client = Arc::new(client);

        // ensure folder and write permission
        fs::create_dir_all(base_path.join("logseq/version-files/base")).await?;

        let mut tasks = vec![];
        let mut page_files = vec![];
        for file_path in file_paths {
            // move in variables
            let client = client.clone();
            let file_path = file_path.as_ref().to_string();
            if is_page_file_path(&file_path) {
                page_files.push(file_path.clone());
            }

            let full_file_path = base_path.join(&file_path);

            let progress_callback = {
                let file_path = file_path.clone();
                let graph_uuid = self.uuid.clone();

                move |bytes, total| {
                    static mut LAST: usize = 0;
                    let progress = bytes * 100 / total;
                    unsafe {
                        // reduce notifications
                        if LAST / 10 != progress / 10 || bytes == total {
                            if let Some(callback) = &PROGRESS_CALLBACK {
                                let progress = Progress::upload(
                                    &graph_uuid,
                                    &file_path,
                                    bytes as _,
                                    total as _,
                                );
                                callback(progress);
                            }
                            LAST = progress;
                            log::debug!(
                                "upload progress: {}% {}/{} {}",
                                progress,
                                bytes,
                                total,
                                file_path
                            );
                        }
                    }
                }
            };
            tasks.push(async move {
                let content = fs::read(full_file_path).await?;
                // stage 1.1: md5 metadata
                let md5checksum = md5_hexdigest(&content);
                // stage 1.2: encryption
                let encrypted = self.encrypt_content(&content)?;
                if encrypted.len() > 10 * 1024 * 1024 {
                    log::warn!(
                        "large file {:?} size: {:.2}MiB encrypted: {:.2}MiB",
                        file_path,
                        content.len() as f64 / (1024.0 * 1024.0),
                        encrypted.len() as f64 / (1024.0 * 1024.0)
                    );
                }
                let remote_temp_url = client.upload_tempfile(encrypted, progress_callback).await?;
                let encrypted_file_path = self.encrypt_filename(&file_path)?;
                Result::Ok((encrypted_file_path, remote_temp_url, md5checksum))
            });
        }

        tokio::select! {
            task_results = future::join_all(tasks) => {
                let temp_remote_files = task_results.into_iter().collect::<Result<Vec<_>>>()?;
                let update = client.update_files(temp_remote_files).await?;
                for path in page_files {
                    let target_path = base_path.join("logseq/version-files/base").join(&path);
                    if let Some(dir) = target_path.parent() {
                        fs::create_dir_all(dir).await?;
                    }
                    fs::copy(base_path.join(&path),target_path ).await?;
                    log::debug!("copy page file to version-files: {:?}", path);
                }
                Ok(update.txid)
            }
            _ = cancel_notification.changed() => {
                log::warn!("update remote file cancelled");
                Err(Error::Cancelled)
            }
        }
    }

    pub async fn update_local_version_files<P: AsRef<Path>, S: AsRef<str>>(
        &self,
        base_path: P,
        file_ids: impl IntoIterator<Item = S>,
        token: &str,
    ) -> Result<()> {
        let base_path = base_path.as_ref();
        let mut client = SyncClient::new(&token);
        client.set_graph(&self.uuid, 0);

        let files = client.get_version_files(file_ids).await?;
        for (file_id, file_url) in files {
            let full_file_path = base_path.join("logseq/version-files").join(&file_id);

            let buf = client.download_file(&file_url, |_, _| {}).await?;
            let decrypted = self.decrypt_content(&buf)?;

            if let Some(file_dir) = full_file_path.parent() {
                fs::create_dir_all(file_dir).await?;
            }
            let mut file = fs::File::create(full_file_path).await?;
            file.write_all(&decrypted).await?;
            file.flush().await?;
        }
        Ok(())
    }

    async fn get_file_meta<P: AsRef<Path>, S: AsRef<str>>(
        &self,
        base_path: P,
        file_path: S,
    ) -> Result<FileMeta> {
        use md5::{Digest, Md5};
        let base_path = dunce::canonicalize(base_path.as_ref())?;
        let full_file_path = dunce::canonicalize(base_path.join(file_path.as_ref()))?;

        let canonicalized_file_path = full_file_path
            .strip_prefix(&base_path)
            .ok()
            .and_then(|p| p.to_str())
            .map(|p| p.replace("\\", "/").trim_start_matches('/').to_string())
            .ok_or(Error::InvalidArg)?;
        let normalized_file_path = canonicalized_file_path.nfc().collect::<String>();

        let mut file = fs::File::open(&full_file_path).await?;

        let mut nread = 0;
        let mut buf = Vec::with_capacity(1024 * 1024);
        let mut hasher = Md5::new();
        loop {
            let n = file.read_buf(&mut buf).await?;
            if n == 0 {
                break;
            }
            nread += n;
            hasher.update(&buf[..n]);
            unsafe {
                buf.set_len(0);
            }
        }
        let digest = hasher.finalize();

        let metadata = file.metadata().await?;
        Ok(FileMeta {
            size: nread as _,
            mtime: metadata
                .modified()
                .ok()
                .and_then(|m| m.duration_since(SystemTime::UNIX_EPOCH).ok())
                .unwrap_or(Duration::default())
                .as_millis() as _,
            ctime: metadata
                .created()
                .ok()
                .and_then(|m| m.duration_since(SystemTime::UNIX_EPOCH).ok())
                .unwrap_or(Duration::default())
                .as_millis() as _,
            md5: format!("{:x}", digest),
            fname: canonicalized_file_path.to_owned(),
            incoming_fname: file_path.as_ref().to_string(),
            normalized_fname: normalized_file_path,
            encrypted_fname: self.encrypt_filename(&canonicalized_file_path)?,
        })
    }
}

unsafe impl Send for Graph {}
unsafe impl Sync for Graph {}

#[cfg_attr(feature = "napi", napi(object))]
#[derive(Debug)]
pub struct FileMeta {
    pub size: i64,
    /// modified time, in milliseconds
    pub mtime: i64,
    /// creation time, in milliseconds
    pub ctime: i64,
    pub md5: String,
    // legacy field
    pub fname: String,
    pub incoming_fname: String,
    // NFC normalized file name
    pub normalized_fname: String,
    // encrypted file name
    pub encrypted_fname: String,
}

/// Metadata for batch remote update
#[cfg_attr(feature = "napi", napi(object))]
#[derive(Debug)]
pub struct Metadata {
    pub fs_case_sensitive: bool,
    pub version: String,
    pub revision: String,
    pub platform: String,
}

fn is_page_file(file_path: &str) -> bool {
    let t = file_path.to_lowercase();
    t.ends_with(".md") || t.ends_with(".org") || t.ends_with(".markdown")
}

fn is_page_file_path<P: AsRef<Path>>(file_path: P) -> bool {
    let t = file_path
        .as_ref()
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .to_lowercase();
    t.ends_with(".md") || t.ends_with(".org") || t.ends_with(".markdown")
}
