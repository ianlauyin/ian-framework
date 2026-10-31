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
| `ip` | The first source that is not empty. The sources are below. |
| `user_agent` | The header `user-agent`. A missing header gives an empty `user_agent`. |
| `latency_ms` | The handler time in milliseconds. It includes the body read. |
| `level` | From the response status. The rules are below. |
| `extra` | The keys `query` and `body`. The rules are below. |

The `ip` sources are in this order:

1. The first item in the header `x-forwarded-for`.
2. The header `x-real-ip`.
3. The host of `ConnectInfo<SocketAddr>`.

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

### Extra

`extra` has only these keys:

| Key | Value |
| --- | --- |
| `query` | One object with each query field, when the query has a field. |
| `body` | One object with each body field, when the body has a field. |

Query rules:

- The middleware leaves out `query` when the request has no query or the query has no field.
- The middleware reads the query as `application/x-www-form-urlencoded` text. It decodes each name and each value.
- A query field without `=` has an empty string value.
- Each query value is a string.
- A query name that occurs more than one time has one array of its string values, in query order.

The middleware reads the body only when all these rules are true:

- The header `content-length` is a number from 1 through 65536.
- The media type in the header `content-type` is `application/json` or a type that ends with `+json`. The match ignores case and the parameters after `;`.

Body rules:

- `body` is the JSON object.
- The middleware keeps each nested value.
- A JSON body that is not an object gives no `body`.
- A body that is not valid JSON gives no `body`.
- The middleware does not log a body of a different media type.
- An empty object gives no `body`.
- The middleware gives the same body bytes to the inner handler.
- The middleware does not read a different body. Thus a large body or a body without `content-length` goes to the handler as a stream.
- When the body read fails, the middleware does not call the inner handler. It returns status 400 and records the event with status 400.
- The shipper masks `query` and `body` with the masking rules. The masking rules are in `spec/framework_rs/logging/masking.md`. `query` and `body` are nested objects, so the key rule applies to their keys.
