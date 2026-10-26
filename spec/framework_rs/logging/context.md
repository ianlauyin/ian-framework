# Request logging

This document describes request and event logging in the framework.
This feature is not product business logic.

## Sub-features

| Sub-feature | Document |
| --- | --- |
| Event and log level | `spec/framework_rs/logging/event.md` |
| Shipper, adapter and trace-only mode | `spec/framework_rs/logging/shipper.md` |
| Masking | `spec/framework_rs/logging/masking.md` |
| Axum request middleware | `spec/framework_rs/logging/middleware.md` |
| Untrusted client intake and client log route | `spec/framework_rs/logging/client-log.md` |

## Crate

- The package name is `ian_framework`.
- The crate path is `framework_rs/`.
- The module path is `ian_framework::logging`.
- `rust-toolchain.toml` gives the Rust version for this repository.

## Build

This module has no Cargo feature. Each build compiles this module.

The `gcs` feature is optional. The Google Cloud Storage client is in `spec/framework_rs/gcs/context.md`.

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
cargo check --features gcs
cargo test --features gcs
```

| Command | Effect |
| --- | --- |
| `cargo check` | Checks the build with no optional feature. |
| `cargo test` | Runs the tests with no optional feature. |
| `cargo check --features gcs` | Checks the build with the `gcs` feature. |
| `cargo test --features gcs` | Runs the tests with the `gcs` feature. |

## Out of scope

This feature does not include these items:

- A log server process.
- Authentication to the log server. The bearer API key check is in `spec/framework_rs/http/bearer-key.md`.
- A product log policy.
- Trash rules, browse caps, or upload gates.
- An adapter that maps the app to a different path.
