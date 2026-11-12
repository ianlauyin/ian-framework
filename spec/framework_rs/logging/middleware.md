# Axum middleware

The parts that apply to all of this feature are in `spec/framework_rs/logging/context.md`.

## Axum middleware

`ian_framework::http::middleware::access_log` is the Axum middleware function. The source file is `framework_rs/src/http/middleware/access_log.rs`. Use it with `axum::middleware::from_fn_with_state` and one logger.

General rules:

- The middleware does not read the header `x-request-id`.
- The middleware does not write the header `x-request-id`.
- The middleware does not set the event field `id`. The event constructor gives a new UUID v7 text.
- The middleware records one event after the inner handler returns.
- The middleware does not set `app` on the event.
- A log failure does not change the HTTP response.

### Event fields

| Field | Value |
| --- | --- |
| `action` | `API/{method}:{path}`. `method` is the request method. `path` is the request path, with no query string. An empty path is `-`. |
| `latency_ms` | The handler time in milliseconds. |
| `level` | From the response status. The rules are below. |
| `error_message` | `response status {status}` when the level is `error`. Else null. |
| `context` | The keys `ip`, `user_agent` and `request_url`. The rules are below. |
| `extra` | Empty. The middleware does not read the request body. |

### Context

`context` has only these keys:

| Key | Value |
| --- | --- |
| `ip` | The header `cf-connecting-ip`, when it holds an IP address. The rules are below. |
| `user_agent` | The header `user-agent`. A missing or empty header gives `-`. |
| `request_url` | The request path with the query string, as the request line gives it. An empty path is `-`. |

Cloudflare sets the header `cf-connecting-ip` to the real client IP. It is the only source of `ip`.

- `ian_framework::http::headers::client_ip(headers)` gives the client IP, `Option<&str>`. `ian_framework::http::headers::user_agent(headers)` gives the user agent, `Option<&str>`. Each function reads one header of a `HeaderMap`. The middleware, the rate limit middleware and the client log route use them. When a function gives no value, the middleware uses `-`, and the other callers use an empty string.

- The framework supports only one deployment: Cloudflare in front of a reverse proxy, and the reverse proxy in front of the process.
- The middleware does not read `x-forwarded-for`, `x-real-ip` or the socket address.
- The project must make the process reachable only through Cloudflare. A firewall that allows only the Cloudflare IP ranges does this. A client that reaches the process directly can send any `cf-connecting-ip`.

Header rules:

- The middleware removes spaces at the start and at the end of a header value.
- A header value that is not an IP address gives the `ip` `-`.
- A request without the header gets the `ip` `-`.
- The middleware skips a header value that is not text.

Level rules:

| Status | Level |
| --- | --- |
| 500 or higher | `error` |
| 400 through 499 | `warn` |
| Below 400 | `info` |

The logger masks `context` with the masking rules. The masking rules are in `spec/framework_rs/logging/masking.md`. The pattern rule applies to the text of `request_url`.
