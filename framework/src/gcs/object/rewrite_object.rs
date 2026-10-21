use serde::Deserialize;

use super::super::encode::{encode_object_id, encode_seg};
use super::super::{send, Call, GcsBackend, GcsError, GcsErrorKind};
use super::shared::{object_url, validate_object_name, ObjectMeta};

impl GcsBackend {
    pub async fn rewrite_object(&self, from: &str, to: &str) -> Result<ObjectMeta, GcsError> {
        validate_object_name(from)?;
        validate_object_name(to)?;
        let mut token: Option<String> = None;
        loop {
            let response = send(
                self,
                &Call {
                    method: reqwest::Method::POST,
                    url: rewrite_url(self.bucket(), from, to, token.as_deref()),
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
            let next = step.rewrite_token.ok_or_else(|| {
                GcsError::new(GcsErrorKind::Api, "rewrite did not return a token")
            })?;
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

#[derive(Debug, Deserialize)]
struct RawRewrite {
    #[serde(default)]
    done: bool,
    #[serde(rename = "rewriteToken", default)]
    rewrite_token: Option<String>,
    #[serde(default)]
    resource: Option<super::shared::RawObject>,
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
