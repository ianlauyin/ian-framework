use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::{Request, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::http::middleware::set_client_ip::request_ip;
use crate::http::ApiError;

const PRUNE_AT: usize = 10_000;
// A full map drops this many of the least recently used buckets at once, so the scan is rare.
const EVICT_COUNT: usize = PRUNE_AT / 10;
const UNKNOWN_WAIT: Duration = Duration::from_secs(3600);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RateLimitConfig {
    pub capacity: u32,
    pub refill_per_sec: f64,
}

impl RateLimitConfig {
    pub fn new(capacity: u32, refill_per_sec: f64) -> Self {
        Self {
            capacity,
            refill_per_sec,
        }
    }
}

#[derive(Clone)]
pub struct RateLimiter {
    config: RateLimitConfig,
    buckets: Arc<Mutex<HashMap<String, Bucket>>>,
}

struct Bucket {
    tokens: f64,
    updated: Instant,
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            buckets: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn check(&self, key: &str, now: Instant) -> Result<(), Duration> {
        let capacity = f64::from(self.config.capacity);
        let refill = self.config.refill_per_sec.max(0.0);
        let mut buckets = self
            .buckets
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if buckets.len() >= PRUNE_AT && !buckets.contains_key(key) {
            buckets.retain(|_, bucket| refilled(bucket, now, refill, capacity) < capacity);
        }
        if buckets.len() >= PRUNE_AT && !buckets.contains_key(key) {
            let mut used: Vec<(Instant, String)> = buckets
                .iter()
                .map(|(name, bucket)| (bucket.updated, name.clone()))
                .collect();
            let count = EVICT_COUNT.min(used.len());
            used.select_nth_unstable_by_key(count - 1, |(updated, _)| *updated);
            for (_, name) in used.into_iter().take(count) {
                buckets.remove(&name);
            }
        }
        let bucket = buckets.entry(key.to_string()).or_insert(Bucket {
            tokens: capacity,
            updated: now,
        });
        bucket.tokens = refilled(bucket, now, refill, capacity);
        bucket.updated = now;
        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            return Ok(());
        }
        if refill == 0.0 {
            return Err(UNKNOWN_WAIT);
        }
        Err(Duration::try_from_secs_f64((1.0 - bucket.tokens) / refill).unwrap_or(UNKNOWN_WAIT))
    }
}

pub async fn rate_limit(
    State(limiter): State<RateLimiter>,
    request: Request,
    next: Next,
) -> Response {
    let key = request_ip(&request);
    match limiter.check(&key, Instant::now()) {
        Ok(()) => next.run(request).await,
        Err(wait) => {
            let mut response = ApiError::new(
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limited",
                "too many requests",
            )
            .into_response();
            let seconds = wait.as_secs_f64().ceil().max(1.0) as u64;
            if let Ok(value) = HeaderValue::from_str(&seconds.to_string()) {
                response.headers_mut().insert(header::RETRY_AFTER, value);
            }
            response
        }
    }
}

fn refilled(bucket: &Bucket, now: Instant, refill: f64, capacity: f64) -> f64 {
    let elapsed = now.saturating_duration_since(bucket.updated).as_secs_f64();
    (bucket.tokens + elapsed * refill).min(capacity)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use axum::middleware::from_fn_with_state;
    use axum::routing::get;
    use axum::Router;
    use tower::ServiceExt;

    fn limiter(capacity: u32, refill_per_sec: f64) -> RateLimiter {
        RateLimiter::new(RateLimitConfig::new(capacity, refill_per_sec))
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
            .header("x-forwarded-for", forwarded)
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
}
