pub(crate) mod list;
pub(crate) mod object;
pub(crate) mod ops_log;
pub(crate) mod put_request;
pub(crate) mod signed_url;

use std::collections::BTreeMap;
use std::fmt;
use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use google_cloud_auth::credentials;
use google_cloud_auth::signer::Signer;
use google_cloud_storage::builder::storage::SignedUrlBuilder;
use google_cloud_storage::client::{Storage, StorageControl};
use google_cloud_storage::http::Method;
use google_cloud_storage::model::Object;
use google_cloud_wkt::FieldMask;

use crate::exception;
use crate::exception::Exception;
use crate::gcp::error::to_exception;
use crate::gcp::storage::ops_log::OpsRecord;
use crate::gcp::storage::signed_url::{content_disposition_value, validate_lifetime};
use crate::logging::Logger;

pub use crate::gcp::storage::list::{ListPage, ListQuery};
pub use crate::gcp::storage::object::{ObjectMeta, ObjectReader};
pub use crate::gcp::storage::put_request::{PutBody, PutOptions, PutRequest};

pub const BUCKET_NAME_INVALID: &str = "BUCKET_NAME_INVALID";
pub const CLIENT_BUILD_FAILED: &str = "CLIENT_BUILD_FAILED";
pub const FILE_METADATA_FAILED: &str = "FILE_METADATA_FAILED";
pub const FILE_OPEN_FAILED: &str = "FILE_OPEN_FAILED";
pub const METADATA_INVALID: &str = "METADATA_INVALID";
pub const MOVE_SAME_OBJECT: &str = "MOVE_SAME_OBJECT";
pub const NOT_A_FILE: &str = "NOT_A_FILE";
pub const OBJECT_NAME_INVALID: &str = "OBJECT_NAME_INVALID";
pub const RESOURCE_NOT_FOUND: &str = "RESOURCE_NOT_FOUND";
pub const REWRITE_NO_OBJECT: &str = "REWRITE_NO_OBJECT";
pub const REWRITE_STEP_LIMIT: &str = "REWRITE_STEP_LIMIT";
pub const REWRITE_TOKEN_STUCK: &str = "REWRITE_TOKEN_STUCK";
pub const SIGNER_EMAIL_INVALID: &str = "SIGNER_EMAIL_INVALID";
pub const SIGN_URL_FAILED: &str = "SIGN_URL_FAILED";
pub const URL_SIGNER_BUILD_FAILED: &str = "URL_SIGNER_BUILD_FAILED";

// The most rewrite calls for one object. A rewrite that needs more is a fault.
const MAX_REWRITE_STEPS: u32 = 10_000;
// GCS refuses a longer object name.
pub(crate) const MAX_OBJECT_NAME_BYTES: usize = 1024;

#[derive(Clone)]
pub struct GcsClient {
    inner: Arc<Inner>,
}

impl GcsClient {
    pub fn bucket(&self) -> &str {
        &self.inner.bucket
    }

    // `signer_email` is the service account that signs URLs. Without it, the signer is the
    // account of the credentials, which must be a service account or the metadata server.
    pub async fn from_adc(
        bucket_name: impl Into<String>,
        signer_email: Option<&str>,
    ) -> Result<Self, Exception> {
        let bucket = bucket_name.into();
        validate_bucket(&bucket)?;
        let storage = Storage::builder().build().await.map_err(|err| {
            exception!(
                code = CLIENT_BUILD_FAILED,
                "cannot build the client",
                source = err
            )
        })?;
        let control = StorageControl::builder().build().await.map_err(|err| {
            exception!(
                code = CLIENT_BUILD_FAILED,
                "cannot build the client",
                source = err
            )
        })?;
        Ok(Self {
            inner: Arc::new(Inner {
                bucket_path: format!("projects/_/buckets/{bucket}"),
                bucket,
                storage,
                control,
                signer: build_signer(signer_email)?,
                ops_log: std::sync::Mutex::new(None),
            }),
        })
    }

    pub async fn get_bytes(&self, name: &str) -> Result<Vec<u8>, Exception> {
        self.open_object(name).await?.read_to_vec().await
    }

    pub async fn get_bytes_opt(&self, name: &str) -> Result<Option<Vec<u8>>, Exception> {
        not_found_as_none(self.get_bytes(name).await)
    }

