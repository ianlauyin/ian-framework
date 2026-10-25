use serde::Deserialize;

use crate::gcs::{GcsError, GcsErrorKind, ObjectMeta};

use super::object_meta::RawObject;

pub(super) enum RewriteStep {
    Done(ObjectMeta),
    Next(String),
}

impl RewriteStep {
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
}
