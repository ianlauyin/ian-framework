use std::pin::Pin;

use futures_util::{Stream, StreamExt};

use crate::gcs::{GcsError, GcsErrorKind};

pub struct ObjectReader {
    pub(super) response: reqwest::Response,
}

impl ObjectReader {
    pub async fn read_to_vec(self) -> Result<Vec<u8>, GcsError> {
        let bytes = self.response.bytes().await.map_err(|err| {
            GcsError::with_source(GcsErrorKind::Transport, "read object body failed", err)
        })?;
        Ok(bytes.to_vec())
    }

    pub fn byte_stream(self) -> Pin<Box<dyn Stream<Item = Result<Vec<u8>, GcsError>> + Send>> {
        Box::pin(self.response.bytes_stream().map(|item| {
            item.map(|chunk| chunk.to_vec()).map_err(|err| {
                GcsError::with_source(GcsErrorKind::Transport, "read object body failed", err)
            })
        }))
    }
}
