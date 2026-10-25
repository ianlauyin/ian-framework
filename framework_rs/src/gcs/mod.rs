mod encode;
mod error;
mod gcs_client;

pub use error::{GcsError, GcsErrorKind};
pub use gcs_client::{GcsClient, ListPage, ListQuery, ObjectMeta, ObjectReader, PutOptions};

const API_ROOT: &str = "https://storage.googleapis.com/storage/v1";
