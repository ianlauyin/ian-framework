# Time helpers

This document describes the time and time zone helpers in the framework.
This feature is not product business logic.

## Sub-features

| Sub-feature | Document |
| --- | --- |
| Hong Kong time | `spec/framework_rs/time/hkt.md` |

## Crate

- The package name is `ian_framework`.
- The crate path is `framework_rs/`.
- The module path is `ian_framework::time`.
- The source files are `framework_rs/src/time/mod.rs` and `framework_rs/src/time/hkt.rs`.
- The tests are in `framework_rs/src/time/hkt.rs`.

## Build

This module has no Cargo feature. Each build compiles this module.

The module uses the `chrono` crate.

The install line for the crate is in `spec/framework_rs/logging/context.md`.

## Out of scope

This feature does not include these items:

- A time zone database.
- Time zones other than Hong Kong time.
- Date and time parsing and formatting helpers.
