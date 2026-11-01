# Axum middleware

The parts that apply to all of this feature are in `spec/framework_rs/logging/context.md`.

## Axum middleware

`log_request` is the Axum middleware function. Use it with `axum::middleware::from_fn_with_state` and one shipper.

General rules:

- The middleware does not read the header `x-request-id`.
- The middleware does not write the header `x-request-id`.
- The middleware leaves the event field `id` empty.
- The middleware records one event after the inner handler returns.
- The middleware does not set `app` on the event.
- A log failure does not change the HTTP response.

### Event fields

| Field | Value |
| --- | --- |
| `action` | `API/{method}:{path}`. `method` is the request method. `path` is the request path, with no query string. An empty path is `/`. |
| `latency_ms` | The handler time in milliseconds. It includes the body read. |
| `level` | From the response status. The rules are below. |
| `context` | The keys `ip`, `user_agent` and `request_url`. The rules are below. |
| `info` | The keys `body.{path}`. The rules are below. |

### Context

`context` has only these keys:

| Key | Value |
| --- | --- |
| `ip` | The first source that is not empty. The sources are below. |
| `user_agent` | The header `user-agent`. A missing header gives an empty string. |
| `request_url` | The request path with the query string, as the request line gives it. An empty path is `/`. |

The `ip` sources are in this order:

1. The header `cf-connecting-ip`. Cloudflare sets it to the real client IP.
2. The first item in the header `x-forwarded-for`.
3. The header `x-real-ip`.
4. The host of `ConnectInfo<SocketAddr>`.

Header rules:

- The middleware removes spaces at the start and at the end of a header value.
- The host does not include the port.
- A request with no source gets an empty `ip`.
- The middleware skips a header value that is not text.

Level rules:

| Status | Level |
| --- | --- |
| 500 or higher | `error` |
| 400 through 499 | `warn` |
| Below 400 | `info` |

### Info

`info` has only the keys `body.{path}`. Each key has one scalar body value:

| Key | Value |
| --- | --- |
| `body.{path}` | One string, number, or `bool` of the body. `path` is the field names, and the array positions that start at 0, joined with `.`. |

The middleware reads the body only when all these rules are true:

- The header `content-length` is a number from 1 through 65536.
- The media type in the header `content-type` is `application/json` or a type that ends with `+json`. The match ignores case and the parameters after `;`.

Body rules:

- The body is one JSON object.
- The middleware writes each nested object and each array as keys with a longer `path`.
- The middleware leaves out a null value.
- A JSON body that is not an object gives no `body.{path}` key.
- A body that is not valid JSON gives no `body.{path}` key.
- The middleware does not log a body of a different media type.
- An empty object gives no `body.{path}` key.
- The middleware gives the same body bytes to the inner handler.
- The middleware does not read a different body. Thus a large body or a body without `content-length` goes to the handler as a stream.
- When the body read fails, the middleware does not call the inner handler. It returns status 400 and records the event with status 400.
- The shipper masks `context` and `info` with the masking rules. The masking rules are in `spec/framework_rs/logging/masking.md`. The key rule applies to the full key, which includes the names of the parents. The pattern rule applies to the text of `request_url`.
