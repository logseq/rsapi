use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("{0}")]
    Encryption(#[from] lsq_encryption::Error),
    #[error(transparent)]
    SyncClient(#[from] sync::Error),
    #[error("graph env not set, uuid not found")]
    GraphNotSet,
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid arguments")]
    InvalidArg,
    #[error("cancelled")]
    Cancelled,
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(feature = "napi")]
impl From<Error> for napi::Error {
    fn from(error: Error) -> Self {
        napi::Error::new(napi::Status::GenericFailure, error.to_string())
    }
}
