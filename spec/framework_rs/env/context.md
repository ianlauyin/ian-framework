# Environment variable read helpers

This document describes the environment variable read helpers in the framework.
This feature is not product business logic.

## Sub-features

| Sub-feature | Document |
| --- | --- |
| Read functions and the error type | `spec/framework_rs/env/read.md` |

## Crate

- The package name is `ian_framework`.
- The crate path is `framework_rs/`.
- The module path is `ian_framework::env`.
- The source files are `framework_rs/src/env/mod.rs`, `framework_rs/src/env/error.rs`, `framework_rs/src/env/required.rs` and `framework_rs/src/env/u64_or.rs`.
- The tests are in `framework_rs/src/env/error.rs`, `framework_rs/src/env/required.rs` and `framework_rs/src/env/u64_or.rs`.
- The shared test helpers are in the test module of `framework_rs/src/env/mod.rs`.
- `rust-toolchain.toml` gives the Rust version for this repository.

## Build

This module has no Cargo feature. Each build compiles this module.

- The module uses only the Rust standard library.
- The tests use `std::os::unix::ffi`. Thus the tests compile only for a Unix target.

The install line for the crate is in `spec/framework_rs/logging/context.md`.

## Tests

- A test that sets an environment variable holds one shared lock.
- The test restores the previous value of the variable before it releases the lock.

## Out of scope

This feature does not include these items:

- A `.env` file loader.
- A project configuration type.
- Read functions for types other than `String` and `u64`.
- The names of the environment variables of a project.
