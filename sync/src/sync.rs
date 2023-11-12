use std::borrow::Cow;
use std::collections::HashMap;
// use std::sync::Arc;
use std::time::Duration;

use serde_json::json;

use crate::error::SyncError;
use crate::helpers::ProgressedBytesStream;
use crate::types::{self, Credentials, TempCredential};
use crate::Result;

static mut TEMP_CREDENTIAL: Option<TempCredential> = None;

static mut HTTPS_PROXY: Option<String> = None;

// API URL base
static mut URL_BASE: &str = "https://api-dev.logseq.com/file-sync/";
static mut BUCKET: &str = "logseq-file-sync-bucket";
static mut REGION: &str = "us-east-2";

const URL_BASE_DEV: &str = "https://api-dev.logseq.com/file-sync/";
const BUCKET_DEV: &str = "logseq-file-sync-bucket";
const REGION_DEV: &str = "us-east-2";

const URL_BASE_PROD: &str = "https://api.logseq.com/file-sync/";
const BUCKET_PROD: &str = "logseq-file-sync-bucket-prod";
const REGION_PROD: &str = "us-east-1";

fn url_base() -> &'static str {
    unsafe { URL_BASE }
}

fn bucket() -> &'static str {
    unsafe { BUCKET }
}

fn region() -> &'static str {
    unsafe { REGION }
}

/// set environment to prod
pub fn set_prod() {
    unsafe {
        URL_BASE = URL_BASE_PROD;
        BUCKET = BUCKET_PROD;
        REGION = REGION_PROD;
    }
}

pub fn set_dev() {
    unsafe {
        URL_BASE = URL_BASE_DEV;
        BUCKET = BUCKET_DEV;
        REGION = REGION_DEV;
    }
}

pub fn set_proxy(proxy: Option<&str>) -> Result<()> {
    unsafe {
        if let Some(proxy) = proxy {
            // Check proxy
            let _ = reqwest::Proxy::https(proxy)?;
            HTTPS_PROXY = Some(proxy.to_string());
        } else {
            HTTPS_PROXY = None;
        }
    }
    Ok(())
}

pub fn reset_user() {
    unsafe {
        TEMP_CREDENTIAL = None;
    }
}

pub struct SyncClient {
    client: reqwest::Client,
    txid: i64,
    graph_uuid: String,
    credentials: Option<Credentials>,
    s3_prefix: Option<String>,
    auth_token: String,
}
unsafe impl Sync for SyncClient {}
unsafe impl Send for SyncClient {}

impl SyncClient {
    pub fn new(token: &str) -> SyncClient {
        let accept_invalid_certs = true;
        // env::var("NODE_TLS_REJECT_UNAUTHORIZED").unwrap_or_default() == "0";
        let client = {
            let mut builder = reqwest::Client::builder()
                .user_agent("Logseq-sync/0.3")
                .connection_verbose(false)
                // .dns_resolver(Arc::new(crate::doh::DoHResolver))
                .timeout(Duration::from_secs(30))
                .connect_timeout(Duration::from_secs(15))
                .http2_keep_alive_interval(Duration::from_secs(10))
                .http2_keep_alive_timeout(Duration::from_secs(60))
                .http2_keep_alive_while_idle(true);
            if let Some(proxy) = unsafe { HTTPS_PROXY.as_ref() } {
                builder = builder.proxy(reqwest::Proxy::https(proxy).unwrap());
            }
            if accept_invalid_certs {
                // log::info!("NODE_TLS_REJECT_UNAUTHORIZED=0, won't validate certs");
                builder = builder.danger_accept_invalid_certs(true);
            }
            builder.build().unwrap()
        };

        SyncClient {
            client,
            txid: -1, // uninited
            credentials: None,
            graph_uuid: String::new(),
            s3_prefix: None,
            auth_token: token.to_string(),
        }
    }

    /// for stateless access
    pub fn set_graph(&mut self, uuid: &str, txid: i64) {
        self.graph_uuid = uuid.to_string();
        self.txid = txid;
    }

    // ==========
    // API helpers
    // ==========

