use std::collections::BTreeMap;
use std::pin::Pin;

use bytes::Bytes;
use chrono::{DateTime, Utc};
use futures_util::stream;
use futures_util::Stream;
use google_cloud_storage::model::Object;
use google_cloud_storage::read_object::ReadObjectResponse;

use crate::exception;
use crate::exception::Exception;
use crate::gcp::error::to_exception;

pub const OBJECT_TOO_LARGE: &str = "OBJECT_TOO_LARGE";

const INTO_BYTES_LIMIT: u64 = 256 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectMeta {
    pub name: String,
    pub size: u64,
    pub content_type: Option<String>,
    pub updated: Option<DateTime<Utc>>,
    pub metadata: BTreeMap<String, String>,
}

impl From<Object> for ObjectMeta {
    fn from(object: Object) -> Self {
        Self {
            name: object.name,
            size: u64::try_from(object.size).unwrap_or_default(),
            content_type: Some(object.content_type).filter(|value| !value.is_empty()),
            updated: object.update_time.and_then(|time| {
                DateTime::from_timestamp(
                    time.seconds(),
                    u32::try_from(time.nanos()).unwrap_or_default(),
                )
            }),
            metadata: object.metadata.into_iter().collect(),
        }
    }
}

pub struct ObjectReader {
    pub(crate) response: ReadObjectResponse,
}

impl ObjectReader {
    pub fn into_stream(self) -> Pin<Box<dyn Stream<Item = Result<Bytes, Exception>> + Send>> {
        Box::pin(stream::unfold(self.response, |mut response| async move {
            let item = response.next().await?;
            Some((item.map_err(body_read_exception), response))
        }))
    }

    pub async fn into_bytes(mut self) -> Result<Vec<u8>, Exception> {
        if u64::try_from(self.response.object().size).is_ok_and(|size| size > INTO_BYTES_LIMIT) {
            return Err(too_large_exception());
        }
        let mut body = Vec::new();
        while let Some(chunk) = self.response.next().await {
            let chunk = chunk.map_err(body_read_exception)?;
            if (body.len() + chunk.len()) as u64 > INTO_BYTES_LIMIT {
                return Err(too_large_exception());
            }
            body.extend_from_slice(&chunk);
        }
        Ok(body)
    }
}

impl std::fmt::Debug for ObjectReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ObjectReader")
            .field("size", &self.response.object().size)
            .finish_non_exhaustive()
    }
}

fn body_read_exception(err: google_cloud_storage::Error) -> Exception {
    to_exception("read object body failed", err)
}

fn too_large_exception() -> Exception {
    exception!(
        code = OBJECT_TOO_LARGE,
        "object is larger than the 256 MiB limit of into_bytes, so use into_stream",
    )
}
