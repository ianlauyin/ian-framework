# HTTP errors and request headers

This document describes the HTTP boundary error, the bearer API key check, the cookie header helpers, the same-origin check, the same-origin middleware, and the process version route in the framework.
This feature is not product business logic.

## Crate

The package name is `ian_framework`.
The crate path is `framework_rs/`.
The module path is `ian_framework::http`.
`rust-toolchain.toml` gives the Rust version for this repository.

## Build

This module has no Cargo feature.
Each build compiles this module.
This module uses these dependencies:

- `axum`
- `getrandom`
- `hex`
- `serde`
- `serde_json`
- `sha2`
- `subtle`

The logging module is `spec/framework_rs/logging.md`.
The Google Cloud Storage client is `spec/framework_rs/gcs.md`.
Cookie sessions and password hashes are `spec/framework_rs/auth.md`.

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

`cargo check` checks the build with no optional feature.
`cargo test` runs the tests of this module and of the logging module.

## ApiError

`ApiError` is the HTTP boundary error.
Only `ApiError` in this module implements `IntoResponse`.
A domain error does not implement `IntoResponse`.
An I/O error does not implement `IntoResponse`.
`ApiError` does not implement `std::error::Error`.
`ApiError` does not store `Box<dyn std::error::Error>`.

`ApiError` has these fields:

| field | type |
| --- | --- |
| `status` | `axum::http::StatusCode` |
| `code` | `String` |
| `message` | `String` |

A caller can read `status`, `code`, and `message`.
`code` is the stable error code.
`message` is the error text.
The display text is the error code, then `: `, then the message.

`ApiError::new` builds one `ApiError`.
The first argument is the status.
The second argument is the error code.
The third argument is the message.

`ApiError::from_status_code` builds one `ApiError` from a status number.
The first argument is the status number.
The second argument is the error code.
The third argument is the message.
A status number from 100 through 999 stays that status.
A status number of zero becomes status 500.
Each other status number becomes status 500.
The error code stays the same.
The message stays the same.

## JSON response

`IntoResponse` for `ApiError` builds the HTTP response.
The response status is the `status` field.
The content type is `application/json`.
The body is one flat JSON object.
The body has these fields:

| field | value |
| --- | --- |
| `code` | the error code |
| `message` | the message |

The body has no `error` key.
The body has no other key.
An example body is `{"code":"not_found","message":"missing"}`.

## Trait IntoApiError

A project error type implements `IntoApiError`.
The trait has no Axum type.
The trait has these methods:

| method | result |
| --- | --- |
| `status_code` | status number, `u16` |
| `error_code` | stable error code, `&str` |
| `message` | error text, `String` |

`ApiError` implements `From` for each type that implements `IntoApiError`.
`ApiError::from` reads `status_code`, `error_code`, and `message`.
An invalid status number becomes status 500.
The status rule is the same as `ApiError::from_status_code`.
The error code stays the same.
The message stays the same.

A handler can return `Result<T, ApiError>`.
The `?` operator converts a project error into `ApiError`.
`map_err(ApiError::from)` converts a project error into `ApiError`.

`load_item` in the example returns `Result<String, ItemError>`.

```rust
enum ItemError {
    Missing,
}

impl ian_framework::http::IntoApiError for ItemError {
    fn status_code(&self) -> u16 {
        match self {
            ItemError::Missing => 404,
        }
    }

    fn error_code(&self) -> &str {
        match self {
            ItemError::Missing => "not_found",
        }
    }

    fn message(&self) -> String {
        match self {
            ItemError::Missing => "missing".to_string(),
        }
    }
}

async fn read_item(id: &str) -> Result<String, ian_framework::http::ApiError> {
    let item = load_item(id)?;
    Ok(item)
}
```

## Bearer API key

A server can use a bearer API key to authenticate a request.
The module has these functions:

| function | result |
| --- | --- |
| `bearer_token(headers: &axum::http::HeaderMap)` | the token, `Option<&str>` |
| `key_matches(token: &str, key: &str)` | the match result, `bool` |

`bearer_token` reads the `Authorization` header.
The header value must be the scheme `Bearer`, then one space, then the token.
The token is an RFC 9110 `token68`.
It has one or more of the characters `A`-`Z`, `a`-`z`, `0`-`9`, `-`, `.`, `_`, `~`, `+`, `/`, then zero or more `=`.
`bearer_token` gives `None` in these conditions:

- The request has no `Authorization` header.
- The header value is not UTF-8 text.
- The scheme is not `Bearer`. The scheme is not case sensitive, so `bearer` is also `Bearer`.
- The header value has no space.
- The token is not a `token68`. For example, the token is empty, or it contains a space or a tab.

