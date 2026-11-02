use std::net::SocketAddr;

use axum::extract::{ConnectInfo, Request, State};
use axum::middleware::Next;
use axum::response::Response;

use crate::http::middleware::client_ip;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientIp(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientIpSource {
    Headers,
    Socket,
}

pub async fn set_client_ip(
    State(source): State<ClientIpSource>,
    mut request: Request,
    next: Next,
) -> Response {
    let ip = match source {
        ClientIpSource::Headers => client_ip(&request),
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

fn socket_ip(request: &Request) -> String {
    request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ConnectInfo(addr)| addr.ip().to_string())
        .unwrap_or_default()
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

    #[test]
    fn request_ip_falls_back_to_the_headers() {
        let request = Request::builder()
            .header("x-forwarded-for", "203.0.113.8, 10.0.0.1")
            .body(Body::empty())
            .unwrap();
        assert_eq!(request_ip(&request), "203.0.113.8");
    }
}
