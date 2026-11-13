use std::collections::BTreeMap;
use std::fmt;
use std::future::Future;
use std::time::Duration;

use bytes::Bytes;
use google_cloud_auth::signer::Signer;
use google_cloud_storage::builder::storage::SignedUrlBuilder;
use google_cloud_storage::client::{Storage, StorageControl};
use google_cloud_storage::http::Method;
use google_cloud_storage::model::Object;
use google_cloud_wkt::FieldMask;

use crate::exception;
use crate::exception::Exception;
use crate::gcp::error::to_exception;
use crate::gcp::storage::put_request::open_upload_file;
use crate::gcp::storage::signed_url::{build_signer, content_disposition_value, validate_lifetime};
use crate::gcp::storage::validate::{validate_bucket, validate_metadata, validate_object_name};
use crate::logging::{with_log, LogEvent, LogLevel};

use crate::gcp::storage::list::{ListPage, ListQuery};
use crate::gcp::storage::object::{ObjectMeta, ObjectReader};
use crate::gcp::storage::put_request::{PutBody, PutRequest};

pub const CLIENT_BUILD_FAILED: &str = "CLIENT_BUILD_FAILED";
pub const MOVE_SAME_OBJECT: &str = "MOVE_SAME_OBJECT";
pub const RESOURCE_NOT_FOUND: &str = "RESOURCE_NOT_FOUND";
pub const REWRITE_NO_OBJECT: &str = "REWRITE_NO_OBJECT";
pub const REWRITE_STEP_LIMIT: &str = "REWRITE_STEP_LIMIT";
pub const REWRITE_TOKEN_STUCK: &str = "REWRITE_TOKEN_STUCK";
pub const SIGN_URL_FAILED: &str = "SIGN_URL_FAILED";

// The most rewrite calls for one object. A rewrite that needs more is a fault.
const MAX_REWRITE_STEPS: u32 = 10_000;

#[derive(Clone)]
pub struct GcsClient {
    bucket: String,
    bucket_path: String,
    storage: Storage,
    control: StorageControl,
    signer: Signer,
}

impl GcsClient {
    // `signer_email` is the service account that signs URLs. Without it, the signer is the
    // account of the credentials, which must be a service account or the metadata server.
    pub async fn from_adc(
        bucket_name: impl Into<String>,
        signer_email: Option<&str>,
    ) -> Result<Self, Exception> {
        let bucket = bucket_name.into();
        validate_bucket(&bucket)?;
        let build_failed = |err| {
            exception!(
                code = CLIENT_BUILD_FAILED,
                "cannot build the client",
                source = err
            )
        };
        let storage = Storage::builder().build().await.map_err(build_failed)?;
        let control = StorageControl::builder()
            .build()
            .await
            .map_err(build_failed)?;
        Ok(Self {
            bucket_path: format!("projects/_/buckets/{bucket}"),
            bucket,
            storage,
            control,
            signer: build_signer(signer_email)?,
        })
    }

    pub async fn open_object(&self, name: &str) -> Result<Option<ObjectReader>, Exception> {
        validate_object_name(name)?;
        let result = self
            .with_op_log("open", name, async {
                let response = self
                    .storage
                    .read_object(&self.bucket_path, name)
                    .send()
                    .await
                    .map_err(|err| to_exception("open object failed", err))?;
                Ok(ObjectReader { response })
            })
            .await;
        not_found_as_none(result)
    }

    pub async fn stat(&self, name: &str) -> Result<Option<ObjectMeta>, Exception> {
        validate_object_name(name)?;
        let result = self
            .with_op_log("stat", name, async {
                let object = self
                    .control
                    .get_object()
                    .set_bucket(&self.bucket_path)
                    .set_object(name)
                    .send()
                    .await
                    .map_err(|err| to_exception("stat object failed", err))?;
                Ok(ObjectMeta::from(object))
            })
            .await;
        not_found_as_none(result)
    }

