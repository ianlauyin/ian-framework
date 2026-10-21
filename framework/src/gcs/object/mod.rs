mod copy_object;
mod delete_object;
mod get_bytes;
mod list;
mod open_object;
mod patch_metadata;
mod put_bytes;
mod put_file;
mod put_stream;
mod read;
mod resumable;
mod rewrite_object;
mod shared;
mod stat;

pub use read::ObjectReader;
pub use shared::{ListPage, ListQuery, ObjectMeta, PutOptions};

pub(crate) use shared::validate_object_name;
