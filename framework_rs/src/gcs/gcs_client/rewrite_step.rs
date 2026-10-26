use serde::Deserialize;

use crate::gcs::encode::{encode_object_id, encode_seg};
use crate::gcs::gcs_client::object::RawObject;
use crate::gcs::gcs_client::object_url;
use crate::gcs::{GcsError, GcsErrorKind, ObjectMeta};

pub(super) enum RewriteStep {
    Done(ObjectMeta),
    Next(String),
}

impl RewriteStep {
    pub(super) fn url(bucket_name: &str, from: &str, to: &str, token: Option<&str>) -> String {
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

    pub(super) fn from_slice(bytes: &[u8]) -> Result<Self, GcsError> {
        let raw: RawRewrite = serde_json::from_slice(bytes).map_err(|err| {
            GcsError::with_source(GcsErrorKind::Api, "rewrite JSON is invalid", err)
        })?;
        if raw.done {
            let resource = raw.resource.ok_or_else(|| {
                GcsError::new(GcsErrorKind::Api, "rewrite did not return object metadata")
            })?;
            return Ok(Self::Done(resource.into_meta()?));
        }
        match raw.rewrite_token {
            Some(token) if !token.is_empty() => Ok(Self::Next(token)),
            _ => Err(GcsError::new(
                GcsErrorKind::Api,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrite_step_reads_a_token_until_done() {
        let step = RewriteStep::from_slice(br#"{"done":false,"rewriteToken":"abc"}"#).unwrap();
        assert!(matches!(step, RewriteStep::Next(token) if token == "abc"));
        let done = RewriteStep::from_slice(br#"{"done":true,"resource":{"name":"b","size":"3"}}"#)
            .unwrap();
        let RewriteStep::Done(resource) = done else {
            panic!("expected a finished rewrite");
        };
        assert_eq!(resource.name, "b");
        assert_eq!(resource.size, 3);
        assert!(RewriteStep::from_slice(br#"{"done":false}"#).is_err());
        assert!(RewriteStep::from_slice(br#"{"done":true}"#).is_err());
        assert!(RewriteStep::from_slice(br#"{"done":false,"rewriteToken":""}"#).is_err());
    }

    #[test]
    fn rewrite_url_sets_the_token_query() {
        assert_eq!(
            RewriteStep::url("buck", "a/b", "c d", None),
            "https://storage.googleapis.com/storage/v1/b/buck/o/a%2Fb/rewriteTo/b/buck/o/c%20d"
        );
        let url = RewriteStep::url("buck", "a", "c", Some("tok/en"));
        assert!(url.ends_with("?rewriteToken=tok%2Fen"));
    }
}
