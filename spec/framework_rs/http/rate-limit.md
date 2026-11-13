# Rate limit middleware

The parts that apply to all of this feature are in `spec/framework_rs/http/context.md`.

## Rate limit middleware

- The source file is `framework_rs/src/http/middleware/rate_limit.rs`.
- `ian_framework::http::middleware::rate_limit` is an Axum middleware function.
- The project adds it to a router with `axum::middleware::from_fn_with_state` and one `RateLimiter`.

| Type | Meaning |
| --- | --- |
| `RateLimiter` | The buckets. `RateLimiter::new(capacity: u32, refill_per_sec: f64, prune_at: usize)` makes it. `capacity` is the size of one bucket, `refill_per_sec` is its refill speed and `prune_at` is the most keys that the limiter holds. A clone shares the same buckets. |

`RateLimiter::new` returns `Result<RateLimiter, Exception>`. When `prune_at` is zero, it returns an exception with the code `RATE_LIMIT_PRUNE_AT_INVALID`.

## Algorithm

- The limiter is a token bucket for each key.
- The key is the client IP. The middleware reads it with the `ip` rules of `spec/framework_rs/logging/middleware.md`. Only the header `cf-connecting-ip` gives the IP.
- When the client IP is an IPv6 address, the key is its first 64 bits (the /64 network), because one client can own a whole /64 network. An IPv4 address is the key as it is.
- A request with an empty IP shares one bucket with all other requests that have an empty IP.
- A new bucket is full. Its tokens are `capacity`.
- Each request takes one token. The bucket gets `refill_per_sec` tokens each second, up to `capacity`.
- A `refill_per_sec` below zero is the same as zero. With zero, a bucket never refills.
- A request with no whole token left is refused.
- When the map holds `prune_at` keys or more, and a request has a new key, the limiter first deletes each bucket that is full again.
- If the map still holds `prune_at` keys after that, the limiter also deletes the buckets that were used longest ago. It deletes `prune_at / 10` buckets, with a minimum of 1. The limiter scans the map only in this case, so the scan occurs at most one time for each `prune_at / 10` new keys. Thus the map does not grow past `prune_at` keys.
- After it deletes the buckets that were used longest ago, the limiter calls `Logger::warn` with the action `HTTP/rateLimitEvicted` and the context keys `prune_at` and `evicted`. This warning shows that `prune_at` is too small for the traffic: the deleted buckets were not full, so their clients get a full bucket again. The logger rules are in `spec/framework_rs/logging/logger.md`.
- The state is in the memory of the process. Each limiter has its own state. A restart clears it.

## Refused request

The response has status 429 and the message `too many requests`. The body is:

```json
{"message":"too many requests"}
```

The response has the header `Retry-After`. The value is the wait for one token, in whole seconds, rounded up. The minimum is 1. When the bucket never refills, the value is 3600.

Usage rules:

- The middleware counts every request, also a request that a later middleware rejects. Thus a request with a wrong password or a wrong API key uses a token.
- To use a stricter limit on one route, add a second `RateLimiter` to that route only.
- Put `rate_limit` outside `require_api_key`.
