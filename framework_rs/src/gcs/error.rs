use crate::exception;
use crate::exception::Exception;
use crate::gcp::{api_detail, truncate};

const RESPONSE_DETAIL_MAX_BYTES: usize = 200;
const SIGNED_PARAM_PREFIX: &str = "x-goog-";
const REDACTED: &str = "***";
const SECRET_KEYS: [&str; 11] = [
    "upload_id",
    "access_token",
    "refresh_token",
    "id_token",
    "token",
    "signature",
    "sig",
    "client_secret",
    "password",
    "secret",
    "authorization",
];

pub(crate) fn is_retryable(err: &Exception) -> bool {
    matches!(err.code(), "GCS_TRANSPORT" | "GCS_UNAVAILABLE")
}

pub(crate) fn is_retryable_status(status: u16) -> bool {
    matches!(status, 408 | 429 | 500 | 502 | 503 | 504)
}

// A URL or a path keeps its path and loses its query. A value of a known secret key is replaced. Other text stays.
pub(crate) fn redact_query(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut redact_next = false;
    for (is_token, part) in split_tokens(text) {
        if !is_token {
            out.push_str(part);
            continue;
        }
        let lower = part.to_ascii_lowercase();
        if redact_next && !matches!(lower.as_str(), "bearer" | "basic") {
            out.push_str(REDACTED);
            redact_next = false;
            continue;
        }
        redact_next = matches!(
            lower.trim_end_matches([':', '=']),
            "authorization" | "bearer" | "basic"
        );
        out.push_str(&redact_token(part));
    }
    out
}

fn split_tokens(text: &str) -> Vec<(bool, &str)> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut in_token = false;
    for (index, ch) in text.char_indices() {
        let separator = is_token_end(ch);
        if separator == in_token {
            if let Some(part) = text.get(start..index) {
                if !part.is_empty() {
                    parts.push((in_token, part));
                }
            }
            start = index;
            in_token = !separator;
        }
    }
    if let Some(part) = text.get(start..) {
        if !part.is_empty() {
            parts.push((in_token, part));
        }
    }
    parts
}

fn redact_token(token: &str) -> String {
    let token = match token.split_once('?') {
        Some((path, _)) if path.contains('/') => path,
        _ => token,
    };
    token
        .split('&')
        .map(|pair| match pair.split_once('=') {
            Some((key, _)) if is_secret_key(key) => format!("{key}={REDACTED}"),
            _ => pair.to_string(),
        })
        .collect::<Vec<_>>()
        .join("&")
}

fn is_secret_key(key: &str) -> bool {
    let key = key.trim_start_matches('?').to_ascii_lowercase();
    key.starts_with(SIGNED_PARAM_PREFIX) || SECRET_KEYS.contains(&key.as_str())
}

fn is_token_end(ch: char) -> bool {
    ch.is_whitespace()
        || matches!(
            ch,
            '"' | '\'' | '<' | '>' | ')' | '\\' | ',' | ';' | ']' | '}'
        )
}

// The gcp code and the gcs code have the same meaning. The message and the source stay.
pub(crate) fn from_gcp(err: Exception) -> Exception {
    let code = match err.code() {
        "GCP_AUTH" => "GCS_AUTH",
        "GCP_CONFIG" => "GCS_CONFIG",
        "GCP_API" => "GCS_API",
        "GCP_TRANSPORT" => "GCS_TRANSPORT",
        _ => return err,
    };
    err.with_code(code)
}

pub(crate) async fn gcs_response_exception(response: reqwest::Response) -> Exception {
    let status = response.status().as_u16();
    let text = response.text().await.unwrap_or_default();
    let detail = response_detail(&text);
    let code = match status {
        401 | 403 => "GCS_AUTH",
        404 => "GCS_NOT_FOUND",
        _ if is_retryable_status(status) => "GCS_UNAVAILABLE",
        _ => "GCS_API",
    };
    exception!(format!("status {status}: {detail}"), code = code)
}

