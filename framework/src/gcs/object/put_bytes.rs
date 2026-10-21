use super::super::{GcsBackend, GcsError, GcsErrorKind};
use super::resumable::{
    advance_offset, expect_done, put_chunk_retrying, start_resumable, UploadStep, CHUNK_SIZE,
};
use super::shared::{
    content_type_of, validate_metadata, validate_object_name, ObjectMeta, PutOptions,
};

impl GcsBackend {
    pub async fn put_bytes(
        &self,
        name: &str,
        bytes: &[u8],
        options: &PutOptions,
    ) -> Result<ObjectMeta, GcsError> {
        validate_object_name(name)?;
        let content_type = content_type_of(options)?;
        validate_metadata(&options.metadata)?;
        let total = bytes.len() as u64;
        let location =
            start_resumable(self, name, &content_type, &options.metadata, Some(total)).await?;
        if total == 0 {
            return expect_done(
                put_chunk_retrying(self, &location, &content_type, b"", 0, Some(0), true).await?,
            );
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
            match put_chunk_retrying(
                self,
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
