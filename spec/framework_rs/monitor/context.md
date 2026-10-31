# Monitor

This document describes the host monitors in the framework.
This feature is not product business logic.

A monitor checks a host value on an interval. When the value meets a threshold, it makes one `LogEvent` and gives the event to a sink that the caller supplies. A monitor does not know the shipper or the log server store.

## Sub-features

| Sub-feature | Document |
| --- | --- |
| CPU monitor on a Linux host | `spec/framework_rs/monitor/cpu-monitor.md` |
| Disk monitor on a Unix host | `spec/framework_rs/monitor/disk-monitor.md` |

## Crate

- The package name is `ian_framework`.
- The crate path is `framework_rs/`.
- The module path is `ian_framework::monitor`.
- The source files are in `framework_rs/src/monitor/`. The tests are in the source file of each monitor.
- The monitors use `ian_framework::logging::LogEvent` and `ian_framework::shutdown::ShutdownReceiver`.

## Build

This module is behind the Cargo feature `monitor`. The feature enables the `libc` dependency, which the disk monitor uses. A build without the feature does not compile this module.

Use these commands in `framework_rs/`:

```sh
cargo check --features monitor
cargo test --features monitor
```

The install line for the crate is in `spec/framework_rs/logging/context.md`.

## Out of scope

This feature does not include these items:

- A log server process or a store.
- A product alert policy.
- Monitors for memory or network.
