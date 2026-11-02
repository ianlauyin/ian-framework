# Rate limit middleware

The parts that apply to all of this feature are in `spec/framework_rs/http/context.md`.

## Rate limit middleware

- The source file is `framework_rs/src/http/middleware/rate_limit.rs`.
- `ian_framework::http::middleware::rate_limit` is an Axum middleware function.
- The project adds it to a router with `axum::middleware::from_fn_with_state` and one `RateLimiter`.

| Type | Meaning |
| --- | --- |
| `RateLimitConfig { capacity: u32, refill_per_sec: f64 }` | The size of one bucket and its refill speed. `RateLimitConfig::new` makes it. |
| `RateLimiter` | The buckets. `RateLimiter::new(config)` makes it. A clone shares the same buckets. |

## Algorithm

- The limiter is a token bucket for each key.
- The key is the client IP. The middleware reads `ClientIp` from the request extensions. When there is no `ClientIp`, it uses the `Headers` rules of `spec/framework_rs/http/client-ip.md`.
- A request with an empty IP shares one bucket with all other requests that have an empty IP.
- A new bucket is full. Its tokens are `capacity`.
- Each request takes one token. The bucket gets `refill_per_sec` tokens each second, up to `capacity`.
- A `refill_per_sec` below zero is the same as zero. With zero, a bucket never refills.
- A request with no whole token left is refused.
- When the map holds 10 000 keys or more, and a request has a new key, the limiter first deletes each bucket that is full again.
- If the map still holds 10 000 keys after that, the limiter also deletes the bucket that was used longest ago. Thus the map does not grow past 10 000 keys.
- The state is in the memory of the process. Each limiter has its own state. A restart clears it.

## Refused request

The response is an `ApiError` with status 429, the error code `rate_limited`, and the message `too many requests`. The body is:

```json
{"code":"rate_limited","message":"too many requests"}
```

The response has the header `Retry-After`. The value is the wait for one token, in whole seconds, rounded up. The minimum is 1. When the bucket never refills, the value is 3600.

Usage rules:

- The middleware counts every request, also a request that a later middleware rejects. Thus a request with a wrong password or a wrong API key uses a token.
- To use a stricter limit on one route, add a second `RateLimiter` to that route only.
- Put `rate_limit` outside `require_api_key`.
- Put `rate_limit` inside `set_client_ip`.