    // update temp credentials if needed
    pub async fn refresh_temp_credential(&mut self) -> Result<()> {
        unsafe {
            if self.credentials.is_none() && TEMP_CREDENTIAL.is_some() {
                let temp_credential = TEMP_CREDENTIAL.clone().unwrap();
                self.credentials = Some(temp_credential.credentials);
                self.s3_prefix = Some(temp_credential.s3_prefix);
            }
        }
        if self
            .credentials
            .as_ref()
            .map(|c| c.is_expired())
            .unwrap_or(true)
        {
            let temp_credential = self.get_temp_credential().await?;

            unsafe {
                TEMP_CREDENTIAL = Some(temp_credential.clone());
            }

            log::debug!(
                "credential refreshed, next expiration at {}",
                temp_credential.credentials.expiration
            );
            self.credentials = Some(temp_credential.credentials);
            self.s3_prefix = Some(temp_credential.s3_prefix);
        }
        Ok(())
    }

    // ==========
    // APIs
    // ==========

    // create a new graph
    // NOTE: actually the return type is SimpleGraph,
    // but the default values is reasonable, so we use Graph instead
    pub async fn create_graph(&self, name: &str) -> Result<types::Graph> {
        let payload = json!({ "GraphName": name });
        let resp = self
            .client
            .post(url_base().to_owned() + "create_graph")
            .body(payload.to_string())
            .bearer_auth(&self.auth_token)
            .header("Content-Type", "application/octet-stream")
            .send()
            .await?;
        let graph: types::Graph = resp.json().await?;
        match graph.message {
            None => Ok(graph),
            Some(text) => SyncError::from_message(text),
        }
    }

    // get the graph
    // FIXME: get a non-existing graph will return an empty string!
    pub async fn get_graph(&self, name: &str) -> Result<types::Graph> {
        let payload = json!({ "GraphName": name });
        let resp = self
            .client
            .post(url_base().to_owned() + "get_graph")
            .body(payload.to_string())
            .bearer_auth(&self.auth_token)
            .header("Content-Type", "application/octet-stream")
            .send()
            .await?
            .error_for_status()?;

        let graph: types::Graph = resp.json().await?;
        match graph.message {
            None => Ok(graph),
            Some(text) => SyncError::from_message(text),
        }
    }

    pub async fn get_graph_by_uuid(&self, uuid: &str) -> Result<types::Graph> {
        let payload = json!({ "GraphUUID": uuid });
        let resp = self
            .client
            .post(url_base().to_owned() + "get_graph")
            .body(payload.to_string())
            .bearer_auth(&self.auth_token)
            .header("Content-Type", "application/octet-stream")
            .send()
            .await?
            .error_for_status()?;

        let graph: types::Graph = resp.json().await?;
        match graph.message {
            None => Ok(graph),
            Some(text) => SyncError::from_message(text),
        }
    }

    pub async fn list_graphs(&self) -> Result<Vec<types::SimpleGraph>> {
        let resp = self
            .client
            .post(url_base().to_owned() + "list_graphs")
            .bearer_auth(&self.auth_token)
            .header("Content-Type", "application/octet-stream")
            .send()
            .await?;

        let result: types::TypicalResponse = resp.json().await?;
        match result.message {
            None => Ok(serde_json::from_value(result.data["Graphs"].clone())?),
            Some(text) => SyncError::from_message(text),
        }
    }

    // get all files metadata: size, etag, key(filepath), last-modified
    pub async fn get_all_files(&self) -> Result<Vec<types::FileObject>> {
        let payload = json!({ "GraphUUID": self.graph_uuid });
        let resp = self
            .client
            .post(url_base().to_owned() + "get_all_files")
            .body(payload.to_string())
            .bearer_auth(&self.auth_token)
            .header("Content-Type", "application/octet-stream")
            .send()
            .await?;

        let result: types::TypicalResponse = resp.json().await?;
        match result.message {
            None => {
                let files: Vec<types::FileObject> =
                    serde_json::from_value(result.data["Objects"].clone())?;
                Ok(files)
            }
            Some(text) => SyncError::from_message(text),
        }
    }

    // files' s3 get-object presigned-url
    pub async fn get_files<P: AsRef<str>, I: IntoIterator<Item = P>>(
        &self,
        files: I,
    ) -> Result<HashMap<String, String>> {
        let payload = json!({
            "GraphUUID": self.graph_uuid,
            "Files": files.into_iter().map(|f| f.as_ref().to_owned()).collect::<Vec<String>>(),
        });
        let resp = self
            .client
            .post(url_base().to_owned() + "get_files")
            .body(payload.to_string())
            .bearer_auth(&self.auth_token)
            .header("Content-Type", "application/octet-stream")
            .send()
            .await?;

        let result: serde_json::Value = resp.json().await?;
        let files: HashMap<String, String> =
            serde_json::from_value(result["PresignedFileUrls"].clone())?;
        Ok(files)
    }

