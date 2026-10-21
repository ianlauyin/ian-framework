use std::path::Path;

use super::super::{GcsBackend, GcsError, GcsErrorKind};
use super::resumable::{
    advance_offset, expect_done, put_chunk_retrying, start_resumable, UploadStep, CHUNK_SIZE,
};
use super::shared::{
    content_type_of, validate_metadata, validate_object_name, ObjectMeta, PutOptions,
};

impl GcsBackend {
    pub async fn put_file(
        &self,
        name: &str,
        path: &Path,
        options: &PutOptions,
    ) -> Result<ObjectMeta, GcsError> {
        validate_object_name(name)?;
        let content_type = content_type_of(options)?;
        validate_metadata(&options.metadata)?;
        let mut file = tokio::fs::File::open(path).await.map_err(|err| {
            GcsError::with_source(
                GcsErrorKind::Transport,
                format!("cannot open {}", path.display()),
                err,
            )
        })?;
        let meta = file.metadata().await.map_err(|err| {
            GcsError::with_source(
                GcsErrorKind::Transport,
                format!("cannot read metadata for {}", path.display()),
                err,
            )
        })?;
        if !meta.is_file() {
            return Err(GcsError::new(
                GcsErrorKind::Config,
                format!("{} is not a file", path.display()),
            ));
        }
        let total = meta.len();
        let location =
            start_resumable(self, name, &content_type, &options.metadata, Some(total)).await?;
        if total == 0 {
            return expect_done(
                put_chunk_retrying(self, &location, &content_type, b"", 0, Some(0), true).await?,
            );
        }
        let mut offset = 0u64;
        let mut rewinds = 0u32;
        use tokio::io::AsyncSeekExt;
        while offset < total {
            let remaining = total - offset;
            let is_last = remaining <= CHUNK_SIZE as u64;
            let len = usize::try_from(remaining.min(CHUNK_SIZE as u64)).unwrap_or(CHUNK_SIZE);
            file.seek(std::io::SeekFrom::Start(offset))
                .await
                .map_err(|err| {
                    GcsError::with_source(
                        GcsErrorKind::Transport,
                        "cannot seek in the upload file",
                        err,
                    )
                })?;
            let chunk = read_exact_n(&mut file, len).await?;
            match put_chunk_retrying(
                self,
                &location,
                &content_type,
                &chunk,
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

async fn read_exact_n(file: &mut tokio::fs::File, len: usize) -> Result<Vec<u8>, GcsError> {
    use tokio::io::AsyncReadExt;
    let mut buf = vec![0u8; len];
    let mut filled = 0;
    while filled < len {
        let got = file.read(&mut buf[filled..]).await.map_err(|err| {
            GcsError::with_source(GcsErrorKind::Transport, "read file failed", err)
        })?;
        if got == 0 {
            return Err(GcsError::new(
                GcsErrorKind::Transport,
                "file ended before the expected length",
            ));
        }
        filled += got;
    }
    Ok(buf)
}
