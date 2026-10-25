# Process version

This document describes the version route in the framework.
This feature is not product business logic.

## Crate

The package name is `ian_framework`.
The crate path is `framework/`.
The module path is `ian_framework::version`.
`rust-toolchain.toml` gives the Rust version for this repository.

## Feature `version`

The Cargo feature name is `version`.
`version` is not a default feature.
The `version` feature enables these dependencies:

- `axum`
- `getrandom`
- `hex`
- `serde`

The `version` feature does not enable `http`.
The `version` feature does not enable `logging`.
The `version` feature does not enable `auth`.
The `version` feature does not enable `gcs`.

Put this block in the project `Cargo.toml`:

```toml
[dependencies]
ian_framework = { version = "0.1", default-features = false, features = ["version"] }
```

Use these commands in `framework/`:

```sh
cargo check --no-default-features --features version
cargo test --no-default-features --features version
cargo test --features version
```

## Route

`version_router()` gives an `axum::Router` with one route.
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

## Process version

The process version is 16 random bytes from `getrandom`, in lowercase hex.
The process version has 32 characters.
The first request makes the process version.
Each later request in the same process gets the same value.
A new process makes a new value.
A client compares the value with the last value. When the value changes, a new server process is running.

When `getrandom` fails, the response status is 500 and the body is empty.
The next request tries again.

## Out of scope

This feature does not include these items:

- The build version or the release version of the project.
- The client code that reads the route.
- Authentication of the route.