    pub async fn list_page(&self, query: &ListQuery) -> Result<ListPage, Exception> {
        let prefix = query.prefix.as_deref().unwrap_or_default();
        self.with_op_log("list", prefix, async {
            let mut request = self.control.list_objects().set_parent(&self.bucket_path);
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

    pub async fn put_object(&self, request: &PutRequest) -> Result<ObjectMeta, Exception> {
        let name = request.name.as_str();
        let options = &request.options;
        validate_object_name(name)?;
        let content_type = options.content_type_or_default()?;
        validate_metadata(&options.metadata)?;
        let metadata = options.metadata.clone();
        let bucket_path = &self.bucket_path;
        self.with_op_log("put", name, async {
            let result = match &request.body {
                PutBody::Bytes(bytes) => {
                    self.storage
                        .write_object(bucket_path, name, Bytes::copy_from_slice(bytes))
                        .set_content_type(content_type)
                        .set_metadata(metadata)
                        .send_unbuffered()
                        .await
                }
                PutBody::File(path) => {
                    let file = open_upload_file(path).await?;
                    self.storage
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
        self.with_op_log("rewrite", from, async {
            let mut token = String::new();
            for _ in 0..MAX_REWRITE_STEPS {
                let step = self
                    .control
                    .rewrite_object()
                    .set_source_bucket(&self.bucket_path)
                    .set_source_object(from)
                    .set_destination_bucket(&self.bucket_path)
                    .set_destination_name(to)
                    .set_rewrite_token(&token)
                    .send()
                    .await
                    .map_err(|err| to_exception("rewrite object failed", err))?;
                if step.done {
                    return step.resource.map(ObjectMeta::from).ok_or_else(|| {
                        exception!(code = REWRITE_NO_OBJECT, "rewrite gave no object")
                    });
                }
                if step.rewrite_token == token {
                    return Err(exception!(
                        code = REWRITE_TOKEN_STUCK,
                        "rewrite token did not change"
                    ));
                }
                token = step.rewrite_token;
            }
            Err(exception!(
                code = REWRITE_STEP_LIMIT,
                "rewrite did not finish within the step limit",
            ))
        })
        .await
    }

    // The new keys replace the keys with the same name. The other keys stay.
    pub async fn merge_metadata(
        &self,
        name: &str,
        metadata: &BTreeMap<String, String>,
    ) -> Result<ObjectMeta, Exception> {
        validate_object_name(name)?;
        validate_metadata(metadata)?;
        self.with_op_log("patch", name, async {
            let current = self
                .control
                .get_object()
                .set_bucket(&self.bucket_path)
                .set_object(name)
                .send()
                .await
                .map_err(|err| to_exception("read object metadata failed", err))?;
            let mut merged = current.metadata;
            merged.extend(metadata.clone());
            let object = Object::new()
                .set_bucket(&self.bucket_path)
                .set_name(name)
                .set_metadata(merged);
            let updated = self
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
            .with_op_log("delete", name, async {
                self.control
                    .delete_object()
                    .set_bucket(&self.bucket_path)
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
            Err(delete_err) => Err(self.recover_from_failed_move(from, to, delete_err).await),
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
        let mut builder = SignedUrlBuilder::for_object(&self.bucket_path, name)
            .with_method(Method::GET)
            .with_expiration(lifetime);
        if let Some(filename) = attachment_filename {
            builder = builder.with_query_param(
                "response-content-disposition",
                content_disposition_value(filename)?,
            );
        }
        self.with_op_log("sign", name, async {
            builder
                .sign_with(&self.signer)
                .await
                .map_err(|err| exception!(code = SIGN_URL_FAILED, "sign URL failed", source = err))
        })
        .await
    }

    // A failed delete does not prove that the source still exists. Delete the copy only
    // when the source is confirmed, else the copy can be the only one.
    async fn recover_from_failed_move(
        &self,
        from: &str,
        to: &str,
        delete_err: Exception,
    ) -> Exception {
        let kind = delete_err.code();
        let message = match self.stat(from).await {
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

    // A missing object is a normal result of `open_object` and `stat`, so it is an info record.
    async fn with_op_log<T>(
        &self,
        operation: &str,
        object: &str,
        work: impl Future<Output = Result<T, Exception>>,
    ) -> Result<T, Exception> {
        with_log(work, |result| {
            let action = format!("GCS/{operation}:/{object}");
            let (level, message, error) = match result {
                Ok(_) => (LogLevel::Info, "gcs operation succeeded", None),
                Err(err) if err.code() == RESOURCE_NOT_FOUND => {
                    (LogLevel::Info, "gcs operation succeeded", None)
                }
                Err(err) => (
                    LogLevel::Error,
                    "gcs operation failed",
                    Some(String::from(&*err)),
                ),
            };
            let mut event = LogEvent::build(action, level, error);
            event.extra.insert("message".to_string(), message.into());
            event
                .extra
                .insert("bucket".to_string(), self.bucket.as_str().into());
            event
        })
        .await
    }
}

impl fmt::Debug for GcsClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GcsClient")
            .field("bucket", &self.bucket)
            .finish_non_exhaustive()
    }
}

pub(crate) fn not_found_as_none<T>(result: Result<T, Exception>) -> Result<Option<T>, Exception> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(err) if err.code() == RESOURCE_NOT_FOUND => Ok(None),
        Err(err) => Err(err),
    }
}
