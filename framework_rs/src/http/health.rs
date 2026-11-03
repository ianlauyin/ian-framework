use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

pub fn health_router() -> Router {
    Router::new().route("/health", get(get_health))
}

async fn get_health() -> Json<HealthView> {
    Json(HealthView { ok: true })
}

#[derive(Serialize)]
struct HealthView {
    ok: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn health_is_ok() {
        let response = health_router()
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["content-type"], "application/json");
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(body.as_ref(), br#"{"ok":true}"#);
    }

    #[tokio::test]
    async fn other_paths_are_not_found() {
        let response = health_router()
            .oneshot(
                Request::builder()
                    .uri("/other")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
