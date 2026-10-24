use super::resumable::{advance_offset, UploadStep, CHUNK_SIZE};
use crate::gcs::gcs_backend::{validate_metadata, validate_object_name, GcsBackend};
use crate::gcs::{GcsError, GcsErrorKind, ObjectMeta, PutOptions};

impl GcsBackend {
    pub async fn put_bytes(
        &self,
        name: &str,
        bytes: &[u8],
        options: &PutOptions,
    ) -> Result<ObjectMeta, GcsError> {
        validate_object_name(name)?;
        let content_type = options.content_type()?;
        validate_metadata(&options.metadata)?;
        let total = bytes.len() as u64;
        let location = self
            .start_resumable(name, &content_type, &options.metadata, Some(total))
            .await?;
        if total == 0 {
            return self
                .put_chunk_retrying(&location, &content_type, b"", 0, Some(0), true)
                .await?
                .expect_done();
        }
        let mut offset = 0u64;
        let mut rewinds = 0u32;
        while offset < total {
            let remaining = total - offset;
            let is_last = remaining <= CHUNK_SIZE as u64;
            let len = usize::try_from(remaining.min(CHUNK_SIZE as u64)).unwrap_or(CHUNK_SIZE);
            let start = usize::try_from(offset)
                .map_err(|_| GcsError::new(GcsErrorKind::Api, "object is too large"))?;
            let chunk = &bytes[start..start + len];
            match self
                .put_chunk_retrying(
                    &location,
                    &content_type,
                    chunk,
                    offset,
                    Some(total),
                    is_last,
                )
                .await?
            {
                UploadStep::Done(meta) => return Ok(meta),
                UploadStep::Continue { next } => {
                    advance_offset(&mut offset, &mut rewinds, next, total)?;
                }
            }
        }
        Err(GcsError::new(
            GcsErrorKind::Api,
            "finished upload did not return object metadata",
        ))
    }
}
