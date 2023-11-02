use thiserror::Error;

#[derive(Error, Debug)]
pub enum SyncError {
    #[error("authorize failed")]
    Unauthorized,
    #[error("unknown error")]
    Unknown,
    #[error("ExpiredToken: s3 token expired")]
    ExpiredToken,
    // TODO: handle status code
    #[error("reqwest error: {0}")]
    Request(#[from] reqwest::Error),
    #[error("serde error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("utf8 error: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    #[error("{0}")]
    Custom(String),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl SyncError {
    pub fn from_message<T>(message: String) -> Result<T, Self> {
        match &*message {
            "Unauthorized" => Err(SyncError::Unauthorized),
            "ExistedGraphErr" => Err(SyncError::Custom(message)),
            // invalid content-type
            "Internal Server Error" => Err(SyncError::Custom("Server Error".to_string())),
            _ => Err(SyncError::Custom(message)),
        }
    }
}
