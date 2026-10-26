# HTTP errors and request headers

This document describes these items in the framework:

- The HTTP boundary error.
- The bearer API key check.
- The cookie header helpers.
- The same-origin check and the same-origin middleware.
- The process version route.

This feature is not product business logic.

## Crate

- The package name is `ian_framework`.
- The crate path is `framework_rs/`.
- The module path is `ian_framework::http`.
- `rust-toolchain.toml` gives the Rust version for this repository.

## Build

This module has no Cargo feature. Each build compiles this module.

This module uses these dependencies:

- `axum`
- `getrandom`
- `hex`
- `serde`
- `serde_json`
- `sha2`
- `subtle`

Related documents:

- The logging module is in `spec/framework_rs/logging.md`.
- The Google Cloud Storage client is in `spec/framework_rs/gcs.md`.
- Cookie sessions and password hashes are in `spec/framework_rs/auth.md`.

Put this block in the project `Cargo.toml`:

```toml
[dependencies]
ian_framework = "0.1"
```

Use these commands in `framework_rs/`:

```sh
cargo check
cargo test
```

| Command | Effect |
| --- | --- |
| `cargo check` | Checks the build with no optional feature. |
| `cargo test` | Runs the tests of this module and of the logging module. |

## ApiError

`ApiError` is the HTTP boundary error.

- Only `ApiError` in this module implements `IntoResponse`.
- A domain error does not implement `IntoResponse`.
- An I/O error does not implement `IntoResponse`.
- `ApiError` does not implement `std::error::Error`.
- `ApiError` does not store `Box<dyn std::error::Error>`.

`ApiError` has these fields. A caller can read each field.

| Field | Type | Value |
| --- | --- | --- |
| `status` | `axum::http::StatusCode` | The HTTP status. |
| `code` | `String` | The stable error code. |
| `message` | `String` | The error text. |

The display text is the error code, then `: `, then the message.

### Constructors

| Function | Arguments, in order | Result |
| --- | --- | --- |
| `ApiError::new` | status, error code, message | One `ApiError`. |
| `ApiError::from_status_code` | status number, error code, message | One `ApiError` from a status number. |

`ApiError::from_status_code` uses these rules:

- A status number from 100 through 999 stays that status.
- Each other status number, zero included, becomes status 500.
- The error code does not change.
- The message does not change.

## JSON response

`IntoResponse` for `ApiError` builds the HTTP response.

- The response status is the `status` field.
- The content type is `application/json`.
- The body is one flat JSON object.
- The body has no `error` key.
- The body has only the keys in the table below.

| Key | Value |
| --- | --- |
| `code` | The error code. |
| `message` | The message. |

## Trait IntoApiError

A project error type implements `IntoApiError`. The trait has no Axum type.

The trait has these methods:

| Method | Result |
| --- | --- |
| `status_code` | The status number, `u16`. |
| `error_code` | The stable error code, `&str`. |
| `message` | The error text, `String`. |

`ApiError` implements `From` for each type that implements `IntoApiError`. `ApiError::from` uses these rules:

- It reads `status_code`, `error_code`, and `message`.
- It uses the status rule of `ApiError::from_status_code`. Thus an invalid status number becomes status 500.
- The error code does not change.
- The message does not change.

A handler can return `Result<T, ApiError>`. These two forms convert a project error into `ApiError`:

- The `?` operator.
- `map_err(ApiError::from)`.

## Bearer API key

A server can use a bearer API key to authenticate a request.

The module has these functions:

| Function | Result |
| --- | --- |
| `bearer_token(headers: &axum::http::HeaderMap)` | The token, `Option<&str>`. |
| `key_matches(token: &str, key: &str)` | The match result, `bool`. |

### bearer_token

`bearer_token` reads the `Authorization` header.

- The header value must be the scheme `Bearer`, then one space, then the token.
- The scheme match is not case sensitive.
- The token is an RFC 9110 `token68`. It has one or more of the characters `A`-`Z`, `a`-`z`, `0`-`9`, `-`, `.`, `_`, `~`, `+`, `/`. Then it has zero or more `=`.

`bearer_token` gives `None` in these conditions:

- The request has no `Authorization` header.
- The header value is not UTF-8 text.
- The scheme is not `Bearer`.
- The header value has no space.
- The token is not a `token68`.

### key_matches

- `key_matches` makes the SHA-256 hash of the token and the SHA-256 hash of the key.
- It compares the two hashes in constant time with `subtle::ConstantTimeEq`.
- The comparison time does not depend on the position of the first different byte.
- The comparison time does not depend on the length of the key.
- `key_matches` gives `true` only if the token and the key are equal.
- `key_matches` gives `false` when the key is empty. Thus a key that is not set does not match.

### Project tasks

- The project keeps the API key.
- The project selects the HTTP status and the error for a rejected request.

## Cookie headers

The module has these functions:

| Function | Result |
| --- | --- |
| `read_cookie(headers: &axum::http::HeaderMap, name: &str)` | The cookie value, `Option<&str>`. |
| `sets_cookie(headers: &axum::http::HeaderMap, name: &str)` | The match result, `bool`. |

`read_cookie` reads each `Cookie` request header. An HTTP/2 client can send each cookie in a different `Cookie` header.

- `read_cookie` returns the value of the first cookie with the name `name`.
- It skips a `Cookie` header that is not UTF-8 text.
- It returns `None` when no cookie has the name.
- It does not remove quotes from the value.

`sets_cookie` reads each `Set-Cookie` response header. It returns `true` when one of them sets the cookie with the name `name`.

Cookie sessions use these functions. Cookie sessions are in `spec/framework_rs/auth.md`.

## Same-origin check

The module has this function:

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
- The client log route uses this middleware. The client log route is in `spec/framework_rs/logging.md`.

## Process version route

- The source file is `framework_rs/src/http/version.rs`.
- `ian_framework::http::version_router()` gives an `axum::Router` with one route.
- The route is `GET /version`.
- The project merges this router into its own router.
- The project adds its own layers to the merged router.
- The route has no authentication.
- The client code that reads the route is in `spec/framework_fe/version.md`.

The response:

- The response status is 200.
- The content type is `application/json`.
- The body is one flat JSON object with the keys in the table below.

| Key | Value |
| --- | --- |
| `version` | The process version, a string. |

The process version:

- The process version is 16 random bytes from `getrandom`, in lowercase hex. It has 32 characters.
- The first request makes the process version.
- Each later request in the same process gets the same value.
- A new process makes a new value.
- A client compares the value with the last value. When the value changes, a new server process is running.
- The process version is not the build version or the release version of the project.

When `getrandom` fails, the response status is 500 and the body is empty. The next request tries again.

## Out of scope

This feature does not include these items:

- Authentication other than the bearer API key check.
- Cookie sessions. Cookie sessions are in `spec/framework_rs/auth.md`.
- An API key store or API key rotation.
- A catalog of product error codes.
- A project error enum.
- A project procedure.
- A project spec.
- `Box<dyn std::error::Error>` in the handler error path.