    pub async fn open_object(&self, name: &str) -> Result<ObjectReader, Exception> {
        validate_object_name(name)?;
        self.run_logged("open", name, async {
            let response = self
                .inner
                .storage
                .read_object(&self.inner.bucket_path, name)
                .send()
                .await
                .map_err(|err| to_exception("open object failed", err))?;
            Ok(ObjectReader { response })
        })
        .await
    }

    pub async fn open_object_opt(&self, name: &str) -> Result<Option<ObjectReader>, Exception> {
        not_found_as_none(self.open_object(name).await)
    }

    pub async fn stat(&self, name: &str) -> Result<ObjectMeta, Exception> {
        validate_object_name(name)?;
        self.run_logged("stat", name, async {
            let object = self
                .inner
                .control
                .get_object()
                .set_bucket(&self.inner.bucket_path)
                .set_object(name)
                .send()
                .await
                .map_err(|err| to_exception("stat object failed", err))?;
            Ok(ObjectMeta::from(object))
        })
        .await
    }

    pub async fn stat_opt(&self, name: &str) -> Result<Option<ObjectMeta>, Exception> {
        not_found_as_none(self.stat(name).await)
    }

    pub async fn list(&self, query: &ListQuery) -> Result<ListPage, Exception> {
        let prefix = query.prefix.as_deref().unwrap_or_default();
        self.run_logged("list", prefix, async {
            let mut request = self
                .inner
                .control
                .list_objects()
                .set_parent(&self.inner.bucket_path);
            if let Some(prefix) = &query.prefix {
                request = request.set_prefix(prefix);
            }
            if let Some(delimiter) = &query.delimiter {
                request = request.set_delimiter(delimiter);
            }
            if let Some(token) = &query.page_token {
                request = request.set_page_token(token);
            }
            if let Some(max) = query.max_results {
                request = request.set_page_size(i32::try_from(max).unwrap_or(i32::MAX));
            }
            let response = request
                .send()
                .await
                .map_err(|err| to_exception("list objects failed", err))?;
            Ok(ListPage::from_response(response))
        })
        .await
    }

    pub async fn put(&self, request: &PutRequest) -> Result<ObjectMeta, Exception> {
        let name = request.name.as_str();
        let options = &request.options;
        validate_object_name(name)?;
        let content_type = options.content_type_or_default()?;
        validate_metadata(&options.metadata)?;
        let metadata = options.metadata.clone();
        let bucket_path = &self.inner.bucket_path;
        self.run_logged("put", name, async {
            let result = match &request.body {
                PutBody::Bytes(bytes) => {
                    self.inner
                        .storage
                        .write_object(bucket_path, name, Bytes::copy_from_slice(bytes))
                        .set_content_type(content_type)
                        .set_metadata(metadata)
                        .send_unbuffered()
                        .await
                }
                PutBody::File(path) => {
                    let file = open_upload_file(path).await?;
                    self.inner
                        .storage
                        .write_object(bucket_path, name, file)
                        .set_content_type(content_type)
                        .set_metadata(metadata)
                        .send_unbuffered()
                        .await
                }
            };
            let object = result.map_err(|err| to_exception("put object failed", err))?;
            Ok(ObjectMeta::from(object))
        })
        .await
    }

    pub async fn rewrite_object(&self, from: &str, to: &str) -> Result<ObjectMeta, Exception> {
        validate_object_name(from)?;
        validate_object_name(to)?;
        self.run_logged("rewrite", from, async {
            let mut token = String::new();
            for _ in 0..MAX_REWRITE_STEPS {
                let step = self
                    .inner
                    .control
                    .rewrite_object()
                    .set_source_bucket(&self.inner.bucket_path)
                    .set_source_object(from)
                    .set_destination_bucket(&self.inner.bucket_path)
                    .set_destination_name(to)
                    .set_rewrite_token(&token)
                    .send()
                    .await
                    .map_err(|err| to_exception("rewrite object failed", err))?;
                if !step.done {
                    if step.rewrite_token == token {
                        return Err(exception!(
                            code = REWRITE_TOKEN_STUCK,
                            "rewrite token did not change"
                        ));
                    }
                    token = step.rewrite_token;
                    continue;
                }
                return match step.resource {
                    Some(object) => Ok(ObjectMeta::from(object)),
                    None => Err(exception!(
                        code = REWRITE_NO_OBJECT,
                        "rewrite gave no object"
                    )),
                };
            }
            Err(exception!(
                code = REWRITE_STEP_LIMIT,
                "rewrite did not finish within the step limit",
            ))
        })
        .await
    }

