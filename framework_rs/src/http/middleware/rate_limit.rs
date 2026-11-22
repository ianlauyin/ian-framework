use std::collections::HashMap;
use std::net::{IpAddr, Ipv6Addr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::{Request, State};
use axum::http::{HeaderValue, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::exception;
use crate::exception::Exception;
use crate::http::error_code;
use crate::http::headers::client_ip;
use crate::logging::LogEvent;
use crate::logging::record;
use crate::mutex;

pub const RATE_LIMIT_PRUNE_AT_INVALID: &str = "RATE_LIMIT_PRUNE_AT_INVALID";

pub const UNKNOWN_WAIT: Duration = Duration::from_secs(3600);

#[derive(Clone)]
pub struct RateLimiter {
    capacity: f64,
    refill_per_sec: f64,
    prune_at: usize,
    pub buckets: Arc<Mutex<HashMap<Option<IpAddr>, Bucket>>>,
}

impl RateLimiter {
    pub fn new(capacity: u32, refill_per_sec: f64, prune_at: usize) -> Result<Self, Exception> {
        if prune_at == 0 {
            return Err(exception!(
                code = RATE_LIMIT_PRUNE_AT_INVALID,
                "rate limiter prune_at must be above zero",
            ));
        }
        Ok(Self {
            capacity: f64::from(capacity),
            refill_per_sec: refill_per_sec.max(0.0),
            prune_at,
            buckets: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub fn check(&self, key: Option<IpAddr>, now: Instant) -> Result<(), Exception> {
        let mut buckets = mutex::lock(&self.buckets);
        self.make_room(&mut buckets, key, now);
        let bucket = buckets.entry(key).or_insert(Bucket {
            tokens: self.capacity,
            updated: now,
        });
        bucket.tokens = self.refilled(bucket, now);
        bucket.updated = now;
        if bucket.tokens < 1.0 {
            return Err(exception!(
                code = error_code::TOO_MANY_REQUESTS,
                "too many requests",
            ));
        }
        bucket.tokens -= 1.0;
        Ok(())
    }

    pub fn retry_after(&self, key: Option<IpAddr>, now: Instant) -> Duration {
        if self.refill_per_sec <= 0.0 {
            return UNKNOWN_WAIT;
        }
        let buckets = mutex::lock(&self.buckets);
        let Some(bucket) = buckets.get(&key) else {
            return UNKNOWN_WAIT;
        };
        let tokens = self.refilled(bucket, now);
        if tokens >= 1.0 {
            return Duration::ZERO;
        }
        Duration::try_from_secs_f64((1.0 - tokens) / self.refill_per_sec).unwrap_or(UNKNOWN_WAIT)
    }

    fn make_room(
        &self,
        buckets: &mut HashMap<Option<IpAddr>, Bucket>,
        key: Option<IpAddr>,
        now: Instant,
    ) {
        if buckets.len() < self.prune_at || buckets.contains_key(&key) {
            return;
        }
        buckets.retain(|_, bucket| self.refilled(bucket, now) < self.capacity);
        if buckets.len() < self.prune_at {
            return;
        }
        let mut used: Vec<(Instant, Option<IpAddr>)> = buckets
            .iter()
            .map(|(key, bucket)| (bucket.updated, *key))
            .collect();
        let count = self.evict_count().min(used.len());
        used.select_nth_unstable_by_key(count - 1, |(updated, _)| *updated);
        for (_, key) in used.into_iter().take(count) {
            buckets.remove(&key);
        }
        record(
            LogEvent::warn("HTTP/rateLimitEvicted")
                .with_context("prune_at", self.prune_at as u64)
                .with_context("evicted", count as u64),
        );
    }

    pub fn evict_count(&self) -> usize {
        (self.prune_at / 10).max(1)
    }

    fn refilled(&self, bucket: &Bucket, now: Instant) -> f64 {
        let elapsed = now.saturating_duration_since(bucket.updated).as_secs_f64();
        (bucket.tokens + elapsed * self.refill_per_sec).min(self.capacity)
    }
}

pub async fn rate_limit(
    State(limiter): State<RateLimiter>,
    request: Request,
    next: Next,
) -> Response {
    let key = limit_key(client_ip(request.headers()).unwrap_or_default());
    let now = Instant::now();
    match limiter.check(key, now) {
        Ok(()) => next.run(request).await,
        Err(error) => {
            let seconds = limiter.retry_after(key, now).as_secs_f64().ceil().max(1.0) as u64;
            let mut response = error.into_response();
            response
                .headers_mut()
                .insert(header::RETRY_AFTER, HeaderValue::from(seconds));
            response
        }
    }
}

pub struct Bucket {
    tokens: f64,
    updated: Instant,
}

pub fn limit_key(ip: &str) -> Option<IpAddr> {
    match ip.parse::<IpAddr>().ok()? {
        IpAddr::V6(v6) => {
            let mut octets = v6.octets();
            octets[8..].fill(0);
            Some(IpAddr::V6(Ipv6Addr::from(octets)))
        }
        v4 => Some(v4),
    }
}
