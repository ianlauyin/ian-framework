use std::borrow::Cow;

use tokio::io::{AsyncReadExt, AsyncSeekExt};

use crate::gcs::{GcsError, GcsErrorKind, PutBody};

// The source must re-read any range because the session can move the offset back.
pub(crate) enum UploadSource<'a> {
    Bytes(&'a [u8]),
    File { file: tokio::fs::File, len: u64 },
}

impl<'a> UploadSource<'a> {
    pub(crate) async fn open(body: &'a PutBody) -> Result<Self, GcsError> {
        let path = match body {
            PutBody::Bytes(bytes) => return Ok(Self::Bytes(bytes)),
            PutBody::File(path) => path,
        };
        let file = tokio::fs::File::open(path).await.map_err(|err| {
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
        Ok(Self::File {
            file,
            len: meta.len(),
        })
    }

    pub(crate) fn len(&self) -> u64 {
        match self {
            Self::Bytes(bytes) => bytes.len() as u64,
            Self::File { len, .. } => *len,
        }
    }

    pub(super) async fn read_chunk(
        &mut self,
        offset: u64,
        len: usize,
    ) -> Result<Cow<'a, [u8]>, GcsError> {
        match self {
            Self::Bytes(bytes) => {
                let start = offset as usize;
                Ok(Cow::Borrowed(&bytes[start..start + len]))
            }
            Self::File { file, .. } => {
                file.seek(std::io::SeekFrom::Start(offset))
                    .await
                    .map_err(|err| {
                        GcsError::with_source(
                            GcsErrorKind::Transport,
                            "cannot seek in the upload file",
                            err,
                        )
                    })?;
                let mut chunk = vec![0u8; len];
                file.read_exact(&mut chunk).await.map_err(|err| {
                    GcsError::with_source(GcsErrorKind::Transport, "read file failed", err)
                })?;
                Ok(Cow::Owned(chunk))
            }
        }
    }
}
