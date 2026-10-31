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

## App settings

The framework does not read an environment variable for the log server settings. The app reads its own environment variables, with the names that the app chooses, and gives the values to `ShipperConfig`. `ShipperConfig` is in `spec/framework_rs/logging/shipper.md`.

| Setting | Use |
| --- | --- |
| `endpoint` | The base URL of the log server. When it is not set or blank, the shipper is trace-only. |
| `post_api_key` | The POST API key of the log server. The shipper sends it as `Authorization: Bearer {key}` on each batch POST. When it is not set or blank, the shipper sends no `Authorization` header. |

The log server trusts each event of an authorized POST. Thus it stores the `ip` and `user_agent` that the project backend sets.

## Crate

- The package name is `ian_framework`.
- The crate path is `framework_rs/`.
- The module path is `ian_framework::logging`.

## Build

Each build compiles this module. This module does not use the `gcp` feature.

- The `gcs` feature is optional. The Google Cloud Storage client is in `spec/framework_rs/gcs/context.md`.
- The `gcp` feature is optional. The `gcs` feature enables it. The Secret Manager access is in `spec/framework_rs/gcp/context.md`.
- The `monitor` feature is optional. The CPU and disk monitors are in `spec/framework_rs/monitor/context.md`.

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
cargo check --features gcp
cargo test --all-features
cargo clippy --all-features --all-targets
```

| Command | Effect |
| --- | --- |
| `cargo check` | Checks the build with no optional feature. |
| `cargo test` | Runs the tests with no optional feature. |
| `cargo check --features gcs` | Checks the build with the `gcs` feature. |
| `cargo test --features gcs` | Runs the tests with the `gcs` feature. |
| `cargo check --features gcp` | Checks the build with the `gcp` feature. |
| `cargo test --all-features` | Runs the tests with all the optional features. |
| `cargo clippy --all-features --all-targets` | Lints the build and the tests with all the optional features. |

## Out of scope

This feature does not include these items:

- A log server process.
- The log server check of the POST API key. The shipper only sends the key. The shipper rules are in `spec/framework_rs/logging/shipper.md`. The bearer API key check is in `spec/framework_rs/http/bearer-key.md`.
- A product log policy.
- Trash rules, browse caps, or upload gates.
- An adapter that maps the app to a different path.
