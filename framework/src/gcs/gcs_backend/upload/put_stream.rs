use std::pin::Pin;

use futures_util::{Stream, StreamExt};

use super::resumable::{UploadStep, CHUNK_ALIGN, CHUNK_SIZE};
use crate::gcs::gcs_backend::{validate_metadata, validate_object_name, GcsBackend};
use crate::gcs::{GcsError, GcsErrorKind, ObjectMeta, PutOptions};

impl GcsBackend {
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
        let mut pending = Vec::new();
        let mut stream = Box::pin(stream);
        let mut buf = Vec::new();
        let mut origin = 0u64;
        let mut eof = false;
        loop {
            while !eof && buf.len() < CHUNK_SIZE {
                let need = CHUNK_SIZE - buf.len();
                let piece = pull_chunk(&mut stream, &mut pending, need).await?;
                if piece.is_empty() {
                    eof = true;
                    break;
                }
                buf.extend(piece);
            }
            if buf.is_empty() {
                if origin != 0 {
                    return Err(GcsError::new(
                        GcsErrorKind::Api,
                        "finished upload did not return object metadata",
                    ));
                }
                return self
                    .put_chunk_retrying(&location, &content_type, b"", 0, Some(0), true)
                    .await?
                    .expect_done();
            }
            let is_last = eof;
            let send_len = if is_last {
                buf.len()
            } else if !buf.len().is_multiple_of(CHUNK_ALIGN as usize) {
                return Err(GcsError::new(
                    GcsErrorKind::Api,
                    "resumable chunk is not a multiple of 256 KiB",
                ));
            } else {
                CHUNK_SIZE.min(buf.len())
            };
            let total = if is_last {
                Some(origin + send_len as u64)
            } else {
                None
            };
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
                    let end = origin + buf.len() as u64;
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
                    let drop_n = usize::try_from(next - origin).map_err(|_| {
                        GcsError::new(GcsErrorKind::Api, "resumable offset is too large")
                    })?;
                    buf.drain(..drop_n);
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

async fn pull_chunk<S, E>(
    stream: &mut Pin<Box<S>>,
    pending: &mut Vec<u8>,
    max: usize,
) -> Result<Vec<u8>, GcsError>
where
    S: Stream<Item = Result<Vec<u8>, E>> + ?Sized,
    E: std::fmt::Display,
{
    let mut out = Vec::new();
    while out.len() < max {
        if !pending.is_empty() {
            let need = max - out.len();
            if pending.len() <= need {
                out.append(pending);
            } else {
                out.extend(pending.drain(..need));
            }
            continue;
        }
        match stream.next().await {
            None => break,
            Some(Err(err)) => {
                return Err(GcsError::new(
                    GcsErrorKind::Transport,
                    format!("object stream: {err}"),
                ));
            }
            Some(Ok(chunk)) => {
                if !chunk.is_empty() {
                    pending.extend(chunk);
                }
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn pull_chunk_keeps_bytes_across_pieces() {
        let chunks = vec![Ok::<_, &str>(Vec::new()), Ok(vec![1, 2, 3, 4, 5])];
        let mut stream = Box::pin(futures_util::stream::iter(chunks));
        let mut pending = Vec::new();
        let first = pull_chunk(&mut stream, &mut pending, 3).await.unwrap();
        let second = pull_chunk(&mut stream, &mut pending, 3).await.unwrap();
        let third = pull_chunk(&mut stream, &mut pending, 3).await.unwrap();
        assert_eq!(first, vec![1, 2, 3]);
        assert_eq!(second, vec![4, 5]);
        assert_eq!(third, Vec::<u8>::new());
        assert!(pending.is_empty());
    }
}
