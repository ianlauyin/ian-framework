use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::{Request, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::http::middleware::client_ip;
use crate::http::ApiError;

pub(crate) const PRUNE_AT: usize = 10_000;
// A full map drops this many of the least recently used buckets at once, so the scan is rare.
pub(crate) const EVICT_COUNT: usize = PRUNE_AT / 10;
pub(crate) const UNKNOWN_WAIT: Duration = Duration::from_secs(3600);

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
    pub(crate) buckets: Arc<Mutex<HashMap<String, Bucket>>>,
}

pub(crate) struct Bucket {
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

    pub(crate) fn check(&self, key: &str, now: Instant) -> Result<(), Duration> {
        let capacity = f64::from(self.config.capacity);
        let refill = self.config.refill_per_sec.max(0.0);
        let mut buckets = self
            .buckets
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        make_room(&mut buckets, key, now, refill, capacity);
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
    let key = limit_key(&client_ip(&request));
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

// One client can own a whole IPv6 /64 network, so all addresses of a /64 share one bucket.
pub(crate) fn limit_key(ip: &str) -> String {
    match ip.parse::<IpAddr>() {
        Ok(IpAddr::V6(v6)) => {
            let mut octets = v6.octets();
            octets[8..].fill(0);
            format!("{}/64", std::net::Ipv6Addr::from(octets))
        }
        _ => ip.to_string(),
    }
}

fn make_room(
    buckets: &mut HashMap<String, Bucket>,
    key: &str,
    now: Instant,
    refill: f64,
    capacity: f64,
) {
    if buckets.len() < PRUNE_AT || buckets.contains_key(key) {
        return;
    }
    buckets.retain(|_, bucket| refilled(bucket, now, refill, capacity) < capacity);
    if buckets.len() < PRUNE_AT {
        return;
    }
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

fn refilled(bucket: &Bucket, now: Instant, refill: f64, capacity: f64) -> f64 {
    let elapsed = now.saturating_duration_since(bucket.updated).as_secs_f64();
    (bucket.tokens + elapsed * refill).min(capacity)
}
