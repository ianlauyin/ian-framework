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
                code = "GCS_LOCAL",
                format!("cannot open {}", path.display()),
                source = err,
            )
        })?;
        let meta = file.metadata().await.map_err(|err| {
            exception!(
                code = "GCS_LOCAL",
                format!("cannot read metadata for {}", path.display()),
                source = err,
            )
        })?;
        if !meta.is_file() {
            return Err(exception!(
                code = "GCS_CONFIG",
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

    pub(crate) async fn read_chunk(
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
        .ok_or_else(|| exception!(code = "GCS_API", "upload range is past the end of the data"))
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
                code = "GCS_LOCAL",
                "cannot seek in the upload file",
                source = err,
            )
        })?;
    let mut chunk = vec![0u8; len];
    file.read_exact(&mut chunk)
        .await
        .map_err(|err| exception!(code = "GCS_LOCAL", "read file failed", source = err))?;
    Ok(chunk)
}
