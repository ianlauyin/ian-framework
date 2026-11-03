use std::borrow::Cow;

use tokio::io::{AsyncReadExt, AsyncSeekExt};

use crate::gcs::gcs_client::PutBody;
use crate::gcs::{GcsError, GcsErrorKind};

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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn bytes_are_read_from_the_offset() {
        let body = PutBody::Bytes(b"abcdef".to_vec());
        let mut source = UploadSource::open(&body).await.unwrap();
        assert_eq!(source.len(), 6);
        let chunk = source.read_chunk(2, 3).await.unwrap();
        assert_eq!(&chunk[..], b"cde");
    }

    #[tokio::test]
    async fn a_file_is_read_in_a_chunk() {
        let path = std::env::temp_dir().join(format!("upload-src-{}.bin", std::process::id()));
        std::fs::write(&path, b"hello").unwrap();
        let body = PutBody::File(path.clone());
        let mut source = UploadSource::open(&body).await.unwrap();
        assert_eq!(source.len(), 5);
        let chunk = source.read_chunk(1, 3).await.unwrap();
        assert_eq!(&chunk[..], b"ell");
        std::fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn a_missing_path_and_a_directory_are_rejected() {
        let missing = std::env::temp_dir().join(format!("upload-missing-{}", std::process::id()));
        let Err(err) = UploadSource::open(&PutBody::File(missing)).await else {
            panic!("expected an error");
        };
        assert_eq!(err.kind(), GcsErrorKind::Transport);
        let Err(err) = UploadSource::open(&PutBody::File(std::env::temp_dir())).await else {
            panic!("expected an error");
        };
        assert_eq!(err.kind(), GcsErrorKind::Config);
    }
}
