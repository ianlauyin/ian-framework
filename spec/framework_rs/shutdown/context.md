# Shutdown signal

This document describes the shutdown signal in the framework.
This feature is not product business logic.

## Sub-features

| Sub-feature | Document |
| --- | --- |
| Shutdown receiver | `spec/framework_rs/shutdown/receiver.md` |

## Crate

- The package name is `ian_framework`.
- The crate path is `framework_rs/`.
- The module path is `ian_framework::shutdown`.
- The source files are `framework_rs/src/shutdown/mod.rs` and `framework_rs/src/shutdown/shutdown_receiver.rs`.
- The tests are in `framework_rs/src/shutdown/shutdown_receiver.rs`.

## Build

This module has no Cargo feature. Each build compiles this module.

- The module uses the Tokio feature `signal`.
- The module uses `tokio::signal::unix`. Thus the crate compiles only for a Unix target.

The install line for the crate is in `spec/framework_rs/logging/context.md`.

## Use in the framework

`LogShipper::spawn_ship_loop` takes one `ShutdownReceiver`. When the receiver is triggered, the ship loop makes its final flush. The rules are in `spec/framework_rs/logging/shipper.md`.

## Out of scope

This feature does not include these items:

- A shutdown time limit.
- A wait for the tasks of the project.
- Signals other than Ctrl+C and `SIGTERM`.
