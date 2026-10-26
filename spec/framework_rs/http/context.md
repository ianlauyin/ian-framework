# HTTP errors and request headers

This document describes these items in the framework:

- The HTTP boundary error.
- The bearer API key check.
- The cookie header helpers.
- The same-origin check and the same-origin middleware.
- The process version route.

This feature is not product business logic.

## Sub-features

| Sub-feature | Document |
| --- | --- |
| HTTP boundary error | `spec/framework_rs/http/api-error.md` |
| Bearer API key check | `spec/framework_rs/http/bearer-key.md` |
| Cookie header helpers | `spec/framework_rs/http/cookie.md` |
| Same-origin check and same-origin middleware | `spec/framework_rs/http/same-origin.md` |
| Process version route | `spec/framework_rs/http/version-route.md` |

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
