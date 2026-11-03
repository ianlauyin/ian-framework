use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicBool, Ordering};

use axum::extract::{ConnectInfo, Request, State};
use axum::middleware::Next;
use axum::response::Response;

use crate::http::middleware::client_ip;

static WARNED_PUBLIC_PEER: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientIp(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientIpSource {
    Headers,
    // One header that the proxy sets or overwrites, such as `cf-connecting-ip`. Without the header, the socket host is used.
    Header(&'static str),
    // The entry of `x-forwarded-for` that is `trusted_hops` places from the right end. Each trusted proxy adds one entry, and a client can only add entries at the left. Without such an entry, the socket host is used.
    ForwardedFor { trusted_hops: usize },
    Socket,
}

pub async fn set_client_ip(
    State(source): State<ClientIpSource>,
    mut request: Request,
    next: Next,
) -> Response {
    let ip = match source {
        ClientIpSource::Headers => {
            warn_public_peer(&request);
            client_ip(&request)
        }
        ClientIpSource::Header(name) => {
            header_ip(&request, name).unwrap_or_else(|| socket_ip(&request))
        }
        ClientIpSource::ForwardedFor { trusted_hops } => {
            forwarded_ip(&request, trusted_hops).unwrap_or_else(|| socket_ip(&request))
        }
        ClientIpSource::Socket => socket_ip(&request),
    };
    request.extensions_mut().insert(ClientIp(ip));
    next.run(request).await
}

pub(crate) fn request_ip(request: &Request) -> String {
    match request.extensions().get::<ClientIp>() {
        Some(ClientIp(ip)) => ip.clone(),
        None => client_ip(request),
    }
}

// A peer on a public address did not come through a private proxy hop, so it can send any value in the headers.
fn warn_public_peer(request: &Request) {
    let Some(ConnectInfo(addr)) = request.extensions().get::<ConnectInfo<SocketAddr>>() else {
        return;
    };
    if !is_public(addr.ip()) || WARNED_PUBLIC_PEER.swap(true, Ordering::Relaxed) {
        return;
    }
    tracing::warn!(
        "ClientIpSource::Headers got a request from a public peer address, so a client can set the client ip"
    );
}

fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => !(v4.is_private() || v4.is_loopback() || v4.is_link_local()),
        IpAddr::V6(v6) => !(v6.is_loopback() || v6.segments()[0] & 0xfe00 == 0xfc00),
    }
}

fn header_ip(request: &Request, name: &str) -> Option<String> {
    let value = request.headers().get(name)?.to_str().ok()?.trim();
    valid_ip(value)
}

fn forwarded_ip(request: &Request, trusted_hops: usize) -> Option<String> {
    if trusted_hops == 0 {
        return None;
    }
    let entries: Vec<&str> = request
        .headers()
        .get_all("x-forwarded-for")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|text| text.split(','))
        .map(str::trim)
        .collect();
    let entry = entries.iter().rev().nth(trusted_hops - 1)?;
    valid_ip(entry)
}

// A value that is not an IP could be any text that a client chooses, so it must not become a rate limit key or a log value.
fn valid_ip(value: &str) -> Option<String> {
    value.parse::<IpAddr>().ok().map(|ip| ip.to_string())
}

