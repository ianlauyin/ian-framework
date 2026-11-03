mod encode;
mod error;
pub mod gcs_client;

pub use error::{GcsError, GcsErrorKind};

const API_ROOT: &str = "https://storage.googleapis.com/storage/v1";
const UPLOAD_ROOT: &str = "https://storage.googleapis.com/upload/storage/v1";
const UPLOAD_HOST_ROOT: &str = "https://storage.googleapis.com/";
const IAM_ROOT: &str = "https://iamcredentials.googleapis.com/v1";
