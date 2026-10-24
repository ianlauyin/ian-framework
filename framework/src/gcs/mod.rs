mod encode;
mod error;
mod gcs_client;
mod object_meta;

pub use error::{GcsError, GcsErrorKind};
pub use gcs_client::{GcsClient, ListPage, ListQuery, ObjectReader, PutOptions};
pub use object_meta::ObjectMeta;

const API_ROOT: &str = "https://storage.googleapis.com/storage/v1";