    // The new keys replace the keys with the same name. The other keys stay.
    pub async fn patch_metadata(
        &self,
        name: &str,
        metadata: &BTreeMap<String, String>,
    ) -> Result<ObjectMeta, Exception> {
        validate_object_name(name)?;
        validate_metadata(metadata)?;
        self.run_logged("patch", name, async {
            let current = self
                .inner
                .control
                .get_object()
                .set_bucket(&self.inner.bucket_path)
                .set_object(name)
                .send()
                .await
                .map_err(|err| to_exception("read object metadata failed", err))?;
            let mut merged = current.metadata;
            merged.extend(metadata.clone());
            let object = Object::new()
                .set_bucket(&self.inner.bucket_path)
                .set_name(name)
                .set_metadata(merged);
            let updated = self
                .inner
                .control
                .update_object()
                .set_object(object)
                .set_update_mask(FieldMask::default().set_paths(["metadata"]))
                .send()
                .await
                .map_err(|err| to_exception("patch object metadata failed", err))?;
            Ok(ObjectMeta::from(updated))
        })
        .await
    }

    pub async fn delete_object(&self, name: &str) -> Result<(), Exception> {
        validate_object_name(name)?;
        let result = self
            .run_logged("delete", name, async {
                self.inner
                    .control
                    .delete_object()
                    .set_bucket(&self.inner.bucket_path)
                    .set_object(name)
                    .send()
                    .await
                    .map_err(|err| to_exception("delete object failed", err))
            })
            .await;
        not_found_as_none(result).map(|_| ())
    }

    pub async fn move_object(&self, from: &str, to: &str) -> Result<ObjectMeta, Exception> {
        if from == to {
            return Err(exception!(
                code = MOVE_SAME_OBJECT,
                "move source and destination are the same object",
            ));
        }
        let meta = self.rewrite_object(from, to).await?;
        match self.delete_object(from).await {
            Ok(()) => Ok(meta),
            Err(delete_err) => Err(self.cleanup_after_failed_move(from, to, delete_err).await),
        }
    }

    pub async fn signed_get_url(
        &self,
        name: &str,
        expires_in: Duration,
        attachment_filename: Option<&str>,
    ) -> Result<String, Exception> {
        validate_object_name(name)?;
        let lifetime = validate_lifetime(expires_in)?;
        let mut builder = SignedUrlBuilder::for_object(&self.inner.bucket_path, name)
            .with_method(Method::GET)
            .with_expiration(lifetime);
        if let Some(filename) = attachment_filename {
            builder = builder.with_query_param(
                "response-content-disposition",
                content_disposition_value(filename)?,
            );
        }
        self.run_logged("sign", name, async {
            builder
                .sign_with(&self.inner.signer)
                .await
                .map_err(|err| exception!(code = SIGN_URL_FAILED, "sign URL failed", source = err))
        })
        .await
    }

    pub fn attach_ops_log(&self, logger: Logger) {
        *lock_logger(&self.inner.ops_log) = Some(logger);
    }

    // A failed delete does not prove that the source still exists. Delete the copy only
    // when the source is confirmed, else the copy can be the only one.
    async fn cleanup_after_failed_move(
        &self,
        from: &str,
        to: &str,
        delete_err: Exception,
    ) -> Exception {
        let kind = delete_err.code();
        let message = match self.stat_opt(from).await {
            Ok(Some(_)) => match self.delete_object(to).await {
                Ok(()) => return delete_err,
                Err(cleanup_err) => format!(
                    "move could not delete the source ({delete_err}) and could not delete \
                     the destination ({cleanup_err}); the object exists at both names"
                ),
            },
            Ok(None) => format!(
                "move could not delete the source ({delete_err}) but the source is gone; \
                 the object exists only at the destination"
            ),
            Err(check_err) => format!(
                "move could not delete the source ({delete_err}) and could not check \
                 the source ({check_err}); the object may exist at both names"
            ),
        };
        exception!(code = kind, message, source = delete_err)
    }

