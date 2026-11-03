use std::borrow::Cow;

use tokio::io::{AsyncReadExt, AsyncSeekExt};

use crate::exception;
use crate::exception::Exception;
use crate::gcs::gcs_client::PutBody;

// The source must re-read any range because the session can move the offset back.
pub(crate) enum UploadSource<'a> {
    Bytes(&'a [u8]),
    File { file: tokio::fs::File, len: u64 },
}

impl<'a> UploadSource<'a> {
    pub(crate) async fn open(body: &'a PutBody) -> Result<Self, Exception> {
        let path = match body {
            PutBody::Bytes(bytes) => return Ok(Self::Bytes(bytes)),
            PutBody::File(path) => path,
        };
        let file = tokio::fs::File::open(path).await.map_err(|err| {
            exception!(
                format!("cannot open {}", path.display()),
                code = "GCS_LOCAL",
                source = err
            )
        })?;
        let meta = file.metadata().await.map_err(|err| {
            exception!(
                format!("cannot read metadata for {}", path.display()),
                code = "GCS_LOCAL",
                source = err
            )
        })?;
        if !meta.is_file() {
            return Err(exception!(
                format!("{} is not a file", path.display()),
                code = "GCS_CONFIG"
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
    ) -> Result<Cow<'a, [u8]>, Exception> {
        match self {
            Self::Bytes(bytes) => bytes_chunk(bytes, offset, len).map(Cow::Borrowed),
            Self::File { file, .. } => file_chunk(file, offset, len).await.map(Cow::Owned),
        }
    }
}

fn bytes_chunk(bytes: &[u8], offset: u64, len: usize) -> Result<&[u8], Exception> {
    let range = usize::try_from(offset)
        .ok()
        .and_then(|start| Some(start..start.checked_add(len)?));
    range
        .and_then(|range| bytes.get(range))
        .ok_or_else(|| exception!("upload range is past the end of the data", code = "GCS_API"))
}

async fn file_chunk(
    file: &mut tokio::fs::File,
    offset: u64,
    len: usize,
) -> Result<Vec<u8>, Exception> {
    file.seek(std::io::SeekFrom::Start(offset))
        .await
        .map_err(|err| {
            exception!(
                "cannot seek in the upload file",
                code = "GCS_LOCAL",
                source = err
            )
        })?;
    let mut chunk = vec![0u8; len];
    file.read_exact(&mut chunk)
        .await
        .map_err(|err| exception!("read file failed", code = "GCS_LOCAL", source = err))?;
    Ok(chunk)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str, content: &[u8]) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("my-app-{}-{name}", std::process::id()));
        std::fs::write(&path, content).unwrap();
        path
    }

    #[tokio::test]
    async fn bytes_source_reads_a_range_and_refuses_one_past_the_end() {
        let body = PutBody::Bytes(b"hello".to_vec());
        let mut source = UploadSource::open(&body).await.unwrap();
        assert_eq!(source.len(), 5);
        assert_eq!(source.read_chunk(1, 3).await.unwrap().as_ref(), b"ell");
        let err = source.read_chunk(3, 3).await.unwrap_err();
        assert_eq!(err.code(), Some("GCS_API"));
        let err = source.read_chunk(u64::MAX, usize::MAX).await.unwrap_err();
        assert_eq!(err.code(), Some("GCS_API"));
    }

    #[tokio::test]
    async fn file_source_rereads_any_range() {
        let path = temp_file("source-range", b"hello");
        let body = PutBody::File(path.clone());
        let mut source = UploadSource::open(&body).await.unwrap();
        assert_eq!(source.len(), 5);
        assert_eq!(source.read_chunk(2, 3).await.unwrap().as_ref(), b"llo");
        assert_eq!(source.read_chunk(0, 2).await.unwrap().as_ref(), b"he");
        let err = source.read_chunk(3, 9).await.unwrap_err();
        assert_eq!(err.code(), Some("GCS_LOCAL"));
        std::fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn file_source_refuses_a_missing_file_and_a_directory() {
        let missing = PutBody::File(std::env::temp_dir().join("my-app-missing-file"));
        let err = UploadSource::open(&missing).await.map(|_| ()).unwrap_err();
        assert_eq!(err.code(), Some("GCS_LOCAL"));
        let directory = PutBody::File(std::env::temp_dir());
        let err = UploadSource::open(&directory)
            .await
            .map(|_| ())
            .unwrap_err();
        assert_eq!(err.code(), Some("GCS_CONFIG"));
    }
}
