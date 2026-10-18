use std::collections::BTreeMap;
use std::path::Path;
use std::pin::Pin;
use std::time::Duration;

use chrono::{DateTime, Utc};
use futures_util::{Stream, StreamExt};
use serde::Deserialize;

use super::encode::{encode_object_id, encode_seg};
use super::{
    bucket, is_retryable, send, Call, GcsBackend, GcsError, GcsErrorKind, TRANSFER_TIMEOUT,
};

const DEFAULT_CONTENT_TYPE: &str = "application/octet-stream";
const CHUNK_SIZE: usize = 8 * 1024 * 1024;
const CHUNK_ALIGN: u64 = 256 * 1024;
const CHUNK_ATTEMPTS: u32 = 5;
const CHUNK_BACKOFF: Duration = Duration::from_millis(200);
const API_ROOT: &str = "https://storage.googleapis.com/storage/v1";
const UPLOAD_ROOT: &str = "https://storage.googleapis.com/upload/storage/v1";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PutOptions {
    pub content_type: Option<String>,
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectMeta {
    pub name: String,
    pub size: u64,
    pub content_type: Option<String>,
    pub updated: Option<DateTime<Utc>>,
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListPage {
    pub objects: Vec<ObjectMeta>,
    pub prefixes: Vec<String>,
    pub next_page_token: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListQuery {
    pub prefix: Option<String>,
    pub delimiter: Option<String>,
    pub page_token: Option<String>,
    pub max_results: Option<u32>,
}

pub struct ObjectReader {
    response: reqwest::Response,
}

pub async fn put_bytes(
    backend: &GcsBackend,
    name: &str,
    bytes: &[u8],
    options: &PutOptions,
) -> Result<ObjectMeta, GcsError> {
    validate_object_name(name)?;
    let content_type = content_type_of(options)?;
    validate_metadata(&options.metadata)?;
    let total = bytes.len() as u64;
    let location =
        start_resumable(backend, name, &content_type, &options.metadata, Some(total)).await?;
    if total == 0 {
        return expect_done(
            put_chunk_retrying(backend, &location, &content_type, b"", 0, Some(0), true).await?,
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
            backend,
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

pub async fn put_file(
    backend: &GcsBackend,
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
        start_resumable(backend, name, &content_type, &options.metadata, Some(total)).await?;
    if total == 0 {
        return expect_done(
            put_chunk_retrying(backend, &location, &content_type, b"", 0, Some(0), true).await?,
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
            backend,
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

pub async fn put_stream<S, E>(
    backend: &GcsBackend,
    name: &str,
    stream: S,
    options: &PutOptions,
) -> Result<ObjectMeta, GcsError>
where
    S: Stream<Item = Result<Vec<u8>, E>> + Send,
    E: std::fmt::Display,
{
    validate_object_name(name)?;
    let content_type = content_type_of(options)?;
    validate_metadata(&options.metadata)?;
    let location = start_resumable(backend, name, &content_type, &options.metadata, None).await?;
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
            return expect_done(
                put_chunk_retrying(backend, &location, &content_type, b"", 0, Some(0), true)
                    .await?,
            );
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
        match put_chunk_retrying(
            backend,
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

pub async fn get_bytes(backend: &GcsBackend, name: &str) -> Result<Vec<u8>, GcsError> {
    read_to_vec(open_object(backend, name).await?).await
}

pub async fn open_object(backend: &GcsBackend, name: &str) -> Result<ObjectReader, GcsError> {
    validate_object_name(name)?;
    let response = send(
        backend,
        &Call {
            method: reqwest::Method::GET,
            url: format!("{}?alt=media", object_url(bucket(backend), name)),
            headers: Vec::new(),
            json: None,
            bytes: None,
            extra_status: &[],
            timeout: None,
        },
    )
    .await?;
    Ok(ObjectReader { response })
}

pub async fn read_to_vec(reader: ObjectReader) -> Result<Vec<u8>, GcsError> {
    let bytes = reader.response.bytes().await.map_err(|err| {
        GcsError::with_source(GcsErrorKind::Transport, "read object body failed", err)
    })?;
    Ok(bytes.to_vec())
}

pub fn byte_stream(
    reader: ObjectReader,
) -> Pin<Box<dyn Stream<Item = Result<Vec<u8>, GcsError>> + Send>> {
    Box::pin(reader.response.bytes_stream().map(|item| {
        item.map(|chunk| chunk.to_vec()).map_err(|err| {
            GcsError::with_source(GcsErrorKind::Transport, "read object body failed", err)
        })
    }))
}

pub async fn stat(backend: &GcsBackend, name: &str) -> Result<ObjectMeta, GcsError> {
    validate_object_name(name)?;
    let response = send(
        backend,
        &Call {
            method: reqwest::Method::GET,
            url: object_url(bucket(backend), name),
            headers: Vec::new(),
            json: None,
            bytes: None,
            extra_status: &[],
            timeout: None,
        },
    )
    .await?;
    meta_from_response(response).await
}

pub async fn list(backend: &GcsBackend, query: &ListQuery) -> Result<ListPage, GcsError> {
    let response = send(
        backend,
        &Call {
            method: reqwest::Method::GET,
            url: list_url(bucket(backend), query),
            headers: Vec::new(),
            json: None,
            bytes: None,
            extra_status: &[],
            timeout: None,
        },
    )
    .await?;
    let bytes = response.bytes().await.map_err(|err| {
        GcsError::with_source(GcsErrorKind::Transport, "list response is not bytes", err)
    })?;
    list_page_from_slice(&bytes)
}

pub async fn delete_object(backend: &GcsBackend, name: &str) -> Result<(), GcsError> {
    validate_object_name(name)?;
    let response = send(
        backend,
        &Call {
            method: reqwest::Method::DELETE,
            url: object_url(bucket(backend), name),
            headers: Vec::new(),
            json: None,
            bytes: None,
            extra_status: &[],
            timeout: None,
        },
    )
    .await?;
    let _ = response.bytes().await;
    Ok(())
}

pub async fn patch_metadata(
    backend: &GcsBackend,
    name: &str,
    metadata: &BTreeMap<String, String>,
) -> Result<ObjectMeta, GcsError> {
    validate_object_name(name)?;
    validate_metadata(metadata)?;
    let response = send(
        backend,
        &Call {
            method: reqwest::Method::PATCH,
            url: object_url(bucket(backend), name),
            headers: Vec::new(),
            json: Some(serde_json::json!({ "metadata": metadata })),
            bytes: None,
            extra_status: &[],
            timeout: None,
        },
    )
    .await?;
    meta_from_response(response).await
}

pub async fn copy_object(
    backend: &GcsBackend,
    from: &str,
    to: &str,
) -> Result<ObjectMeta, GcsError> {
    validate_object_name(from)?;
    validate_object_name(to)?;
    let url = format!(
        "{}/copyTo/b/{}/o/{}",
        object_url(bucket(backend), from),
        encode_seg(bucket(backend)),
        encode_object_id(to),
    );
    let response = send(
        backend,
        &Call {
            method: reqwest::Method::POST,
            url,
            headers: Vec::new(),
            json: Some(serde_json::json!({})),
            bytes: None,
            extra_status: &[],
            timeout: None,
        },
    )
    .await?;
    meta_from_response(response).await
}

pub async fn rewrite_object(
    backend: &GcsBackend,
    from: &str,
    to: &str,
) -> Result<ObjectMeta, GcsError> {
    validate_object_name(from)?;
    validate_object_name(to)?;
    let mut token: Option<String> = None;
    loop {
        let response = send(
            backend,
            &Call {
                method: reqwest::Method::POST,
                url: rewrite_url(bucket(backend), from, to, token.as_deref()),
                headers: Vec::new(),
                json: Some(serde_json::json!({})),
                bytes: None,
                extra_status: &[],
                timeout: None,
            },
        )
        .await?;
        let bytes = response.bytes().await.map_err(|err| {
            GcsError::with_source(
                GcsErrorKind::Transport,
                "rewrite response is not bytes",
                err,
            )
        })?;
        let step = rewrite_step_from_slice(&bytes)?;
        if step.done {
            return step.resource.ok_or_else(|| {
                GcsError::new(GcsErrorKind::Api, "rewrite did not return object metadata")
            });
        }
        let next = step
            .rewrite_token
            .ok_or_else(|| GcsError::new(GcsErrorKind::Api, "rewrite did not return a token"))?;
        if token.as_deref() == Some(next.as_str()) {
            return Err(GcsError::new(
                GcsErrorKind::Api,
                "rewrite token did not change",
            ));
        }
        token = Some(next);
    }
}

pub(crate) fn validate_object_name(name: &str) -> Result<(), GcsError> {
    if name.is_empty() || name.contains(['\r', '\n']) {
        return Err(GcsError::new(
            GcsErrorKind::Config,
            "object name is empty or contains a line break",
        ));
    }
    Ok(())
}

fn content_type_of(options: &PutOptions) -> Result<String, GcsError> {
    match &options.content_type {
        Some(value) => {
            if value.is_empty() || !value.bytes().all(|byte| (0x20..=0x7e).contains(&byte)) {
                return Err(GcsError::new(
                    GcsErrorKind::Config,
                    "content type is empty or is not printable ASCII",
                ));
            }
            Ok(value.clone())
        }
        None => Ok(DEFAULT_CONTENT_TYPE.to_string()),
    }
}

fn validate_metadata(metadata: &BTreeMap<String, String>) -> Result<(), GcsError> {
    for (key, value) in metadata {
        if key.is_empty() || key.contains(['\r', '\n']) || value.contains(['\r', '\n']) {
            return Err(GcsError::new(
                GcsErrorKind::Config,
                "metadata key is empty or metadata contains a line break",
            ));
        }
    }
    Ok(())
}

fn object_url(bucket_name: &str, name: &str) -> String {
    format!(
        "{API_ROOT}/b/{}/o/{}",
        encode_seg(bucket_name),
        encode_object_id(name),
    )
}

fn rewrite_url(bucket_name: &str, from: &str, to: &str, token: Option<&str>) -> String {
    let mut url = format!(
        "{}/rewriteTo/b/{}/o/{}",
        object_url(bucket_name, from),
        encode_seg(bucket_name),
        encode_object_id(to),
    );
    if let Some(token) = token {
        url.push_str("?rewriteToken=");
        url.push_str(&encode_seg(token));
    }
    url
}

fn list_url(bucket_name: &str, query: &ListQuery) -> String {
    let max_results = query.max_results.map(|max| max.to_string());
    let mut pairs: Vec<(&str, &str)> = Vec::new();
    if let Some(prefix) = query.prefix.as_deref() {
        pairs.push(("prefix", prefix));
    }
    if let Some(delimiter) = query.delimiter.as_deref() {
        pairs.push(("delimiter", delimiter));
    }
    if let Some(token) = query.page_token.as_deref() {
        pairs.push(("pageToken", token));
    }
    if let Some(max) = max_results.as_deref() {
        pairs.push(("maxResults", max));
    }
    pairs.sort_by(|left, right| left.0.cmp(right.0));
    let mut url = format!("{API_ROOT}/b/{}/o", encode_seg(bucket_name));
    if pairs.is_empty() {
        return url;
    }
    url.push('?');
    for (index, (key, value)) in pairs.iter().enumerate() {
        if index > 0 {
            url.push('&');
        }
        url.push_str(&encode_seg(key));
        url.push('=');
        url.push_str(&encode_seg(value));
    }
    url
}

async fn start_resumable(
    backend: &GcsBackend,
    name: &str,
    content_type: &str,
    metadata: &BTreeMap<String, String>,
    total_len: Option<u64>,
) -> Result<String, GcsError> {
    let url = format!(
        "{UPLOAD_ROOT}/b/{}/o?uploadType=resumable",
        encode_seg(bucket(backend)),
    );
    let mut body = serde_json::json!({
        "name": name,
        "contentType": content_type,
    });
    if !metadata.is_empty() {
        body["metadata"] = serde_json::to_value(metadata).map_err(|err| {
            GcsError::with_source(GcsErrorKind::Config, "metadata is not JSON", err)
        })?;
    }
    let mut headers = vec![(
        reqwest::header::HeaderName::from_static("x-upload-content-type"),
        content_type.to_string(),
    )];
    if let Some(len) = total_len {
        headers.push((
            reqwest::header::HeaderName::from_static("x-upload-content-length"),
            len.to_string(),
        ));
    }
    let response = send(
        backend,
        &Call {
            method: reqwest::Method::POST,
            url,
            headers,
            json: Some(body),
            bytes: None,
            extra_status: &[],
            timeout: None,
        },
    )
    .await?;
    let location = response
        .headers()
        .get(reqwest::header::LOCATION)
        .ok_or_else(|| {
            GcsError::new(
                GcsErrorKind::Api,
                "resumable upload did not return Location",
            )
        })?;
    let location = location
        .to_str()
        .map_err(|_| GcsError::new(GcsErrorKind::Api, "resumable Location header is not UTF-8"))?
        .to_string();
    let _ = response.bytes().await;
    if !location.starts_with("https://") {
        return Err(GcsError::new(
            GcsErrorKind::Api,
            "resumable Location is not an https URL",
        ));
    }
    Ok(location)
}

fn expect_done(step: UploadStep) -> Result<ObjectMeta, GcsError> {
    match step {
        UploadStep::Done(meta) => Ok(meta),
        UploadStep::Continue { .. } => Err(GcsError::new(
            GcsErrorKind::Api,
            "finished upload did not return object metadata",
        )),
    }
}

enum UploadStep {
    Done(ObjectMeta),
    Continue { next: u64 },
}

enum Sent {
    Done(ObjectMeta),
    Accepted { next: u64 },
}

enum Probe {
    Complete(ObjectMeta),
    Incomplete { next: u64 },
}

fn retry_delay(attempt: u32) -> Duration {
    CHUNK_BACKOFF.saturating_mul(u32::max(attempt, 1))
}

fn advance_offset(
    offset: &mut u64,
    rewinds: &mut u32,
    next: u64,
    total: u64,
) -> Result<(), GcsError> {
    if next > total {
        return Err(GcsError::new(
            GcsErrorKind::Api,
            "resumable offset is past the end of the object",
        ));
    }
    if next > *offset {
        *rewinds = 0;
        *offset = next;
        return Ok(());
    }
    if next < *offset {
        *rewinds += 1;
        if *rewinds > CHUNK_ATTEMPTS {
            return Err(GcsError::new(
                GcsErrorKind::Transport,
                "resumable session did not keep uploaded bytes",
            ));
        }
        *offset = next;
        return Ok(());
    }
    Err(GcsError::new(
        GcsErrorKind::Api,
        "resumable upload did not advance",
    ))
}

fn chunk_suffix_len(chunk_len: u64, start: u64, offset: u64, finished: bool) -> Option<u64> {
    if offset < start || offset > start + chunk_len {
        return None;
    }
    let suffix = start + chunk_len - offset;
    if suffix == 0 {
        if finished && chunk_len == 0 && offset == start {
            return Some(0);
        }
        return None;
    }
    if finished || suffix.is_multiple_of(CHUNK_ALIGN) {
        return Some(suffix);
    }
    None
}

fn chunk_content_range(start: u64, len: u64, total: Option<u64>) -> String {
    if len == 0 {
        return format!("bytes */{}", total.unwrap_or(0));
    }
    let end = start + len - 1;
    match total {
        Some(total) => format!("bytes {start}-{end}/{total}"),
        None => format!("bytes {start}-{end}/*"),
    }
}

fn probe_content_range(total: Option<u64>) -> String {
    match total {
        Some(total) => format!("bytes */{total}"),
        None => "bytes */*".to_string(),
    }
}

fn next_offset_from_range(header: &str) -> Result<u64, GcsError> {
    let header = header.trim();
    let rest = header
        .strip_prefix("bytes=")
        .ok_or_else(|| GcsError::new(GcsErrorKind::Api, "resumable Range header is invalid"))?;
    let rest = rest.split(',').next().unwrap_or(rest).trim();
    let (start, end) = rest
        .split_once('-')
        .ok_or_else(|| GcsError::new(GcsErrorKind::Api, "resumable Range header is invalid"))?;
    let start: u64 = start
        .trim()
        .parse()
        .map_err(|_| GcsError::new(GcsErrorKind::Api, "resumable Range header is invalid"))?;
    let end: u64 = end
        .trim()
        .parse()
        .map_err(|_| GcsError::new(GcsErrorKind::Api, "resumable Range header is invalid"))?;
    if end < start {
        return Err(GcsError::new(
            GcsErrorKind::Api,
            "resumable Range header is invalid",
        ));
    }
    end.checked_add(1)
        .ok_or_else(|| GcsError::new(GcsErrorKind::Api, "resumable Range header is invalid"))
}

async fn put_chunk_retrying(
    backend: &GcsBackend,
    location: &str,
    content_type: &str,
    chunk: &[u8],
    start: u64,
    total: Option<u64>,
    finished: bool,
) -> Result<UploadStep, GcsError> {
    let chunk_len = chunk.len() as u64;
    let mut offset = start;
    let mut attempt = 0u32;
    loop {
        if chunk_suffix_len(chunk_len, start, offset, finished).is_none() {
            return Ok(UploadStep::Continue { next: offset });
        }
        attempt += 1;
        if attempt > CHUNK_ATTEMPTS {
            return Err(GcsError::new(
                GcsErrorKind::Transport,
                "resumable chunk failed after retries",
            ));
        }
        if attempt > 1 {
            tokio::time::sleep(retry_delay(attempt - 1)).await;
        }
        let piece = &chunk[(offset - start) as usize..];
        let sent = upload_chunk_once(backend, location, content_type, piece, offset, total).await;
        match sent {
            Ok(Sent::Done(meta)) => return Ok(UploadStep::Done(meta)),
            Ok(Sent::Accepted { next }) => {
                if next >= start + chunk_len {
                    return Ok(UploadStep::Continue { next });
                }
                if next > offset {
                    offset = next;
                    attempt = 0;
                    continue;
                }
            }
            Err(err) if !is_retryable(&err) => return Err(err),
            Err(err) => {
                let attempt_text = attempt.to_string();
                let offset_text = offset.to_string();
                let error_text = super::ops::logged_error(&err);
                super::ops::record_op(
                    backend,
                    super::ops::OpsRecord {
                        level: crate::logging::LogLevel::Warn,
                        message: "gcs resumable chunk failed",
                        method: "PUT",
                        url: location,
                        status: err.status(),
                        latency_ms: None,
                        extra: &[
                            ("attempt", &attempt_text),
                            ("offset", &offset_text),
                            ("error", &error_text),
                        ],
                    },
                );
            }
        }
        match probe_session(backend, location, total).await {
            Ok(Probe::Complete(meta)) => return Ok(UploadStep::Done(meta)),
            Ok(Probe::Incomplete { next }) => {
                let attempt_text = attempt.to_string();
                let offset_text = offset.to_string();
                let next_text = next.to_string();
                super::ops::record_op(
                    backend,
                    super::ops::OpsRecord {
                        level: crate::logging::LogLevel::Debug,
                        message: "gcs resumable probe",
                        method: "PUT",
                        url: location,
                        status: None,
                        latency_ms: None,
                        extra: &[
                            ("attempt", &attempt_text),
                            ("offset", &offset_text),
                            ("next", &next_text),
                        ],
                    },
                );
                if next < start || next >= start + chunk_len {
                    return Ok(UploadStep::Continue { next });
                }
                if next > offset {
                    offset = next;
                    attempt = 0;
                    continue;
                }
                if next < offset {
                    offset = next;
                }
            }
            Err(err) if !is_retryable(&err) => return Err(err),
            Err(err) => {
                let attempt_text = attempt.to_string();
                let offset_text = offset.to_string();
                let error_text = super::ops::logged_error(&err);
                super::ops::record_op(
                    backend,
                    super::ops::OpsRecord {
                        level: crate::logging::LogLevel::Warn,
                        message: "gcs resumable probe failed",
                        method: "PUT",
                        url: location,
                        status: err.status(),
                        latency_ms: None,
                        extra: &[
                            ("attempt", &attempt_text),
                            ("offset", &offset_text),
                            ("error", &error_text),
                        ],
                    },
                );
            }
        }
    }
}

async fn upload_chunk_once(
    backend: &GcsBackend,
    location: &str,
    content_type: &str,
    chunk: &[u8],
    offset: u64,
    total: Option<u64>,
) -> Result<Sent, GcsError> {
    let range = chunk_content_range(offset, chunk.len() as u64, total);
    let response = send(
        backend,
        &Call {
            method: reqwest::Method::PUT,
            url: location.to_string(),
            headers: vec![
                (reqwest::header::CONTENT_TYPE, content_type.to_string()),
                (reqwest::header::CONTENT_RANGE, range),
            ],
            json: None,
            bytes: Some(chunk.to_vec()),
            extra_status: &[308],
            timeout: Some(TRANSFER_TIMEOUT),
        },
    )
    .await?;
    if response.status().is_success() {
        return Ok(Sent::Done(meta_from_response(response).await?));
    }
    let header = response
        .headers()
        .get(reqwest::header::RANGE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let _ = response.bytes().await;
    let next = match header {
        Some(value) => next_offset_from_range(&value)?,
        None => offset + chunk.len() as u64,
    };
    Ok(Sent::Accepted { next })
}

async fn probe_session(
    backend: &GcsBackend,
    location: &str,
    total: Option<u64>,
) -> Result<Probe, GcsError> {
    let response = send(
        backend,
        &Call {
            method: reqwest::Method::PUT,
            url: location.to_string(),
            headers: vec![
                (reqwest::header::CONTENT_RANGE, probe_content_range(total)),
                (reqwest::header::CONTENT_LENGTH, "0".to_string()),
            ],
            json: None,
            bytes: Some(Vec::new()),
            extra_status: &[308],
            timeout: Some(TRANSFER_TIMEOUT),
        },
    )
    .await?;
    if response.status().is_success() {
        return Ok(Probe::Complete(meta_from_response(response).await?));
    }
    let header = response
        .headers()
        .get(reqwest::header::RANGE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let _ = response.bytes().await;
    let next = match header {
        Some(value) => next_offset_from_range(&value)?,
        None => 0,
    };
    Ok(Probe::Incomplete { next })
}

async fn meta_from_response(response: reqwest::Response) -> Result<ObjectMeta, GcsError> {
    let bytes = response.bytes().await.map_err(|err| {
        GcsError::with_source(GcsErrorKind::Transport, "object response is not bytes", err)
    })?;
    object_meta_from_slice(&bytes)
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

struct RewriteStep {
    done: bool,
    rewrite_token: Option<String>,
    resource: Option<ObjectMeta>,
}

fn rewrite_step_from_slice(bytes: &[u8]) -> Result<RewriteStep, GcsError> {
    let raw: RawRewrite = serde_json::from_slice(bytes)
        .map_err(|err| GcsError::with_source(GcsErrorKind::Api, "rewrite JSON is invalid", err))?;
    let resource = match raw.resource {
        Some(object) => Some(object.into_meta()?),
        None => None,
    };
    if raw.done && resource.is_none() {
        return Err(GcsError::new(
            GcsErrorKind::Api,
            "rewrite did not return object metadata",
        ));
    }
    if !raw.done {
        match raw.rewrite_token.as_deref() {
            Some(token) if !token.is_empty() => {}
            _ => {
                return Err(GcsError::new(
                    GcsErrorKind::Api,
                    "rewrite did not return a token",
                ));
            }
        }
    }
    Ok(RewriteStep {
        done: raw.done,
        rewrite_token: raw.rewrite_token.filter(|token| !token.is_empty()),
        resource,
    })
}

fn object_meta_from_slice(bytes: &[u8]) -> Result<ObjectMeta, GcsError> {
    let raw: RawObject = serde_json::from_slice(bytes)
        .map_err(|err| GcsError::with_source(GcsErrorKind::Api, "object JSON is invalid", err))?;
    raw.into_meta()
}

fn list_page_from_slice(bytes: &[u8]) -> Result<ListPage, GcsError> {
    let raw: RawList = serde_json::from_slice(bytes)
        .map_err(|err| GcsError::with_source(GcsErrorKind::Api, "list JSON is invalid", err))?;
    let mut objects = Vec::with_capacity(raw.items.len());
    for item in raw.items {
        objects.push(item.into_meta()?);
    }
    Ok(ListPage {
        objects,
        prefixes: raw.prefixes,
        next_page_token: raw.next_page_token.filter(|token| !token.is_empty()),
    })
}

#[derive(Debug, Deserialize)]
struct RawObject {
    name: String,
    #[serde(default, deserialize_with = "de_size")]
    size: Option<u64>,
    #[serde(rename = "contentType", default)]
    content_type: Option<String>,
    #[serde(default)]
    updated: Option<String>,
    #[serde(default, deserialize_with = "de_metadata")]
    metadata: BTreeMap<String, String>,
}

impl RawObject {
    fn into_meta(self) -> Result<ObjectMeta, GcsError> {
        let updated = match self.updated {
            Some(text) => Some(
                DateTime::parse_from_rfc3339(&text)
                    .map_err(|err| {
                        GcsError::with_source(
                            GcsErrorKind::Api,
                            "object updated time is invalid",
                            err,
                        )
                    })?
                    .with_timezone(&Utc),
            ),
            None => None,
        };
        Ok(ObjectMeta {
            name: self.name,
            size: self.size.unwrap_or(0),
            content_type: self.content_type,
            updated,
            metadata: self.metadata,
        })
    }
}

#[derive(Debug, Deserialize)]
struct RawRewrite {
    #[serde(default)]
    done: bool,
    #[serde(rename = "rewriteToken", default)]
    rewrite_token: Option<String>,
    #[serde(default)]
    resource: Option<RawObject>,
}

#[derive(Debug, Deserialize)]
struct RawList {
    #[serde(default)]
    items: Vec<RawObject>,
    #[serde(default)]
    prefixes: Vec<String>,
    #[serde(rename = "nextPageToken", default)]
    next_page_token: Option<String>,
}

fn de_size<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct Visit;
    impl<'de> serde::de::Visitor<'de> for Visit {
        type Value = Option<u64>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a string or integer size")
        }

        fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            deserializer.deserialize_any(Visit)
        }

        fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<Self::Value, E> {
            Ok(Some(value))
        }

        fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<Self::Value, E> {
            u64::try_from(value).map(Some).map_err(E::custom)
        }

        fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
            if value.is_empty() {
                return Ok(None);
            }
            value.parse::<u64>().map(Some).map_err(E::custom)
        }
    }
    deserializer.deserialize_any(Visit)
}

fn de_metadata<'de, D>(deserializer: D) -> Result<BTreeMap<String, String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<BTreeMap<String, String>>::deserialize(deserializer)?;
    Ok(value.unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_json_keeps_caller_metadata() {
        let json = br#"{
            "name": "photos/cat.jpeg",
            "size": "2048",
            "contentType": "image/jpeg",
            "updated": "2020-01-02T03:04:05.678Z",
            "metadata": {"uploaded-by": "ian", "uploaded-by-id": "7"}
        }"#;
        let meta = object_meta_from_slice(json).unwrap();
        assert_eq!(meta.name, "photos/cat.jpeg");
        assert_eq!(meta.size, 2048);
        assert_eq!(meta.content_type.as_deref(), Some("image/jpeg"));
        assert_eq!(
            meta.updated,
            Some(
                DateTime::parse_from_rfc3339("2020-01-02T03:04:05.678Z")
                    .unwrap()
                    .with_timezone(&Utc)
            )
        );
        assert_eq!(
            meta.metadata.get("uploaded-by").map(String::as_str),
            Some("ian")
        );
        assert_eq!(
            meta.metadata.get("uploaded-by-id").map(String::as_str),
            Some("7")
        );
    }

    #[test]
    fn missing_metadata_is_an_empty_map() {
        let json = br#"{"name":"a","size":1}"#;
        let meta = object_meta_from_slice(json).unwrap();
        assert!(meta.metadata.is_empty());
        assert_eq!(meta.size, 1);
        assert_eq!(meta.updated, None);
    }

    #[test]
    fn list_keeps_trash_prefixes() {
        let json = br#"{
            "items": [{"name":"keep.txt","size":"3","metadata":null}],
            "prefixes": [".trash/", "photos/"],
            "nextPageToken": "next"
        }"#;
        let page = list_page_from_slice(json).unwrap();
        assert_eq!(
            page.prefixes,
            vec![".trash/".to_string(), "photos/".to_string()]
        );
        assert_eq!(page.objects.len(), 1);
        assert!(page.objects[0].metadata.is_empty());
        assert_eq!(page.next_page_token.as_deref(), Some("next"));
    }

    #[test]
    fn list_url_encodes_query_and_does_not_add_a_cap() {
        let url = list_url("buck", &ListQuery::default());
        assert_eq!(url, "https://storage.googleapis.com/storage/v1/b/buck/o");
        let url = list_url(
            "buck",
            &ListQuery {
                prefix: Some("a b".to_string()),
                delimiter: Some("/".to_string()),
                page_token: None,
                max_results: Some(10),
            },
        );
        assert_eq!(
            url,
            "https://storage.googleapis.com/storage/v1/b/buck/o?delimiter=%2F&maxResults=10&prefix=a%20b"
        );
    }

    #[test]
    fn object_url_encodes_slashes() {
        assert_eq!(
            object_url("buck", "a/c"),
            "https://storage.googleapis.com/storage/v1/b/buck/o/a%2Fc"
        );
    }

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

    #[test]
    fn put_options_do_not_insert_product_keys() {
        let options = PutOptions::default();
        assert!(options.metadata.is_empty());
        assert_eq!(
            content_type_of(&options).unwrap(),
            "application/octet-stream"
        );
    }

    #[test]
    fn probe_and_chunk_ranges_match_the_upload_shape() {
        assert_eq!(probe_content_range(Some(0)), "bytes */0");
        assert_eq!(probe_content_range(Some(12)), "bytes */12");
        assert_eq!(probe_content_range(None), "bytes */*");
        assert_eq!(chunk_content_range(0, 0, Some(0)), "bytes */0");
        assert_eq!(chunk_content_range(0, 4, Some(4)), "bytes 0-3/4");
        assert_eq!(chunk_content_range(8, 4, None), "bytes 8-11/*");
    }

    #[test]
    fn resume_range_is_the_end_plus_one() {
        assert_eq!(next_offset_from_range("bytes=0-42").unwrap(), 43);
        assert_eq!(next_offset_from_range("bytes=0-0").unwrap(), 1);
        assert!(next_offset_from_range("bytes=*").is_err());
        assert!(next_offset_from_range("0-1").is_err());
    }

    #[test]
    fn non_final_suffix_must_be_a_multiple_of_256_kib() {
        let align = CHUNK_ALIGN;
        assert_eq!(chunk_suffix_len(align, 0, 0, false), Some(align));
        assert_eq!(chunk_suffix_len(align, 0, 1, false), None);
        assert_eq!(chunk_suffix_len(align, 0, 1, true), Some(align - 1));
        assert_eq!(chunk_suffix_len(0, 0, 0, true), Some(0));
        assert_eq!(chunk_suffix_len(align, 0, align, false), None);
    }

    #[test]
    fn advance_offset_rewinds_then_resets_after_progress() {
        let mut offset = 10u64;
        let mut rewinds = 0u32;
        advance_offset(&mut offset, &mut rewinds, 4, 100).unwrap();
        assert_eq!(offset, 4);
        assert_eq!(rewinds, 1);
        let stalled = advance_offset(&mut offset, &mut rewinds, 4, 100).unwrap_err();
        assert_eq!(stalled.kind(), GcsErrorKind::Api);
        advance_offset(&mut offset, &mut rewinds, 20, 100).unwrap();
        assert_eq!(offset, 20);
        assert_eq!(rewinds, 0);
        let past = advance_offset(&mut offset, &mut rewinds, 101, 100).unwrap_err();
        assert_eq!(past.kind(), GcsErrorKind::Api);
    }

    #[test]
    fn advance_offset_stops_after_repeated_rewinds() {
        let mut offset = 8u64;
        let mut rewinds = 0u32;
        for _ in 0..CHUNK_ATTEMPTS {
            advance_offset(&mut offset, &mut rewinds, 0, 100).unwrap();
            offset = 8;
        }
        let err = advance_offset(&mut offset, &mut rewinds, 0, 100).unwrap_err();
        assert_eq!(err.kind(), GcsErrorKind::Transport);
    }

    #[test]
    fn rewrite_step_reads_a_token_until_done() {
        let step = rewrite_step_from_slice(br#"{"done":false,"rewriteToken":"abc"}"#).unwrap();
        assert!(!step.done);
        assert_eq!(step.rewrite_token.as_deref(), Some("abc"));
        assert!(step.resource.is_none());
        let done = rewrite_step_from_slice(br#"{"done":true,"resource":{"name":"b","size":"3"}}"#)
            .unwrap();
        assert!(done.done);
        let resource = done.resource.unwrap();
        assert_eq!(resource.name, "b");
        assert_eq!(resource.size, 3);
        assert!(rewrite_step_from_slice(br#"{"done":false}"#).is_err());
        assert!(rewrite_step_from_slice(br#"{"done":true}"#).is_err());
        assert!(rewrite_step_from_slice(br#"{"done":false,"rewriteToken":""}"#).is_err());
    }

    #[test]
    fn rewrite_url_sets_the_token_query() {
        assert_eq!(
            rewrite_url("buck", "a/b", "c d", None),
            "https://storage.googleapis.com/storage/v1/b/buck/o/a%2Fb/rewriteTo/b/buck/o/c%20d"
        );
        let url = rewrite_url("buck", "a", "c", Some("tok/en"));
        assert!(url.ends_with("?rewriteToken=tok%2Fen"));
    }

    #[test]
    fn chunk_retry_uses_the_transfer_timeout() {
        assert_eq!(TRANSFER_TIMEOUT, Duration::from_secs(180));
        assert_eq!(retry_delay(1), Duration::from_millis(200));
        assert_eq!(retry_delay(3), Duration::from_millis(600));
    }
}
