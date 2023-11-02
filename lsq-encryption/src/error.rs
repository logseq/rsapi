use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Invalid key format, cannot parse")]
    ParseKey,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("cannot decrypt")]
    Decrypt,
    #[error("cannot encrypt")]
    Encrypt,
    #[error("invalid arguments")]
    InvalidArg,
    #[error("cannot age decrypt: {0}")]
    AgeDecrypt(#[from] age::DecryptError),
    #[error("cannot age encrypt: {0}")]
    AgeEncrypt(#[from] age::EncryptError),
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(feature = "napi")]
impl From<Error> for napi::Error {
    fn from(error: Error) -> Self {
        napi::Error::new(napi::Status::InvalidArg, error.to_string())
    }
}