    // A missing object is a normal result of an `_opt` call, so it is an info record.
    async fn run_logged<T>(
        &self,
        operation: &str,
        object: &str,
        work: impl Future<Output = Result<T, Exception>>,
    ) -> Result<T, Exception> {
        let started = Instant::now();
        let result = work.await;
        let error = match &result {
            Ok(_) => None,
            Err(err) if err.code() == RESOURCE_NOT_FOUND => None,
            Err(err) => Some(String::from(err)),
        };
        self.write_ops_record(OpsRecord {
            operation,
            object,
            latency_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            error: error.as_deref(),
        });
        result
    }

    fn write_ops_record(&self, record: OpsRecord<'_>) {
        let Some(logger) = lock_logger(&self.inner.ops_log).clone() else {
            return;
        };
        logger.record(record.into_event(self.bucket()));
    }
}

impl fmt::Debug for GcsClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GcsClient")
            .field("bucket", &self.inner.bucket)
            .finish_non_exhaustive()
    }
}

struct Inner {
    bucket: String,
    bucket_path: String,
    storage: Storage,
    control: StorageControl,
    signer: Signer,
    ops_log: std::sync::Mutex<Option<Logger>>,
}

pub(crate) fn not_found_as_none<T>(result: Result<T, Exception>) -> Result<Option<T>, Exception> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(err) if err.code() == RESOURCE_NOT_FOUND => Ok(None),
        Err(err) => Err(err),
    }
}

pub(crate) fn validate_object_name(name: &str) -> Result<(), Exception> {
    if name.is_empty()
        || name.len() > MAX_OBJECT_NAME_BYTES
        || name == "."
        || name == ".."
        || name.contains(['\r', '\n'])
    {
        return Err(exception!(
            code = OBJECT_NAME_INVALID,
            "object name is empty, too long, `.`, `..` or contains a line break",
        ));
    }
    Ok(())
}

pub(crate) fn validate_metadata(metadata: &BTreeMap<String, String>) -> Result<(), Exception> {
    for (key, value) in metadata {
        if key.is_empty() || key.contains(['\r', '\n']) || value.contains(['\r', '\n']) {
            return Err(exception!(
                code = METADATA_INVALID,
                "metadata key is empty or metadata contains a line break",
            ));
        }
    }
    Ok(())
}

pub(crate) fn validate_bucket(bucket_name: &str) -> Result<(), Exception> {
    if bucket_name.is_empty()
        || bucket_name.contains(['/', '\\', ' ', '\r', '\n'])
        || !bucket_name.is_ascii()
    {
        return Err(exception!(
            code = BUCKET_NAME_INVALID,
            "bucket name is empty or contains a forbidden character",
        ));
    }
    Ok(())
}

fn build_signer(signer_email: Option<&str>) -> Result<Signer, Exception> {
    let build_failed = |err| {
        exception!(
            code = URL_SIGNER_BUILD_FAILED,
            "cannot build the URL signer",
            source = err,
        )
    };
    let Some(email) = signer_email else {
        return credentials::Builder::default()
            .build_signer()
            .map_err(build_failed);
    };
    if !email.contains('@') || email.contains(char::is_whitespace) {
        return Err(exception!(
            code = SIGNER_EMAIL_INVALID,
            "signer email is not a service account email",
        ));
    }
    let source = credentials::Builder::default()
        .build()
        .map_err(build_failed)?;
    credentials::impersonated::Builder::from_source_credentials(source)
        .with_target_principal(email)
        .build_signer()
        .map_err(build_failed)
}

async fn open_upload_file(path: &std::path::Path) -> Result<tokio::fs::File, Exception> {
    let file = tokio::fs::File::open(path).await.map_err(|err| {
        exception!(
            code = FILE_OPEN_FAILED,
            format!("cannot open {}", path.display()),
            source = err,
        )
    })?;
    let meta = file.metadata().await.map_err(|err| {
        exception!(
            code = FILE_METADATA_FAILED,
            format!("cannot read metadata for {}", path.display()),
            source = err,
        )
    })?;
    if !meta.is_file() {
        return Err(exception!(
            code = NOT_A_FILE,
            format!("{} is not a file", path.display()),
        ));
    }
    Ok(file)
}

fn lock_logger(
    slot: &std::sync::Mutex<Option<Logger>>,
) -> std::sync::MutexGuard<'_, Option<Logger>> {
    slot.lock().unwrap_or_else(|err| err.into_inner())
}