// The upstream body can hold a signed URL, so the detail has no URL query and no control character.
fn response_detail(body: &str) -> String {
    let detail = api_detail(body).unwrap_or_else(|| body.to_string());
    let redacted = redact_query(truncate(&detail, RESPONSE_DETAIL_MAX_BYTES * 10));
    let clean: String = redacted
        .chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .collect();
    truncate(&clean, RESPONSE_DETAIL_MAX_BYTES).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_gcp_keeps_the_meaning_message_and_source() {
        let err = exception!(
            "outer",
            code = "GCP_TRANSPORT",
            source = exception!("inner", code = "GCP_API")
        );
        let err = from_gcp(err);
        assert_eq!(err.code(), "GCS_TRANSPORT");
        assert_eq!(err.message(), "outer");
        let cause = err.source_exception().unwrap();
        assert_eq!(cause.code(), "GCP_API");
        assert_eq!(cause.message(), "inner");
        assert_eq!(
            from_gcp(exception!("x", code = "GCP_AUTH")).code(),
            "GCS_AUTH"
        );
        assert_eq!(
            from_gcp(exception!("x", code = "GCP_CONFIG")).code(),
            "GCS_CONFIG"
        );
        assert_eq!(
            from_gcp(exception!("x", code = "GCP_API")).code(),
            "GCS_API"
        );
    }

    #[test]
    fn retryable_status_list_is_shared() {
        for status in [408_u16, 429, 500, 502, 503, 504] {
            assert!(is_retryable_status(status), "{status}");
        }
        for status in [200_u16, 400, 401, 403, 404, 501] {
            assert!(!is_retryable_status(status), "{status}");
        }
    }

    #[test]
    fn response_detail_has_no_url_query_or_control_character() {
        let body = "bad url https://storage.example/o?X-Goog-Signature=abc&token=xyz now\r\nnext";
        let detail = response_detail(body);
        assert_eq!(detail, "bad url https://storage.example/o now  next");
        let json = r#"{"error":{"message":"see https://h/o?sig=secret"}}"#;
        assert_eq!(response_detail(json), "see https://h/o");
        assert_eq!(response_detail(&"é".repeat(500)).len(), 200);
    }

    #[test]
    fn redact_query_keeps_plain_text_with_a_question_mark() {
        for text in [
            "Invalid argument?name=foo",
            "is it correct? more text",
            "what?",
        ] {
            assert_eq!(redact_query(text), text);
        }
    }

    #[test]
    fn redact_query_cuts_only_the_url_query_in_compact_json() {
        let json = r#"{"url":"https://h/o?sig=secret&a=1","next":"ok?x"}"#;
        assert_eq!(redact_query(json), r#"{"url":"https://h/o","next":"ok?x"}"#);
        let text = "failed (HTTPS://h/o?upload_id=secret): reset";
        assert_eq!(redact_query(text), "failed (HTTPS://h/o): reset");
    }

    #[test]
    fn redact_query_hides_a_signed_parameter_without_a_url() {
        let text = "bad X-Goog-Signature=abc&X-Goog-Expires=60 done";
        let redacted = redact_query(text);
        assert_eq!(redacted, "bad X-Goog-Signature=***&X-Goog-Expires=*** done");
    }

    #[test]
    fn redact_query_cuts_the_query_of_a_relative_or_scheme_less_url() {
        for (text, expected) in [
            (
                "GET /upload/o?upload_id=secret failed",
                "GET /upload/o failed",
            ),
            (
                "see storage.example/b/o?sig=secret now",
                "see storage.example/b/o now",
            ),
            ("path=/o?x=1,next", "path=/o,next"),
        ] {
            assert_eq!(redact_query(text), expected);
        }
    }

    #[test]
    fn redact_query_hides_a_known_secret_key_without_a_query() {
        assert_eq!(
            redact_query("denied access_token=ya29.abc and upload_id=xyz"),
            "denied access_token=*** and upload_id=***"
        );
        assert_eq!(redact_query("sig=abc&keep=1"), "sig=***&keep=1");
        assert_eq!(
            redact_query("Authorization: Bearer ya29.abc failed"),
            "Authorization: Bearer *** failed"
        );
        assert_eq!(redact_query("bearer abc"), "bearer ***");
    }

    #[test]
    fn redact_query_ends_a_token_at_a_comma_or_a_semicolon() {
        assert_eq!(
            redact_query("a=https://h/o?sig=secret;b=2,c=3]d}"),
            "a=https://h/o;b=2,c=3]d}"
        );
        assert_eq!(redact_query("token=abc,next=1"), "token=***,next=1");
        assert_eq!(redact_query("[\"https://h/o?sig=s\"]"), "[\"https://h/o\"]");
    }

    #[test]
    fn redact_query_keeps_prose() {
        for text in [
            "is it correct? more text",
            "Invalid argument?name=foo",
            "what? yes, no; maybe",
            "the token was rejected",
        ] {
            assert_eq!(redact_query(text), text);
        }
    }

    #[test]
    fn retryable_codes_match_the_upload_policy() {
        assert!(is_retryable(&exception!("reset", code = "GCS_TRANSPORT")));
        assert!(is_retryable(&exception!("busy", code = "GCS_UNAVAILABLE")));
        for code in ["GCS_LOCAL", "GCS_NOT_FOUND", "GCS_API", "GCS_CONFIG"] {
            assert!(!is_retryable(&exception!("x", code = code)), "{code}");
        }
    }
}
