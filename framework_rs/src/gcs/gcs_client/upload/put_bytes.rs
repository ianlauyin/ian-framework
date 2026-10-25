use super::resumable::{advance_offset, UploadStep, CHUNK_SIZE};
use crate::gcs::gcs_client::{validate_metadata, validate_object_name, GcsClient};
use crate::gcs::{GcsError, GcsErrorKind, ObjectMeta, PutOptions};

impl GcsClient {
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
            let start = offset as usize;
            let end = (start + CHUNK_SIZE).min(bytes.len());
            let is_last = end == bytes.len();
            let chunk = &bytes[start..end];
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
