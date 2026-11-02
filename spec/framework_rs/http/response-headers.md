# Response header middleware

The parts that apply to all of this feature are in `spec/framework_rs/http/context.md`.

## Security headers

- The source file is `framework_rs/src/http/middleware/security_headers.rs`.
- `ian_framework::http::middleware::security_headers` is an Axum middleware function.
- The project adds it to a router with `axum::middleware::from_fn`.

The middleware calls the next handler. Then it adds each header in this table to the response:

| Header | Value |
| --- | --- |
| `X-Content-Type-Options` | `nosniff` |
| `X-Frame-Options` | `DENY` |
| `Referrer-Policy` | `no-referrer` |
| `Strict-Transport-Security` | `max-age=31536000; includeSubDomains` |
| `Permissions-Policy` | `camera=(), microphone=(), geolocation=()` |
| `Cross-Origin-Opener-Policy` | `same-origin` |

Rules:

- When the response has a header of this table, the middleware keeps that header.
- The middleware does not set `Content-Security-Policy`. The CSP depends on the project pages.
- Use the middleware on the page routes and on the API routes.

## No store

- The source file is `framework_rs/src/http/middleware/no_store.rs`.
- `ian_framework::http::middleware::no_store` is an Axum middleware function.
- The project adds it to a router with `axum::middleware::from_fn`.

The middleware calls the next handler. When the response has no `Cache-Control` header, the middleware sets `Cache-Control: no-store`. When the response has the header, the middleware keeps it.

Use `no_store` on the API routes. Do not use it on the routes that serve files that a cache can keep.
