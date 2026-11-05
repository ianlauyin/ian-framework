use serde::Deserialize;

use crate::exception;
use crate::exception::Exception;
use crate::gcs::encode::{encode_object_id, encode_seg};
use crate::gcs::gcs_client::object::RawObject;
use crate::gcs::gcs_client::{object_url, ObjectMeta};

pub(crate) enum RewriteStep {
    Done(ObjectMeta),
    Next(String),
}

impl RewriteStep {
    pub(crate) fn url(
        api_root: &str,
        bucket_name: &str,
        from: &str,
        to: &str,
        token: Option<&str>,
    ) -> String {
        let mut url = format!(
            "{}/rewriteTo/b/{}/o/{}",
            object_url(api_root, bucket_name, from),
            encode_seg(bucket_name),
            encode_object_id(to),
        );
        if let Some(token) = token {
            url.push_str("?rewriteToken=");
            url.push_str(&encode_seg(token));
        }
        url
    }

    pub(crate) fn from_slice(bytes: &[u8]) -> Result<Self, Exception> {
        let raw: RawRewrite = serde_json::from_slice(bytes)
            .map_err(|err| exception!(code = "GCS_API", "rewrite JSON is invalid", source = err))?;
        if raw.done {
            let resource = raw.resource.ok_or_else(|| {
                exception!(code = "GCS_API", "rewrite did not return object metadata")
            })?;
            return Ok(Self::Done(resource.into_meta()?));
        }
        match raw.rewrite_token {
            Some(token) if !token.is_empty() => Ok(Self::Next(token)),
            _ => Err(exception!(
                code = "GCS_API",
                "rewrite did not return a token",
            )),
        }
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
