use axum::http::{header, HeaderMap};

use crate::auth::session_store::SessionStore;

impl SessionStore {
    pub fn token<'a>(&self, headers: &'a HeaderMap) -> Option<&'a str> {
        read_cookie(headers, &self.inner.config.cookie_name)
    }
}

fn read_cookie<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    let raw = headers.get(header::COOKIE)?.to_str().ok()?;
    raw.split(';').find_map(|part| {
        let (key, value) = part.trim().split_once('=')?;
        (key == name).then_some(value)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn reads_the_first_cookie_with_the_name() {
        let mut headers = HeaderMap::new();
        assert_eq!(read_cookie(&headers, "sid"), None);
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("theme=dark; sid=abc; sid=def"),
        );
        assert_eq!(read_cookie(&headers, "sid"), Some("abc"));
        assert_eq!(read_cookie(&headers, "si"), None);
        headers.insert(
            header::COOKIE,
            HeaderValue::from_bytes(b"sid=\xff").unwrap(),
        );
        assert_eq!(read_cookie(&headers, "sid"), None);
    }
}
