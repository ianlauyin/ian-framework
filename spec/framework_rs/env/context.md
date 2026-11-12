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
- The source files are `framework_rs/src/env.rs`.
- The tests are in `framework_rs/src/test/env.rs`.
- The shared test helpers are in `framework_rs/src/test/env.rs`.

## Build

Each build compiles this module. `required` and `optional` read only the environment variable.

- The tests use `std::os::unix::ffi`. Thus the tests compile only for a Unix target.

The install line for the crate is in `spec/framework_rs/logging/context.md`.

## Tests

- A test that sets an environment variable holds one shared lock.
- The test restores the previous value of the variable before it releases the lock.
- The shared test helpers are in `framework_rs/src/test/env.rs`. `with_var` sets one variable and `with_vars` sets several variables under the lock. With the `gcp` feature, `block_on` runs one future on a current-thread Tokio runtime. The Secret Manager and credentials tests use these helpers.

## Out of scope

This feature does not include these items:

- A `.env` file loader.
- A project configuration type.
- Read functions for types other than `String`.
- A cache of secret values.
- The names of the environment variables of a project.
