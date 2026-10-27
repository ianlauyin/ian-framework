use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GcpErrorKind {
    Auth,
    Config,
    Api,
    Transport,
}

pub(crate) struct GcpError {
    kind: GcpErrorKind,
    message: String,
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl GcpError {
    #[cfg_attr(not(feature = "gcs"), allow(dead_code))]
    pub(crate) fn kind(&self) -> GcpErrorKind {
        self.kind
    }

    pub(crate) fn new(kind: GcpErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            source: None,
        }
    }

    pub(crate) fn with_source(
        kind: GcpErrorKind,
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self {
            kind,
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }

    #[cfg(feature = "gcs")]
    pub(crate) fn into_parts(self) -> (String, Option<Box<dyn std::error::Error + Send + Sync>>) {
        (self.message, self.source)
    }
}

impl fmt::Debug for GcpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GcpError")
            .field("kind", &self.kind)
            .field("message", &self.message)
            .finish_non_exhaustive()
    }
}

impl fmt::Display for GcpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for GcpError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.source {
            Some(source) => Some(source.as_ref()),
            None => None,
        }
    }
}

pub(crate) fn api_detail(body: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    if let Some(message) = value
        .get("error")
        .and_then(|error| error.get("message"))
        .and_then(|m| m.as_str())
    {
        return Some(message.to_string());
    }
    if let Some(message) = value.get("error_description").and_then(|m| m.as_str()) {
        return Some(message.to_string());
    }
    None
}

pub(crate) fn truncate(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_detail_reads_the_google_message() {
        let body = r#"{"error":{"code":404,"message":"No such object"}}"#;
        assert_eq!(api_detail(body).as_deref(), Some("No such object"));
        let body = r#"{"error":"invalid_grant","error_description":"Bad Request"}"#;
        assert_eq!(api_detail(body).as_deref(), Some("Bad Request"));
        assert_eq!(api_detail("not json"), None);
    }

    #[test]
    fn truncate_keeps_a_char_boundary() {
        assert_eq!(truncate("abc", 5), "abc");
        assert_eq!(truncate("abcdef", 3), "abc");
        assert_eq!(truncate("aé", 2), "a");
    }

    #[test]
    fn display_and_debug_show_the_message() {
        let error = GcpError::new(GcpErrorKind::Auth, "token status 401: denied");
        assert_eq!(error.to_string(), "token status 401: denied");
        assert!(format!("{error:?}").contains("Auth"));
        assert!(std::error::Error::source(&error).is_none());
    }
}