    pub async fn get_version_files<P: AsRef<str>, I: IntoIterator<Item = P>>(
        &self,
        files: I,
    ) -> Result<HashMap<String, String>> {
        let payload = json!({
            "GraphUUID": self.graph_uuid,
            "Files": files.into_iter().map(|f| f.as_ref().to_owned()).collect::<Vec<String>>(),
        });
        let resp = self
            .client
            .post(url_base().to_owned() + "get_version_files")
            .body(payload.to_string())
            .bearer_auth(&self.auth_token)
            .header("Content-Type", "application/octet-stream")
            .send()
            .await?;
        let result: serde_json::Value = resp.json().await?;
        let files: HashMap<String, String> =
            serde_json::from_value(result["PresignedFileUrls"].clone())?;
        Ok(files)
    }

    // expire after 1h
    pub async fn get_temp_credential(&self) -> Result<TempCredential> {
        let resp = self
            .client
            .post(url_base().to_owned() + "get_temp_credential")
            .body("")
            .bearer_auth(&self.auth_token)
            .header("Content-Type", "application/octet-stream")
            .send()
            .await?
            .error_for_status()?;

        let mut credential: TempCredential = resp.json().await?;
        // FIXME: prefix is path style
        credential.s3_prefix = credential
            .s3_prefix
            .strip_prefix(bucket())
            .unwrap()
            .trim_start_matches('/')
            .to_owned()
            + "/";
        Ok(credential)
    }

    // TODO: Use temp file to hold download content
    pub async fn download_file<F>(&self, url: &str, progress_callback: F) -> Result<Vec<u8>>
    where
        F: Fn(usize, usize) + Send + Sync + 'static,
    {
        // FIXME: HEAD requires different signature in presigned URL.
        // Use simulated HEAD request to get file size.
        let head_resp = self
            .client
            .get(url)
            .header("Content-Range", "bytes=0-0")
            .send()
            .await?
            .error_for_status()?;
        let content_length = head_resp.content_length().unwrap_or_default() as usize;

        let timeout = if content_length == 0 {
            // FIXME: unreachable, s3 file should always have a non-zero content-length
            Duration::from_secs(100)
        } else {
            let tt = (content_length / 20 / 1024) as u64;
            Duration::from_secs(u64::max(tt, 50))
        };

        let mut resp = self
            .client
            .get(url)
            .timeout(timeout)
            .send()
            .await?
            .error_for_status()?;

        let mut buf = Vec::with_capacity(1024 * 4);

        let mut nbytes = 0;
        while let Some(chunk) = resp.chunk().await? {
            nbytes += chunk.len();
            buf.extend(chunk);
            progress_callback(nbytes, content_length);
        }
        if content_length != 0 && content_length != nbytes {
            return Err(SyncError::Custom("Incomplete download".to_owned()));
        }
        Ok(buf)
    }

    // upload with Credentials, return remote temp path
    // AccessKeyId, SecretKey, SessionToken
    pub async fn upload_tempfile<F>(
        &self,
        content: Cow<'_, [u8]>,
        progress_callback: F,
    ) -> Result<String>
    where
        F: Fn(usize, usize) + Send + Sync + 'static,
    {
        use s3_presign::Bucket;
        use s3_presign::Credentials;

        let credentials = self.credentials.as_ref().unwrap();

        let credentials = Credentials::new(
            credentials.access_key_id.clone(),
            credentials.secret_key.clone(),
            Some(credentials.session_token.clone()),
        );
        let bucket = Bucket::new(region(), bucket());

        let key = self.s3_prefix.clone().unwrap() + &*random_string(12);

        let presign_url = s3_presign::put(&credentials, &bucket, &key, 60 * 10)
            .ok_or(SyncError::Custom("can not generate presign url".to_owned()))?;

        let content = content.to_owned().to_vec();
        let content_size = content.len();

        // allow 20k/s upload speed
        let timeout = usize::max(content_size / 20 / 1024, 30);

        let stream = ProgressedBytesStream::new(content, progress_callback);

        let resp = self
            .client
            .put(presign_url)
            .body(reqwest::Body::wrap_stream(stream))
            .header("content-length", content_size)
            .header("content-type", "application/octet-stream")
            .timeout(Duration::from_secs(timeout as _))
            .send()
            .await?;

        let code = resp.status().as_u16();
        if code != 200 {
            let body = resp.bytes().await?;
            let content = String::from_utf8_lossy(&body);
            if content.contains("ExpiredToken") {
                return Err(SyncError::ExpiredToken);
            } else {
                return Err(SyncError::Custom(format!(
                    "Can not upload temp file, code={}: {}",
                    code, content
                )));
            }
        }

        Ok(key)
    }

