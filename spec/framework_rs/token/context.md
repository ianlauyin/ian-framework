# Token check

This document describes the token check in the framework.
This feature is not product business logic.

## Sub-features

| Sub-feature | Document |
| --- | --- |
| RFC 9110 `token68` check | `spec/framework_rs/token/token68.md` |

## Crate

- The package name is `ian_framework`.
- The crate path is `framework_rs/`.
- The module path is `ian_framework::token`.
- The source file is `framework_rs/src/token.rs`.
- The tests are in `framework_rs/src/test/token.rs`.

## Build

This module has no Cargo feature. Each build compiles this module.

This module has no dependency.

The install line for the crate is in `spec/framework_rs/logging/context.md`.

## Out of scope

This feature does not include these items:

- The `Authorization` header read. That read is in `spec/framework_rs/http/bearer-key.md`.
- The API key compare. That compare is in `spec/framework_rs/http/bearer-key.md`.
