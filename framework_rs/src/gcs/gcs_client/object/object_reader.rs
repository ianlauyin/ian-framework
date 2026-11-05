use std::pin::Pin;

use futures_util::{Stream, StreamExt};

use crate::exception;
use crate::exception::Exception;

const READ_TO_VEC_LIMIT: u64 = 256 * 1024 * 1024;

pub struct ObjectReader {
    pub(crate) response: reqwest::Response,
}

impl ObjectReader {
    pub(crate) async fn read_to_vec(self) -> Result<Vec<u8>, Exception> {
        if self
            .response
            .content_length()
            .is_some_and(|length| length > READ_TO_VEC_LIMIT)
        {
            return Err(too_large());
        }
        let mut stream = self.response.bytes_stream();
        let mut body = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|err| {
                exception!(
                    code = "GCS_TRANSPORT",
                    "read object body failed",
                    source = err.without_url(),
                )
            })?;
            if (body.len() + chunk.len()) as u64 > READ_TO_VEC_LIMIT {
                return Err(too_large());
            }
            body.extend_from_slice(&chunk);
        }
        Ok(body)
    }

    pub fn byte_stream(self) -> Pin<Box<dyn Stream<Item = Result<Vec<u8>, Exception>> + Send>> {
        Box::pin(self.response.bytes_stream().map(|item| {
            item.map(|chunk| chunk.to_vec()).map_err(|err| {
                exception!(
                    code = "GCS_TRANSPORT",
                    "read object body failed",
                    source = err.without_url(),
                )
            })
        }))
    }
}

fn too_large() -> Exception {
    exception!(
        code = "GCS_CONFIG",
        "object is larger than the 256 MiB limit of get_bytes, so use open_object and byte_stream",
    )
}
