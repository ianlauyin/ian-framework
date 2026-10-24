# HTTP boundary error

This document describes the HTTP boundary error in the framework.
This feature is not product business logic.

## Crate

The package name is `ian_framework`.
The crate path is `framework/`.
The module path is `ian_framework::http`.
`rust-toolchain.toml` gives the Rust version for this repository.

## Feature `http`

The Cargo feature name is `http`.
`http` is a default feature.
A default build compiles this feature.
A project gets this feature with no extra feature flag.
The `http` feature enables these dependencies:

- `axum`
- `serde`
- `serde_json`

The `http` feature does not enable `logging`.
The `http` feature does not enable `gcs`.
The `logging` feature does not enable `http`.
The `gcs` feature does not enable `http`.
The `auth` feature enables `http`.
The logging feature is `spec/framework/logging.md`.
The Google Cloud Storage client is `spec/framework/gcs.md`.
Cookie sessions and password hashes are `spec/framework/auth.md`.
A project with default features off selects the feature `http` to compile this module.

Put this block in the project `Cargo.toml`:

```toml
[dependencies]
ian_framework = "0.1"
```

Use these commands in `framework/`:

```sh
cargo check
cargo test
cargo check --no-default-features
cargo test --no-default-features --features http
```

`cargo check` checks the default build.
The default build includes `http`.
The default build includes `logging`.
`cargo test` runs the default tests.
`cargo check --no-default-features` checks the build with no default features.
That build does not compile this module.
`cargo test --no-default-features --features http` runs the tests for the `http` feature.
That command does not enable `logging`.

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

## Out of scope

This feature does not include these items:

- Authentication.
- A catalog of product error codes.
- A project error enum.
- A project procedure.
- A project spec.
- `Box<dyn std::error::Error>` in the handler error path.