`key_matches` makes the SHA-256 hash of the token and of the key.
It compares the two hashes in constant time with `subtle::ConstantTimeEq`.
The comparison time does not depend on the position of the first different byte.
The comparison time does not depend on the length of the key.
`key_matches` gives `true` only if the token and the key are equal.
`key_matches` gives `false` when the key is empty, so an unset key does not match.

The project keeps the API key.
The project selects the HTTP status and the error for a rejected request.

```rust
fn authorize(headers: &axum::http::HeaderMap, api_key: &str) -> bool {
    ian_framework::http::bearer_token(headers)
        .is_some_and(|token| ian_framework::http::key_matches(token, api_key))
}
```

## Cookie headers

The module has these functions:

| function | result |
| --- | --- |
| `read_cookie(headers: &axum::http::HeaderMap, name: &str)` | the cookie value, `Option<&str>` |
| `sets_cookie(headers: &axum::http::HeaderMap, name: &str)` | the match result, `bool` |

`read_cookie` reads each `Cookie` request header.
An HTTP/2 client can send each cookie in its own `Cookie` header.
`read_cookie` returns the value of the first cookie with the name `name`.
It skips a `Cookie` header that is not UTF-8 text.
It returns `None` when no cookie has the name.
It does not remove quotes from the value.

`sets_cookie` reads each `Set-Cookie` response header.
It returns `true` when one of them sets the cookie with the name `name`.

Cookie sessions use these functions. Cookie sessions are `spec/framework_rs/auth.md`.

## Same-origin check

The module has this function:

| function | result |
| --- | --- |
| `is_same_origin(headers: &axum::http::HeaderMap)` | the check result, `bool` |

`is_same_origin` returns `true` only in these conditions:

- When the request has the header `Sec-Fetch-Site`, the value is `same-origin`.
- When the request has no `Sec-Fetch-Site`, the request has the headers `Origin` and `Host`, and the part of `Origin` after `://` equals `Host`. The comparison is not case sensitive.

A browser that does not send `Sec-Fetch-Site` still sends `Origin` on a `POST`.

## Same-origin middleware

The source file is `framework_rs/src/http/same_origin.rs`.
`ian_framework::http::require_same_origin` is an Axum middleware function.
The project adds it to a router with `axum::middleware::from_fn`.

```rust
let router = axum::Router::new()
    .route("/items", axum::routing::post(create_item))
    .layer(axum::middleware::from_fn(ian_framework::http::require_same_origin));
```

The middleware calls `is_same_origin` for each request.
When `is_same_origin` returns `true`, the middleware passes the request to the next handler.
When `is_same_origin` returns `false`, the middleware does not call the next handler.
The response is an `ApiError` with status 403, the error code `cross_origin`, and the message `cross-origin request`.
The body is `{"code":"cross_origin","message":"cross-origin request"}`.

The middleware checks each HTTP method.
A browser navigation that the user types in the address bar sends `Sec-Fetch-Site: none`, so the middleware rejects it.
Put the middleware only on the API routes. Do not put it on the page routes.
The client log route uses this middleware. The client log route is `spec/framework_rs/logging.md`.

## Process version route

The source file is `framework_rs/src/http/version.rs`.
`ian_framework::http::version_router()` gives an `axum::Router` with one route.
The route is `GET /version`.
The project merges this router into its own router.
The project adds its own layers to the merged router.

The response status is 200.
The content type is `application/json`.
The body is one flat JSON object.
The body has these fields:

| field | value |
| --- | --- |
| `version` | the process version, a string |

An example body is `{"version":"3f2a9c0e5b7d41a8a6c1e2f3d4b5a697"}`.

The process version is 16 random bytes from `getrandom`, in lowercase hex.
The process version has 32 characters.
The first request makes the process version.
Each later request in the same process gets the same value.
A new process makes a new value.
A client compares the value with the last value. When the value changes, a new server process is running.
The process version is not the build version or the release version of the project.
The route has no authentication.
The client code that reads the route is `spec/framework_fe/version.md`.

When `getrandom` fails, the response status is 500 and the body is empty.
The next request tries again.

## Out of scope

This feature does not include these items:

- Authentication other than the bearer API key check.
- Cookie sessions. Cookie sessions are `spec/framework_rs/auth.md`.
- An API key store or API key rotation.
- A catalog of product error codes.
- A project error enum.
- A project procedure.
- A project spec.
- `Box<dyn std::error::Error>` in the handler error path.
