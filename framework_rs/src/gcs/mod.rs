mod encode;
mod error;
pub mod gcs_client;

pub use error::{GcsError, GcsErrorKind};

const API_ROOT: &str = "https://storage.googleapis.com/storage/v1";