fn socket_ip(request: &Request) -> String {
    match request.extensions().get::<ConnectInfo<SocketAddr>>() {
        Some(ConnectInfo(addr)) => addr.ip().to_string(),
        None => {
            tracing::warn!("request has no ConnectInfo, so the client ip is empty");
            String::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use axum::middleware::from_fn_with_state;
    use axum::routing::get;
    use axum::Router;
    use tower::ServiceExt;

    async fn show(request: Request) -> String {
        request_ip(&request)
    }

    async fn ip_with(source: ClientIpSource) -> String {
        let router = Router::new()
            .route("/", get(show))
            .layer(from_fn_with_state(source, set_client_ip));
        let request = Request::builder()
            .uri("/")
            .header("cf-connecting-ip", "203.0.113.9")
            .extension(ConnectInfo(SocketAddr::from(([192, 0, 2, 10], 443))))
            .body(Body::empty())
            .unwrap();
        let response = router.oneshot(request).await.unwrap();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        String::from_utf8(body.to_vec()).unwrap()
    }

    #[tokio::test]
    async fn headers_source_uses_the_proxy_headers() {
        assert_eq!(ip_with(ClientIpSource::Headers).await, "203.0.113.9");
    }

    #[tokio::test]
    async fn socket_source_ignores_the_proxy_headers() {
        assert_eq!(ip_with(ClientIpSource::Socket).await, "192.0.2.10");
    }

    async fn ip_for(source: ClientIpSource, headers: &[(&str, &str)]) -> String {
        let router = Router::new()
            .route("/", get(show))
            .layer(from_fn_with_state(source, set_client_ip));
        let mut request = Request::builder().uri("/");
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        let request = request
            .extension(ConnectInfo(SocketAddr::from(([192, 0, 2, 10], 443))))
            .body(Body::empty())
            .unwrap();
        let response = router.oneshot(request).await.unwrap();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        String::from_utf8(body.to_vec()).unwrap()
    }

    #[tokio::test]
    async fn header_source_reads_only_the_named_header() {
        let source = ClientIpSource::Header("cf-connecting-ip");
        let both = [
            ("cf-connecting-ip", "203.0.113.9"),
            ("x-forwarded-for", "198.51.100.1"),
        ];
        assert_eq!(ip_for(source, &both).await, "203.0.113.9");
        assert_eq!(
            ip_for(source, &[("x-forwarded-for", "198.51.100.1")]).await,
            "192.0.2.10"
        );
    }

    #[tokio::test]
    async fn forwarded_for_source_counts_the_trusted_hops_from_the_right() {
        let spoofed = [("x-forwarded-for", "6.6.6.6, 203.0.113.9, 10.0.0.1")];
        let one = ClientIpSource::ForwardedFor { trusted_hops: 1 };
        assert_eq!(ip_for(one, &spoofed).await, "10.0.0.1");
        let two = ClientIpSource::ForwardedFor { trusted_hops: 2 };
        assert_eq!(ip_for(two, &spoofed).await, "203.0.113.9");
        let four = ClientIpSource::ForwardedFor { trusted_hops: 4 };
        assert_eq!(ip_for(four, &spoofed).await, "192.0.2.10");
        let none = ClientIpSource::ForwardedFor { trusted_hops: 0 };
        assert_eq!(ip_for(none, &spoofed).await, "192.0.2.10");
        assert_eq!(ip_for(one, &[]).await, "192.0.2.10");
    }

    #[tokio::test]
    async fn a_value_that_is_not_an_ip_gives_the_socket_host() {
        let header = ClientIpSource::Header("cf-connecting-ip");
        assert_eq!(
            ip_for(header, &[("cf-connecting-ip", "not an ip")]).await,
            "192.0.2.10"
        );
        let forwarded = ClientIpSource::ForwardedFor { trusted_hops: 1 };
        assert_eq!(
            ip_for(forwarded, &[("x-forwarded-for", "6.6.6.6, evil")]).await,
            "192.0.2.10"
        );
        assert_eq!(
            ip_for(header, &[("cf-connecting-ip", "2001:db8::1")]).await,
            "2001:db8::1"
        );
    }

    #[test]
    fn private_and_loopback_peers_are_not_public() {
        for ip in [
            "10.1.2.3",
            "172.16.0.1",
            "192.168.1.1",
            "127.0.0.1",
            "169.254.1.1",
            "::1",
            "fd00::1",
        ] {
            assert!(!is_public(ip.parse().unwrap()), "{ip}");
        }
        for ip in ["192.0.2.10", "203.0.113.9", "2001:db8::1"] {
            assert!(is_public(ip.parse().unwrap()), "{ip}");
        }
    }

    #[test]
    fn request_ip_falls_back_to_the_headers() {
        let request = Request::builder()
            .header("x-forwarded-for", "203.0.113.8, 10.0.0.1")
            .body(Body::empty())
            .unwrap();
        assert_eq!(request_ip(&request), "203.0.113.8");
    }
}
