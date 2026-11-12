# HTTP errors and request headers

This document describes these items in the framework:

- The HTTP boundary error.
- The bearer API key check.
- The cookie header helpers.
- The same-origin check and the same-origin middleware.
- The rate limit middleware.
- The security header middleware and the no-store middleware.
- The API key middleware.
- The request timeout and body limit middleware.
- The process version route.
- The health route.

This feature is not product business logic.

## Sub-features

| Sub-feature | Document |
| --- | --- |
| HTTP boundary error | `spec/framework_rs/http/api-error.md` |
| Bearer API key check | `spec/framework_rs/http/bearer-key.md` |
| Cookie header helpers | `spec/framework_rs/http/cookie.md` |
| Same-origin check and same-origin middleware | `spec/framework_rs/http/same-origin.md` |
| Rate limit middleware | `spec/framework_rs/http/rate-limit.md` |
| Security header middleware and no-store middleware | `spec/framework_rs/http/response-headers.md` |
| API key middleware | `spec/framework_rs/http/api-key-middleware.md` |
| Request timeout and body limit middleware | `spec/framework_rs/http/limits.md` |
| Process version route | `spec/framework_rs/http/version-route.md` |
| Health route | `spec/framework_rs/http/health-route.md` |

## Crate

- The package name is `ian_framework`.
- The crate path is `framework_rs/`.
- The module path is `ian_framework::http`.
- The header helper module path is `ian_framework::http::headers`.
- The router module path is `ian_framework::http::router`.

## Public items

`ian_framework::http` exports only these items:

- The module `error_code`
- The module `headers`
- The module `middleware`
- The module `router`

`ian_framework::http::error_code` exports only these items:

- `VALIDATION_ERROR`
- `BAD_REQUEST`
- `UNAUTHORIZED`
- `FORBIDDEN`
- `NOT_FOUND`
- `REQUEST_TIMEOUT`
- `PAYLOAD_TOO_LARGE`
- `TOO_MANY_REQUESTS`
- `INTERNAL_ERROR`

`ian_framework::http::middleware` exports only these items:

- `body_limit` and `BodyLimit`
- `access_log`
- `no_store`
- `rate_limit` and `RateLimiter`
- `request_timeout` and `RequestTimeout`
- `require_api_key` and `ApiKey`
- `require_same_origin`
- `security_headers`
- `require_session`, only with the Cargo feature `auth`

`ian_framework::http::headers` exports only these items:

- `bearer_token`
- `client_ip` and `user_agent`
- `read_cookie` and `sets_cookie`
- `is_same_origin`

`ian_framework::http::router` exports only these items:

- `health_router`
- `version_router`

Each item has only one public path.

## Build

This module has no Cargo feature. Each build compiles this module.

This module uses these dependencies:

- `axum`
- `getrandom`
- `hex`
- `http-body-util`
- `serde`
- `serde_json`
- `sha2`
- `subtle`

Related documents:

- The token68 check is in `spec/framework_rs/token/context.md`.
- The logging module is in `spec/framework_rs/logging/context.md`.
- The Google Cloud Storage client is in `spec/framework_rs/gcs/context.md`.
- Cookie sessions and password hashes are in `spec/framework_rs/auth/context.md`.

Put this block in the project `Cargo.toml`:

```toml
[dependencies]
ian_framework = { git = "https://github.com/ianlauyin/ian-framework", rev = "<commit>" }
```

The crate is not on crates.io. A project adds the crate as a git dependency. The project pins `rev` to one commit.

Use these commands in `framework_rs/`:

```sh
cargo check
cargo test
```

| Command | Effect |
| --- | --- |
| `cargo check` | Checks the build with no optional feature. |
| `cargo test` | Runs the tests of this module and of the logging module. |

## Out of scope

This feature does not include these items:

- Authentication other than the bearer API key check.
- Cookie sessions. Cookie sessions are in `spec/framework_rs/auth/session-store.md`.
- An API key store or API key rotation.
- A catalog of product error codes.
- A project error enum.
- A project procedure.
- A project spec.
- `Box<dyn std::error::Error>` in the handler error path.
