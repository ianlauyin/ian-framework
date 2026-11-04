use std::time::{Duration, Instant};

use axum::extract::Request;
use axum::http::{header, StatusCode};
use axum::response::Response;

use crate::http::middleware::rate_limit::*;
use axum::body::{to_bytes, Body};
use axum::middleware::from_fn_with_state;
use axum::routing::get;
use axum::Router;
use tower::ServiceExt;

fn limiter(capacity: u32, refill_per_sec: f64) -> RateLimiter {
    RateLimiter::new(RateLimitConfig::new(capacity, refill_per_sec))
}

#[test]
fn an_ipv6_network_shares_one_key() {
    assert_eq!(
        limit_key("2001:db8:1:2:aaaa::1"),
        limit_key("2001:db8:1:2:bbbb::2")
    );
    assert_ne!(limit_key("2001:db8:1:2::1"), limit_key("2001:db8:1:3::1"));
    assert_eq!(limit_key("192.0.2.1"), "192.0.2.1");
    assert_eq!(limit_key(""), "");
}

#[test]
fn allows_the_capacity_then_refuses() {
    let limiter = limiter(2, 1.0);
    let now = Instant::now();
    assert!(limiter.check("a", now).is_ok());
    assert!(limiter.check("a", now).is_ok());
    assert!(limiter.check("a", now).is_err());
}

#[test]
fn keeps_each_key_apart() {
    let limiter = limiter(1, 1.0);
    let now = Instant::now();
    assert!(limiter.check("a", now).is_ok());
    assert!(limiter.check("a", now).is_err());
    assert!(limiter.check("b", now).is_ok());
}

#[test]
fn refills_with_time_and_reports_the_wait() {
    let limiter = limiter(1, 0.5);
    let now = Instant::now();
    assert!(limiter.check("a", now).is_ok());
    let wait = limiter.check("a", now).unwrap_err();
    assert_eq!(wait, Duration::from_secs(2));
    assert!(limiter.check("a", now + Duration::from_secs(1)).is_err());
    assert!(limiter.check("a", now + Duration::from_secs(3)).is_ok());
}

#[test]
fn a_zero_refill_never_recovers() {
    let limiter = limiter(1, 0.0);
    let now = Instant::now();
    assert!(limiter.check("a", now).is_ok());
    assert_eq!(limiter.check("a", now).unwrap_err(), UNKNOWN_WAIT);
    assert!(limiter
        .check("a", now + Duration::from_secs(86_400))
        .is_err());
}

#[test]
fn prunes_full_buckets_when_the_map_is_large() {
    let limiter = limiter(2, 1.0);
    let now = Instant::now();
    for index in 0..PRUNE_AT {
        assert!(limiter.check(&index.to_string(), now).is_ok());
    }
    let later = now + Duration::from_secs(10);
    assert!(limiter.check("new", later).is_ok());
    assert_eq!(limiter.buckets.lock().unwrap().len(), 1);
}

#[test]
fn evicts_the_oldest_buckets_when_no_bucket_is_full() {
    let limiter = limiter(2, 0.001);
    let now = Instant::now();
    for index in 0..PRUNE_AT {
        let at = now + Duration::from_millis(index as u64);
        assert!(limiter.check(&index.to_string(), at).is_ok());
    }
    let at = now + Duration::from_millis(PRUNE_AT as u64);
    assert!(limiter.check("new", at).is_ok());
    let buckets = limiter.buckets.lock().unwrap();
    assert_eq!(buckets.len(), PRUNE_AT - EVICT_COUNT + 1);
    assert!(!buckets.contains_key("0"));
    assert!(!buckets.contains_key(&(EVICT_COUNT - 1).to_string()));
    assert!(buckets.contains_key(&EVICT_COUNT.to_string()));
    assert!(buckets.contains_key("new"));
}

async fn send(router: &Router, forwarded: &str) -> Response {
    let request = Request::builder()
        .uri("/")
        .header("cf-connecting-ip", forwarded)
        .body(Body::empty())
        .unwrap();
    router.clone().oneshot(request).await.unwrap()
}

#[tokio::test]
async fn answers_429_with_retry_after_and_a_json_body() {
    let router = Router::new()
        .route("/", get(|| async { StatusCode::NO_CONTENT }))
        .layer(from_fn_with_state(limiter(1, 0.1), rate_limit));
    assert_eq!(
        send(&router, "203.0.113.8").await.status(),
        StatusCode::NO_CONTENT
    );
    let refused = send(&router, "203.0.113.8").await;
    assert_eq!(refused.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(refused.headers().get(header::RETRY_AFTER).unwrap(), "10");
    let body = to_bytes(refused.into_body(), usize::MAX).await.unwrap();
    assert_eq!(
        body,
        r#"{"code":"rate_limited","message":"too many requests"}"#
    );
    assert_eq!(
        send(&router, "203.0.113.9").await.status(),
        StatusCode::NO_CONTENT
    );
}