    // key: pages/page1.md
    // value: [s3-prefix]/xxxxxxxxxxx.md
    // key: pages/Hello%20World.md
    // value: [s3-prefix]/xxxxxxxxxxx
    // (key, value, checksum) => (page/page1.md, s3-prefix/xxxxxxxxxxx.md, md5-checksum)
    pub async fn update_files<PK, PV, PH, I>(&self, files: I) -> Result<types::UpdateFiles>
    where
        PK: AsRef<str>,
        PV: AsRef<str>,
        PH: AsRef<str>,
        I: IntoIterator<Item = (PK, PV, PH)>,
    {
        let files: HashMap<String, (String, String)> = files
            .into_iter()
            .map(|(k, v, checksum)| {
                (
                    k.as_ref().to_string(),
                    (v.as_ref().to_string(), checksum.as_ref().to_string()),
                )
            })
            .collect::<HashMap<_, _>>();
        let payload = json!({
            "GraphUUID": self.graph_uuid,
            "TXId": self.txid,
            "Files": files
        });
        let resp = self
            .client
            .post(url_base().to_owned() + "update_files")
            .body(payload.to_string())
            .bearer_auth(&self.auth_token)
            .header("Content-Type", "application/octet-stream")
            .send()
            .await?;

        let result: types::UpdateFiles = resp.json().await?;
        match result.message {
            None => {
                // FIXME: not updated due to self mutablity
                // self.txid = result.txid;
                Ok(result)
            }
            Some(message) => Err(SyncError::Custom(message)),
        }
    }

    pub async fn delete_files<P: AsRef<str>, I: IntoIterator<Item = P>>(
        &mut self,
        files: I,
    ) -> Result<types::DeleteFiles> {
        let payload = json!({
            "GraphUUID": self.graph_uuid,
            "TXId": self.txid,
            "Files": files.into_iter().map(|s| s.as_ref().to_owned()).collect::<Vec<String>>(),
        });

        let resp = self
            .client
            .post(url_base().to_owned() + "delete_files")
            .body(payload.to_string())
            .bearer_auth(&self.auth_token)
            .header("Content-Type", "application/octet-stream")
            .send()
            .await?;

        let result: types::DeleteFiles = resp.json().await?;
        match result.message {
            None => {
                self.txid += result.txid;
                Ok(result)
            }
            Some(message) => Err(SyncError::Custom(message)),
        }
    }

    // FIXME: the API returns redundant paths, since graph_uuid is known,
    // The prefix is useless.
    pub async fn get_diff(&self, txid0: u64) -> Result<Vec<types::Transaction>> {
        let payload = json!({
            "GraphUUID": self.graph_uuid,
            "FromTXId": txid0,
        });

        let resp = self
            .client
            .post(url_base().to_owned() + "get_diff")
            .body(payload.to_string())
            .bearer_auth(&self.auth_token)
            .header("Content-Type", "application/octet-stream")
            .send()
            .await?;

        let result: types::TypicalResponse = resp.json().await?;
        match result.message {
            None => {
                let txns: Vec<types::Transaction> =
                    serde_json::from_value(result.data["Transactions"].clone())?;
                Ok(txns)
            }
            Some(message) => Err(SyncError::Custom(message)),
        }
    }

    // non-ex: 500
    pub async fn rename_file<P1: AsRef<str>, P2: AsRef<str>>(
        &mut self,
        from: P1,
        to: P2,
    ) -> Result<()> {
        let payload = json!({
            "GraphUUID": self.graph_uuid,
            "TXId": self.txid,
            "SrcFile": from.as_ref(),
            "DstFile": to.as_ref(),
        });

        let resp = self
            .client
            .post(url_base().to_owned() + "rename_file")
            .body(payload.to_string())
            .bearer_auth(&self.auth_token)
            .header("Content-Type", "application/octet-stream")
            .send()
            .await?
            .error_for_status()?;

        let result: types::TypicalResponse = resp.json().await?;
        match result.message {
            None => {
                self.txid += result.txid;
                Ok(())
            }
            Some(message) => Err(SyncError::Custom(message)),
        }
    }
}

fn random_string(len: usize) -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let chars: Vec<char> = (0..len)
        .map(|_| {
            let c: u8 = rng.gen_range(b'a'..=b'z');
            c as char
        })
        .collect();
    chars.iter().collect::<String>()
}
