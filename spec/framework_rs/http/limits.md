# Request limit middleware

The parts that apply to all of this feature are in `spec/framework_rs/http/context.md`.

Each of these middleware functions returns an `ApiError`, so the body is always the flat JSON of `spec/framework_rs/http/api-error.md`.

## Request timeout

- The source file is `framework_rs/src/http/middleware/request_timeout.rs`.
- `ian_framework::http::middleware::request_timeout` is an Axum middleware function.
- The project adds it to a router with `axum::middleware::from_fn_with_state` and one `RequestTimeout`.
- `RequestTimeout(pub Duration)` is the time limit.

The middleware gives the next handler the time limit. The limit counts from the call until the response is ready. It does not count the time to send the response body.

When the limit passes, the middleware drops the handler. The response is an `ApiError` with status 408, the error code `request_timeout`, and the message `request timed out`:

```json
{"code":"request_timeout","message":"request timed out"}
```

Rules:

- The limit is one total time. It is not an idle time. Do not put it on a route that receives a large upload. Use a limit that is long enough for the largest upload, or no limit.
- A route can have its own `RequestTimeout`. Put the middleware on that route only.

## Catch panic

- The source file is `framework_rs/src/http/middleware/catch_panic.rs`.
- `ian_framework::http::middleware::catch_panic` is an Axum middleware function.
- The project adds it to a router with `axum::middleware::from_fn`.

When the next handler panics, the middleware catches the panic. The response is an `ApiError` with status 500, the error code `internal_error`, and the message `internal error`:

```json
{"code":"internal_error","message":"internal error"}
```

Rules:

- The middleware calls `Logger::error` with the action `HTTP/handlerPanicked`. The context has the keys `method` and `path`, with the request method and the request path. The event does not have the panic message. The logger rules are in `spec/framework_rs/logging/logger.md`.
- The middleware does not spawn a task. The handler stops when the client closes the connection, as it does without the middleware.
- Put `catch_panic` inside `access_log`. Then `access_log` records the 500 response.
- The middleware does not catch a panic in a layer that is outside it.

## Body limit

- The source file is `framework_rs/src/http/middleware/body_limit.rs`.
- `ian_framework::http::middleware::body_limit` is an Axum middleware function.
- The project adds it to a router with `axum::middleware::from_fn_with_state` and one `BodyLimit`.
- `BodyLimit(pub usize)` is the maximum body size in bytes.

The middleware does these steps:

1. When the request has a `Content-Length` header that is larger than the limit, it does not call the next handler. It sends the 413 response.
2. It wraps the request body, so a read of more than the limit bytes fails. This applies to a body with no `Content-Length` and to a body larger than its `Content-Length`.
3. When the response has status 413 and its `Content-Type` does not start with `application/json`, it replaces the response with the 413 response. A response that already has a JSON body stays as it is.

The 413 response is an `ApiError` with status 413, the error code `payload_too_large`, and the message `request body is too large`:

```json
{"code":"payload_too_large","message":"request body is too large"}
```

Rules:

- Axum limits the body that an extractor reads to 2 MiB. The middleware does not change that limit.
- To use a limit above 2 MiB, or a body that the handler reads as a stream, add `axum::extract::DefaultBodyLimit::disable()` to the route. Then `body_limit` is the only limit.
- A route can have its own `BodyLimit`. Put the middleware on that route only.
