use serde::Deserialize;

use crate::gcs::encode::{encode_object_id, encode_seg};
use crate::gcs::gcs_client::{object_url, validate_object_name, Call, GcsClient};
use crate::gcs::object_meta::RawObject;
use crate::gcs::{GcsError, GcsErrorKind, ObjectMeta};

impl GcsClient {
    pub async fn rewrite_object(&self, from: &str, to: &str) -> Result<ObjectMeta, GcsError> {
        validate_object_name(from)?;
        validate_object_name(to)?;
        let mut token: Option<String> = None;
        loop {
            let response = self
                .send(&Call {
                    method: reqwest::Method::POST,
                    url: rewrite_url(self.bucket(), from, to, token.as_deref()),
                    json: Some(serde_json::json!({})),
                    ..Default::default()
                })
                .await?;
            let bytes = response.bytes().await.map_err(|err| {
                GcsError::with_source(
                    GcsErrorKind::Transport,
                    "rewrite response is not bytes",
                    err,
                )
            })?;
            let next = match RewriteStep::from_slice(&bytes)? {
                RewriteStep::Done(meta) => return Ok(meta),
                RewriteStep::Next(next) => next,
            };
            if token.as_deref() == Some(next.as_str()) {
                return Err(GcsError::new(
                    GcsErrorKind::Api,
                    "rewrite token did not change",
                ));
            }
            token = Some(next);
        }
    }
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

enum RewriteStep {
    Done(ObjectMeta),
    Next(String),
}

impl RewriteStep {
    fn from_slice(bytes: &[u8]) -> Result<Self, GcsError> {
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
            rewrite_url("buck", "a/b", "c d", None),
            "https://storage.googleapis.com/storage/v1/b/buck/o/a%2Fb/rewriteTo/b/buck/o/c%20d"
        );
        let url = rewrite_url("buck", "a", "c", Some("tok/en"));
        assert!(url.ends_with("?rewriteToken=tok%2Fen"));
    }
}
