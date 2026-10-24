use futures_util::{Stream, StreamExt};

use super::resumable::{UploadStep, CHUNK_SIZE};
use crate::gcs::gcs_client::{validate_metadata, validate_object_name, GcsClient};
use crate::gcs::{GcsError, GcsErrorKind, ObjectMeta, PutOptions};

impl GcsClient {
    pub async fn put_stream<S, E>(
        &self,
        name: &str,
        stream: S,
        options: &PutOptions,
    ) -> Result<ObjectMeta, GcsError>
    where
        S: Stream<Item = Result<Vec<u8>, E>> + Send,
        E: std::fmt::Display,
    {
        validate_object_name(name)?;
        let content_type = options.content_type()?;
        validate_metadata(&options.metadata)?;
        let location = self
            .start_resumable(name, &content_type, &options.metadata, None)
            .await?;
        let mut stream = Box::pin(stream);
        let mut buf = Vec::new();
        let mut origin = 0u64;
        let mut eof = false;
        loop {
            while !eof && buf.len() < CHUNK_SIZE {
                match stream.next().await {
                    None => eof = true,
                    Some(Err(err)) => {
                        return Err(GcsError::new(
                            GcsErrorKind::Transport,
                            format!("object stream: {err}"),
                        ));
                    }
                    Some(Ok(piece)) => buf.extend(piece),
                }
            }
            if buf.is_empty() {
                // The stream is empty, or it ended on a chunk boundary.
                return self
                    .put_chunk_retrying(&location, &content_type, b"", origin, Some(origin), true)
                    .await?
                    .expect_done();
            }
            let is_last = eof;
            let send_len = if is_last { buf.len() } else { CHUNK_SIZE };
            let end = origin + send_len as u64;
            let total = if is_last { Some(end) } else { None };
            match self
                .put_chunk_retrying(
                    &location,
                    &content_type,
                    &buf[..send_len],
                    origin,
                    total,
                    is_last,
                )
                .await?
            {
                UploadStep::Done(meta) => return Ok(meta),
                UploadStep::Continue { next } => {
                    if next < origin {
                        return Err(GcsError::new(
                            GcsErrorKind::Transport,
                            "resumable session dropped bytes that are not in memory",
                        ));
                    }
                    if next > end {
                        return Err(GcsError::new(
                            GcsErrorKind::Api,
                            "resumable session is ahead of the buffered bytes",
                        ));
                    }
                    if next == origin {
                        return Err(GcsError::new(
                            GcsErrorKind::Api,
                            "resumable upload did not advance",
                        ));
                    }
                    buf.drain(..(next - origin) as usize);
                    origin = next;
                    if buf.is_empty() && eof {
                        return Err(GcsError::new(
                            GcsErrorKind::Api,
                            "finished upload did not return object metadata",
                        ));
                    }
                }
            }
        }
    }
}
