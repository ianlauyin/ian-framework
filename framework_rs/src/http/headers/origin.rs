use axum::http::header::{HOST, ORIGIN};
use axum::http::HeaderMap;

// A browser that does not send Sec-Fetch-Site still sends Origin on a POST.
pub fn is_same_origin(headers: &HeaderMap) -> bool {
    if let Some(site) = headers.get("sec-fetch-site") {
        return site.as_bytes() == b"same-origin";
    }
    let Some(origin) = headers.get(ORIGIN).and_then(|value| value.to_str().ok()) else {
        return false;
    };
    let Some(host) = headers.get(HOST).and_then(|value| value.to_str().ok()) else {
        return false;
    };
    origin
        .split_once("://")
        .is_some_and(|(_, authority)| authority.eq_ignore_ascii_case(host))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn headers(pairs: &[(&'static str, &str)]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for (name, value) in pairs {
            headers.insert(*name, HeaderValue::from_str(value).unwrap());
        }
        headers
    }

    #[test]
    fn uses_sec_fetch_site_when_present() {
        assert!(is_same_origin(&headers(&[(
            "sec-fetch-site",
            "same-origin"
        )])));
        for site in ["same-site", "cross-site", "none"] {
            assert!(!is_same_origin(&headers(&[("sec-fetch-site", site)])));
        }
        let cross = headers(&[
            ("sec-fetch-site", "cross-site"),
            ("origin", "https://app.example"),
            ("host", "app.example"),
        ]);
        assert!(!is_same_origin(&cross));
    }

    #[test]
    fn falls_back_to_origin_and_host() {
        let same = headers(&[
            ("origin", "https://App.example:8443"),
            ("host", "app.example:8443"),
        ]);
        assert!(is_same_origin(&same));
        let other = headers(&[("origin", "https://evil.example"), ("host", "app.example")]);
        assert!(!is_same_origin(&other));
        let null = headers(&[("origin", "null"), ("host", "app.example")]);
        assert!(!is_same_origin(&null));
        assert!(!is_same_origin(&headers(&[(
            "origin",
            "https://app.example"
        )])));
        assert!(!is_same_origin(&HeaderMap::new()));
    }
}
