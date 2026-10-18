//! JSON API object calls.
//!
//! Results keep `name`, `size`, `content_type`, `updated`, and the raw metadata map.
//! List pages also keep `prefixes` and `next_page_token`.
//! This module does not hide names and it does not apply a page cap.

use std::collections::BTreeMap;
use std::path::Path;
use std::pin::Pin;

use chrono::{DateTime, Utc};
use futures_util::{Stream, StreamExt};
use serde::Deserialize;

use super::encode::{encode_object_id, encode_seg};
use super::{bucket, send, Call, GcsBackend, GcsError, GcsErrorKind};

const DEFAULT_CONTENT_TYPE: &str = "application/octet-stream";
const CHUNK_SIZE: usize = 8 * 1024 * 1024;
const API_ROOT: &str = "https://storage.googleapis.com/storage/v1";
const UPLOAD_ROOT: &str = "https://storage.googleapis.com/upload/storage/v1";

/// Custom metadata and content type for an upload.
///
/// `metadata` is stored as given. This crate does not insert `uploaded-by`
/// or `uploaded-by-id`. A project that wants those keys sets them here.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PutOptions {
    /// Value stored as `contentType`. `None` stores `application/octet-stream`.
    pub content_type: Option<String>,
    pub metadata: BTreeMap<String, String>,
}

/// One object, without product file or folder types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectMeta {
    pub name: String,
    pub size: u64,
    pub content_type: Option<String>,
    pub updated: Option<DateTime<Utc>>,
    pub metadata: BTreeMap<String, String>,
}

/// One page from `objects.list`.
///
/// `prefixes` is the raw delimiter list from GCS. Nothing is filtered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListPage {
    pub objects: Vec<ObjectMeta>,
    pub prefixes: Vec<String>,
    pub next_page_token: Option<String>,
}

/// Query for one list call.
///
/// `max_results` is sent as `maxResults` when set. This crate does not pick a default cap.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListQuery {
    pub prefix: Option<String>,
    pub delimiter: Option<String>,
    pub page_token: Option<String>,
    pub max_results: Option<u32>,
}

/// Open object body from `alt=media`.
pub struct ObjectReader {
    response: reqwest::Response,
}

/// Upload bytes with a resumable session.
pub async fn put_bytes(
    backend: &GcsBackend,
    name: &str,
    bytes: &[u8],
    options: &PutOptions,
) -> Result<ObjectMeta, GcsError> {
    validate_object_name(name)?;
    let content_type = content_type_of(options)?;
    validate_metadata(&options.metadata)?;
    let location = start_resumable(
        backend,
        name,
        &content_type,
        &options.metadata,
        Some(bytes.len() as u64),
    )
    .await?;
    if bytes.is_empty() {
        return finished(
            upload_chunk(backend, &location, &content_type, b"", "bytes */0", true).await?,
        );
    }
    let end = bytes.len() - 1;
    let range = format!("bytes 0-{end}/{}", bytes.len());
    finished(upload_chunk(backend, &location, &content_type, bytes, &range, true).await?)
}

/// Upload a file. The content type comes from `options`, not from the file name.
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
        return finished(
            upload_chunk(backend, &location, &content_type, b"", "bytes */0", true).await?,
        );
    }
    let mut sent = 0u64;
    loop {
        let remaining = total - sent;
        let want = usize::try_from(remaining.min(CHUNK_SIZE as u64)).unwrap_or(CHUNK_SIZE);
        let chunk = read_exact_n(&mut file, want).await?;
        let is_last = sent + chunk.len() as u64 == total;
        let end = sent + chunk.len() as u64 - 1;
        let range = format!("bytes {sent}-{end}/{total}");
        let meta = upload_chunk(backend, &location, &content_type, &chunk, &range, is_last).await?;
        sent += chunk.len() as u64;
        if is_last {
            return finished(meta);
        }
    }
}

