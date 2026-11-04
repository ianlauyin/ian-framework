# Logger

The parts that apply to all of this feature are in `spec/framework_rs/logging/context.md`.

## Logger

- The source file is `framework_rs/src/logging/logger.rs`.
- `ian_framework::logging::Logger` is the global logger of the process. It holds one `LogShipper` in a private `std::sync::OnceLock` static named `LOGGER`.
- The shipper rules are in `spec/framework_rs/logging/shipper.md`.

| Function | Use |
| --- | --- |
| `Logger::init(shipper)` | Sets the global shipper one time. It returns `Err(shipper)` with the new shipper when a shipper is already set. The first shipper stays. |
| `Logger::get()` | Gives `Option<&'static LogShipper>`. It gives none before `init`. |
| `Logger::info(action, context)` | Records one event with the level `info`. |
| `Logger::warn(action, context)` | Records one event with the level `warn`, with no `error_message`. |
| `Logger::warn_with(action, context, error)` | Records one event with the level `warn`. |
| `Logger::error(action, context, error)` | Records one event with the level `error`. |

- `action` is `impl Into<String>`.
- `context` is `std::collections::BTreeMap<String, String>`.
- `error` of `Logger::warn_with` and `Logger::error` is `Box<dyn std::error::Error>`.
- `Logger::warn_with` sets `error_message` to the display text of `error`. When `error` is an `Exception`, that text is the message only.
- When `error` of `Logger::error` is an `Exception`, `error_message` is the display text, a line break, then the trace text. The trace text is `String::from` of that exception. The rules are in `spec/framework_rs/exception.md`. When `error` is not an `Exception`, `error_message` is the display text of `error`.
- `Logger::info` sets no `error_message`. The event rules for `error_message` are in `spec/framework_rs/logging/event.md`.
- The function makes the event with `LogEvent::info`, `LogEvent::warn`, `LogEvent::warn_with` or `LogEvent::error`, and then sets `context`. The constructor rules are in `spec/framework_rs/logging/event.md`.
- The event has the current HKT time, an empty `id`, no `latency_ms` and an empty `info`.
- When a shipper is set, the function calls `LogShipper::record` with the event. The shipper masks and ships the event.
- When no shipper is set, the function writes one `tracing` event at the same level. The fields are `action`, `error_message` and `context`. `context` is the JSON text of the context. The function does not mask the context in this case.
- The framework code that must log an error uses `Logger::error`. It does not use `tracing::error!`.
