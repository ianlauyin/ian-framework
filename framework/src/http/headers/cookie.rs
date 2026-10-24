use axum::http::header::{COOKIE, SET_COOKIE};
use axum::http::HeaderMap;

// HTTP/2 clients can send each cookie in its own Cookie field, so read every field.
pub fn read_cookie<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get_all(COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|text| text.split(';'))
        .find_map(|part| {
            let (key, value) = part.trim().split_once('=')?;
            (key == name).then_some(value)
        })
}

pub fn sets_cookie(headers: &HeaderMap, name: &str) -> bool {
    headers.get_all(SET_COOKIE).iter().any(|value| {
        value
            .to_str()
            .ok()
            .and_then(|text| text.split_once('='))
            .is_some_and(|(key, _)| key.trim() == name)
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
            COOKIE,
            HeaderValue::from_static("theme=dark; sid=abc; sid=def"),
        );
        assert_eq!(read_cookie(&headers, "sid"), Some("abc"));
        assert_eq!(read_cookie(&headers, "si"), None);
    }

    #[test]
    fn reads_each_cookie_field() {
        let mut headers = HeaderMap::new();
        headers.append(COOKIE, HeaderValue::from_static("theme=dark"));
        headers.append(COOKIE, HeaderValue::from_static("sid=abc"));
        assert_eq!(read_cookie(&headers, "sid"), Some("abc"));
    }

    #[test]
    fn skips_a_non_utf8_field() {
        let mut headers = HeaderMap::new();
        headers.append(COOKIE, HeaderValue::from_bytes(b"sid=\xff").unwrap());
        assert_eq!(read_cookie(&headers, "sid"), None);
        headers.append(COOKIE, HeaderValue::from_static("sid=abc"));
        assert_eq!(read_cookie(&headers, "sid"), Some("abc"));
    }

    #[test]
    fn finds_a_set_cookie_with_the_name() {
        let mut headers = HeaderMap::new();
        assert!(!sets_cookie(&headers, "sid"));
        headers.append(SET_COOKIE, HeaderValue::from_static("theme=dark"));
        headers.append(SET_COOKIE, HeaderValue::from_static("sid_old=x"));
        assert!(!sets_cookie(&headers, "sid"));
        headers.append(SET_COOKIE, HeaderValue::from_static("sid=; Max-Age=0"));
        assert!(sets_cookie(&headers, "sid"));
    }
}