/// Upload a stream of byte chunks.
///
/// The length does not need to be known. Chunks sent before the last one are 8 MiB.
/// The last chunk can be shorter.
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
    let mut current = pull_chunk(&mut stream, &mut pending, CHUNK_SIZE).await?;
    if current.is_empty() {
        return finished(
            upload_chunk(backend, &location, &content_type, b"", "bytes */0", true).await?,
        );
    }
    let mut sent = 0u64;
    loop {
        let next = pull_chunk(&mut stream, &mut pending, CHUNK_SIZE).await?;
        let is_last = next.is_empty();
        let end = sent + current.len() as u64 - 1;
        if !is_last && current.len() % (256 * 1024) != 0 {
            return Err(GcsError::new(
                GcsErrorKind::Api,
                "resumable chunk is not a multiple of 256 KiB",
            ));
        }
        let range = if is_last {
            let total = sent + current.len() as u64;
            format!("bytes {sent}-{end}/{total}")
        } else {
            format!("bytes {sent}-{end}/*")
        };
        let meta =
            upload_chunk(backend, &location, &content_type, &current, &range, is_last).await?;
        sent += current.len() as u64;
        if is_last {
            return finished(meta);
        }
        current = next;
    }
}

/// Download the object body into memory.
///
/// For a large object, use [`open_object`] and [`byte_stream`] instead.
pub async fn get_bytes(backend: &GcsBackend, name: &str) -> Result<Vec<u8>, GcsError> {
    read_to_vec(open_object(backend, name).await?).await
}

/// Open the object body without buffering it.
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
        },
    )
    .await?;
    Ok(ObjectReader { response })
}

/// Read an open object into memory.
pub async fn read_to_vec(reader: ObjectReader) -> Result<Vec<u8>, GcsError> {
    let bytes = reader.response.bytes().await.map_err(|err| {
        GcsError::with_source(GcsErrorKind::Transport, "read object body failed", err)
    })?;
    Ok(bytes.to_vec())
}

/// Stream an open object as `Vec<u8>` chunks.
pub fn byte_stream(
    reader: ObjectReader,
) -> Pin<Box<dyn Stream<Item = Result<Vec<u8>, GcsError>> + Send>> {
    Box::pin(reader.response.bytes_stream().map(|item| {
        item.map(|chunk| chunk.to_vec()).map_err(|err| {
            GcsError::with_source(GcsErrorKind::Transport, "read object body failed", err)
        })
    }))
}

/// Read object metadata.
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
        },
    )
    .await?;
    meta_from_response(response).await
}

/// List one page. Follow `next_page_token` to continue. No cap is applied here.
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
        },
    )
    .await?;
    let bytes = response.bytes().await.map_err(|err| {
        GcsError::with_source(GcsErrorKind::Transport, "list response is not bytes", err)
    })?;
    list_page_from_slice(&bytes)
}

/// Delete one object. A missing object is [`GcsErrorKind::NotFound`].
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
        },
    )
    .await?;
    let _ = response.bytes().await;
    Ok(())
}

/// Patch custom metadata. GCS merges the keys in `metadata`.
///
/// This does not rewrite `uploaded-by` on its own. Pass the keys you want to set.
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
        },
    )
    .await?;
    meta_from_response(response).await
}

/// Copy one object to another name in the same bucket.
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
        },
    )
    .await?;
    meta_from_response(response).await
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

fn finished(meta: Option<ObjectMeta>) -> Result<ObjectMeta, GcsError> {
    meta.ok_or_else(|| {
        GcsError::new(
            GcsErrorKind::Api,
            "finished upload did not return object metadata",
        )
    })
}

async fn upload_chunk(
    backend: &GcsBackend,
    location: &str,
    content_type: &str,
    chunk: &[u8],
    range: &str,
    finished: bool,
) -> Result<Option<ObjectMeta>, GcsError> {
    let response = send(
        backend,
        &Call {
            method: reqwest::Method::PUT,
            url: location.to_string(),
            headers: vec![
                (reqwest::header::CONTENT_TYPE, content_type.to_string()),
                (reqwest::header::CONTENT_RANGE, range.to_string()),
            ],
            json: None,
            bytes: Some(chunk.to_vec()),
            extra_status: if finished { &[] } else { &[308] },
        },
    )
    .await?;
    if !finished {
        let _ = response.bytes().await;
        return Ok(None);
    }
    meta_from_response(response).await.map(Some)
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
}
