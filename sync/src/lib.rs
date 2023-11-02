pub use error::SyncError as Error;
pub use sync::{reset_user, set_dev, set_prod, set_proxy, SyncClient};

mod doh;
mod error;
pub mod helpers;
pub mod sync;
pub mod types;

pub type Result<T> = std::result::Result<T, Error>;
