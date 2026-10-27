# Environment variable read helpers

This document describes the environment variable read helpers in the framework.
This feature is not product business logic.

## Sub-features

| Sub-feature | Document |
| --- | --- |
| Read functions, the Secret Manager lookup and the error type | `spec/framework_rs/env/read.md` |

## Crate

- The package name is `ian_framework`.
- The crate path is `framework_rs/`.
- The module path is `ian_framework::env`.
- The source files are `framework_rs/src/env/mod.rs`, `framework_rs/src/env/error.rs`, `framework_rs/src/env/optional.rs`, `framework_rs/src/env/required.rs` and `framework_rs/src/env/u64_or.rs`.
- The tests are in the same source files.
- The shared test helpers are in the test module of `framework_rs/src/env/mod.rs`.
- `rust-toolchain.toml` gives the Rust version for this repository.

## Build

Each build compiles this module. The public items and their signatures are the same with and without the Cargo feature `gcp`.

- Without the `gcp` feature, `required` and `optional` read only the environment variable.
- With the `gcp` feature, `required` and `optional` read Secret Manager first. They use the Secret Manager access in `spec/framework_rs/gcp/secret-manager.md`.
- The tests use `std::os::unix::ffi`. Thus the tests compile only for a Unix target.

The install line for the crate is in `spec/framework_rs/logging/context.md`.

## Tests

- A test that sets an environment variable holds one shared lock.
- The test restores the previous value of the variable before it releases the lock.
- The shared test helpers are in `pub(crate) mod tests` of `framework_rs/src/env/mod.rs`. `with_var` sets one variable and `with_vars` sets several variables under the lock. `block_on` runs one future on a current-thread Tokio runtime. The env tests and the `ShipperConfig` tests use these helpers.
- The tests do not call Google Cloud. With the `gcp` feature, `block_on` first sets the shared Secret Manager state to "no credentials". The lookup tests use a fake Secret Manager on a local test server. The shared fake is in the test module of `framework_rs/src/gcp/mod.rs`.

## Out of scope

This feature does not include these items:

- A `.env` file loader.
- A project configuration type.
- Read functions for types other than `String` and `u64`.
- A cache of secret values.
- The names of the environment variables of a project.
