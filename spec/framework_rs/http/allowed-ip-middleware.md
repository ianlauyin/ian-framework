# Allowed IP middleware

The parts that apply to all of this feature are in `spec/framework_rs/http/context.md`. The client IP rules are in `spec/framework_rs/logging/middleware.md`.

## Allowed IP middleware

- The source file is `framework_rs/src/http/middleware/require_allowed_ip.rs`.
- `ian_framework::http::middleware::require_allowed_ip` is an Axum middleware function.
- The project adds it to a router with `axum::middleware::from_fn_with_state` and one `AllowedIps`.
- `AllowedIps::new(ips)` makes the set. Each item is an IP address text. A clone shares the set.
- `AllowedIps::new` returns `Result<AllowedIps, Exception>`. When one item is not an IP address, it returns an exception with the code `ALLOWED_IPS_INVALID`.
- Each request that goes through the layer needs an allowed IP. The middleware has no path rule, no method rule and no public path. To leave a route without the check, the project does not put the layer on that route.

## Check

- The middleware reads the client IP with `client_ip`. Only the header `cf-connecting-ip` gives the IP. The rules are in `spec/framework_rs/logging/middleware.md`.
- When the client IP is in the set, the middleware passes the request to the next handler.
- When the client IP is missing, empty, not an IP address, or not in the set, the middleware does not call the next handler. The response has status 403 and the message `ip not allowed`.
- An empty set rejects every request.
- Before the 403 response, the middleware calls `Logger::warn` with the action `HTTP/ipRejected` and the context keys `method` and `ip`. The `ip` value is the header text when the header holds an IP address, or `-` when it does not. The logger rules are in `spec/framework_rs/logging/logger.md`.

The 403 response body is:

```json
{"message":"ip not allowed"}
```

Rules:

- The middleware writes one `warn` event with the request method and the client IP for each 403.
- The project must make the process reachable only through Cloudflare. A client that reaches the process directly can send any `cf-connecting-ip`.
