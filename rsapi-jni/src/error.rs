use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("jni: {0}")]
    Jni(#[from] jni::errors::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("lsq encryption: {0}")]
    Encrypt(#[from] lsq_encryption::error::Error),
    #[error("sync: {0}")]
    Sync(#[from] rsapi_impl::error::Error),
    #[error("{0}")]
    Other(String),
}
