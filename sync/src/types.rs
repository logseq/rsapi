use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Graph {
    // error message
    pub(crate) message: Option<String>,
    #[serde(default, rename = "StorageUsage")]
    pub storage_usage: u64,
    #[serde(default, rename = "TXId")]
    pub txid: i64,
    #[serde(default, rename = "GraphName")]
    pub graph_name: String,
    #[serde(default, rename = "GraphUUID")]
    pub graph_uuid: String,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimpleGraph {
    #[serde(default, rename = "GraphName")]
    pub graph_name: String,
    #[serde(default, rename = "GraphUUID")]
    pub graph_uuid: String,
}

impl From<Graph> for SimpleGraph {
    fn from(graph: Graph) -> Self {
        SimpleGraph {
            graph_name: graph.graph_name,
            graph_uuid: graph.graph_uuid,
        }
    }
}

// get_temp_credential
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct TempCredential {
    pub credentials: Credentials,
    pub s3_prefix: String,
}

/// S3 credentials
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Credentials {
    pub access_key_id: String,
    pub expiration: DateTime<Utc>,
    pub secret_key: String,
    pub session_token: String,
}

impl Credentials {
    // 5min to be expired
    pub fn is_expired(&self) -> bool {
        self.expiration < Utc::now() + chrono::Duration::seconds(60 * 5)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct FileObject {
    #[serde(rename = "ETag")]
    pub etag: String,
    pub key: String,
    pub last_modified: DateTime<Utc>,
    //pub storage_class: String,
    //pub owner: Option<String>,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpdateFiles {
    pub(crate) message: Option<String>,
    #[serde(default, rename = "TXId")]
    pub txid: i64,
    #[serde(default, rename = "UpdateSuccFiles")]
    pub updated_files: Vec<String>,
    #[serde(default, rename = "UpdateFailedFiles")]
    failed_files: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeleteFiles {
    pub(crate) message: Option<String>,
    #[serde(default, rename = "TXId")]
    pub txid: i64,
    #[serde(default, rename = "DeleteSuccFiles")]
    pub deleted_files: Vec<String>,
    #[serde(default, rename = "DeleteFailedFiles")]
    failed_files: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transaction {
    #[serde(rename = "TXId")]
    pub txid: i64,
    #[serde(rename = "TXType")]
    pub r#type: String,
    #[serde(rename = "TXContent")]
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct TypicalResponse {
    pub message: Option<String>,
    #[serde(default, rename = "TXId")]
    pub txid: i64,
    #[serde(default, flatten)]
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GetObject {
    pub message: Option<String>,
}
