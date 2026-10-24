mod encode;
mod error;
mod gcs_client;
mod list_page;
mod list_query;
mod object_meta;
mod object_reader;
mod put_options;

pub use encode::{encode_object_id, encode_object_path, encode_seg};
pub use error::{GcsError, GcsErrorKind};
pub use gcs_client::GcsClient;
pub use list_page::ListPage;
pub use list_query::ListQuery;
pub use object_meta::ObjectMeta;
pub use object_reader::ObjectReader;
pub use put_options::PutOptions;

const API_ROOT: &str = "https://storage.googleapis.com/storage/v1";
