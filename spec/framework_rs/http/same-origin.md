# Same-origin check

The parts that apply to all of this feature are in `spec/framework_rs/http/context.md`.

## Same-origin check

The module `ian_framework::http::headers` has this function:

| Function | Result |
| --- | --- |
| `is_same_origin(headers: &axum::http::HeaderMap)` | The check result, `bool`. |

`is_same_origin` returns `true` only in these conditions:

- The request has the header `Sec-Fetch-Site`, and the value is `same-origin`.
- The request has no `Sec-Fetch-Site`, the request has the headers `Origin` and `Host`, and the part of `Origin` after `://` is equal to `Host`. The comparison is not case sensitive.

A browser that does not send `Sec-Fetch-Site` still sends `Origin` on a `POST`.

## Same-origin middleware

- The source file is `framework_rs/src/http/same_origin.rs`.
- `ian_framework::http::require_same_origin` is an Axum middleware function.
- The project adds it to a router with `axum::middleware::from_fn`.

The middleware calls `is_same_origin` for each request:

- When `is_same_origin` returns `true`, the middleware passes the request to the next handler.
- When `is_same_origin` returns `false`, the middleware does not call the next handler. The response is an `ApiError` with status 403, the error code `cross_origin`, and the message `cross-origin request`.

The 403 response body is:

```json
{"code":"cross_origin","message":"cross-origin request"}
```

Usage rules:

- The middleware checks each HTTP method.
- A browser navigation that the user types in the address bar sends `Sec-Fetch-Site: none`. Thus the middleware rejects it.
- Put the middleware only on the API routes. Do not put it on the page routes.
- The client log route uses this middleware. The client log route is in `spec/framework_rs/logging/client-log.md`.
