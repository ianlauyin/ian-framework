# Axum middleware

The parts that apply to all of this feature are in `spec/framework_rs/logging/context.md`.

## Axum middleware

`ian_framework::http::middleware::access_log` is the Axum middleware function. The source file is `framework_rs/src/http/middleware/access_log.rs`. Use it with `axum::middleware::from_fn`.

General rules:

- The middleware records the event with the global logger from `Logger::get()`. When no global logger is set, the middleware does not record the event. The logger rules are in `spec/framework_rs/logging/logger.md`.
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
| `error_message` | The error detail. The rules are below. |
| `context` | The keys that the handler adds, and the keys `ip`, `user_agent` and `request_url`. The rules are below. |
| `extra` | The keys that the handler adds. The middleware does not read the request body. |

### Error detail

The error detail goes to the log. It never goes to the client.

| Cause | Level | `error_message` |
| --- | --- | --- |
| The handler returns an `Exception` with status 500 or higher | `error` | The display text of the exception, a line break, then the trace text. This is the same text as `Logger::error`. The trace text is in `spec/framework_rs/exception.md`. |
| The handler returns an `Exception` with status 400 through 499 | `warn` | The same text as for status 500. |
| A different response with status 500 or higher | `error` | `response status {status}` |
| A different response with status 400 through 499 | `warn` | Null |
| A response with a status below 400 | `info` | Null |

- `IntoResponse` for `Exception` puts the exception in the response extension `Arc<Exception>`. The rules are in `spec/framework_rs/http/api-error.md`. The middleware removes the extension from the response.
- The logger does not mask `error_message`. The masking rules are in `spec/framework_rs/logging/masking.md`.

### Handler fields

`ian_framework::http::middleware::AccessLog` is an Axum extractor. A handler uses it to add keys to the event of its request.

- `log.context(key, value)` adds one key to `context`. `log.extra(key, value)` adds one key to `extra`. The value is a `LogValue`, or a type that converts to one.
- The middleware puts one `AccessLog` in the request extensions before it calls the handler. Each clone of it writes to the same event.
- The middleware adds the keys after the handler returns. It adds them for each status, also for an error response.
- A later call with the same key replaces the value.
- The middleware writes `ip`, `user_agent` and `request_url` after the handler keys. A handler cannot replace them.
- Without the middleware, the extractor gives a new `AccessLog` that no event uses. The extractor does not fail.

### Context

The middleware writes these keys to `context`:

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

The logger masks `context` and `extra` with the masking rules. The masking rules are in `spec/framework_rs/logging/masking.md`. The pattern rule applies to the text of `request_url`.
