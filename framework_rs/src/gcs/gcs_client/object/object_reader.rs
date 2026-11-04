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
                    "read object body failed",
                    code = "GCS_TRANSPORT",
                    source = err.without_url()
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
                    "read object body failed",
                    code = "GCS_TRANSPORT",
                    source = err.without_url()
                )
            })
        }))
    }
}

fn too_large() -> Exception {
    exception!(
        "object is larger than the 256 MiB limit of get_bytes, so use open_object and byte_stream",
        code = "GCS_CONFIG"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gcs::gcs_client::tests::{fake_client, serve_script, Reply};

    #[tokio::test]
    async fn read_to_vec_returns_the_whole_body() {
        let (addr, _server) = serve_script(vec![Reply::new(200, "hello")]).await;
        let reader = fake_client(addr).open_object("a").await.unwrap();
        assert_eq!(reader.read_to_vec().await.unwrap(), b"hello");
    }

    #[tokio::test]
    async fn read_to_vec_refuses_a_declared_size_over_the_limit() {
        let declared = 300 * 1024 * 1024;
        let (addr, _server) =
            serve_script(vec![Reply::new(200, "").declared_length(declared)]).await;
        let reader = fake_client(addr).open_object("a").await.unwrap();
        let err = reader.read_to_vec().await.unwrap_err();
        assert_eq!(err.code(), "GCS_CONFIG");
    }

    #[tokio::test]
    async fn read_to_vec_reports_a_body_that_ends_early() {
        let (addr, _server) = serve_script(vec![Reply::new(200, "abc").declared_length(10)]).await;
        let reader = fake_client(addr).open_object("a").await.unwrap();
        let err = reader.read_to_vec().await.unwrap_err();
        assert_eq!(err.code(), "GCS_TRANSPORT");
    }

    #[tokio::test]
    async fn byte_stream_yields_the_chunks_and_then_an_error() {
        let (addr, _server) = serve_script(vec![Reply::new(200, "abc").declared_length(10)]).await;
        let reader = fake_client(addr).open_object("a").await.unwrap();
        let items: Vec<_> = reader.byte_stream().collect().await;
        assert_eq!(items[0].as_ref().unwrap(), b"abc");
        let err = items.last().unwrap().as_ref().unwrap_err();
        assert_eq!(err.code(), "GCS_TRANSPORT");
    }
}
