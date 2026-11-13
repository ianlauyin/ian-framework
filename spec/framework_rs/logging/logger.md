# Logger

The parts that apply to all of this feature are in `spec/framework_rs/logging/context.md`.

## Logger

- The source file is `framework_rs/src/logging/logger/mod.rs`.
- The buffer source file is `framework_rs/src/logging/logger/log_buffer.rs`.
- `ian_framework::logging::Logger` holds one `HttpLogShipper`, the mask patterns and one local buffer.
- A clone of one `Logger` shares that buffer, those masks and that shipper.
- The logger masks each event. The shipper does not mask. The masking rules are in `spec/framework_rs/logging/masking.md`.
- The global logger of the process is one `Logger` in a private `std::sync::OnceLock` static named `LOGGER`.
- The shipper rules are in `spec/framework_rs/logging/shipper.md`.
- The buffer capacity is 20000 events. The ship interval is 60 seconds. The config does not set them.

| Function | Use |
| --- | --- |
| `Logger::new(shipper, mask_patterns)` | Builds one logger from one `HttpLogShipper` and a list of mask patterns. The buffer is empty. It returns `Result<Logger, Exception>`. When a pattern is not a valid regex, it returns an exception with the code `MASK_PATTERN_INVALID`. The message names the pattern. |
| `Logger::init(logger)` | Sets the global logger one time. It returns `Err(logger)` with the new logger when a logger is already set. The first logger stays. |
| `Logger::get()` | Gives `Option<&'static Logger>`. It gives none before `init`. |
| `Logger::info(action, context)` | Records one event with the level `info`. |
| `Logger::warn(action, context)` | Records one event with the level `warn`, with no `error_message`. |
| `Logger::warn_with(action, context, error)` | Records one event with the level `warn`. |
| `Logger::error(action, context, error)` | Records one event with the level `error`. |
| `Logger::record(event)` | Stores one event in the buffer. The rules are below. |
| `Logger::direct_send(events)` | Masks a copy of each event and calls `LogShipper::ship` at once. It gives the `ship` result. It does not use the buffer. The rules are below. |
| `Logger::spawn_ship_loop(shutdown)` | Sends the buffer to the log server. The rules are below. |

- `action` is `impl Into<String>`.
- `context` is `std::collections::BTreeMap<String, ian_framework::logging::LogValue>`.
- `error` of `Logger::warn_with` and `Logger::error` is `Box<dyn std::error::Error>`.
- `Logger::warn_with` sets `error_message` to the display text of `error`. When `error` is an `Exception`, that text is the message only.
- When `error` of `Logger::error` is an `Exception`, `error_message` is the display text, a line break, then the trace text. The trace text is `String::from` of that exception. The rules are in `spec/framework_rs/exception.md`. When `error` is not an `Exception`, `error_message` is the display text of `error`.
- `Logger::info` sets no `error_message`. The event rules for `error_message` are in `spec/framework_rs/logging/event.md`.
- The function makes the event with `LogEvent::info`, `LogEvent::warn`, `LogEvent::warn_with` or `LogEvent::error`, and then sets `context`. The constructor rules are in `spec/framework_rs/logging/event.md`.
- The event has the current HKT time, an empty `id`, `latency_ms` 0 and an empty `extra`.
- When a logger is set, the function calls `Logger::record` with the event. `record` masks the event and stores it in the buffer. The ship loop sends the buffer.
- When no logger is set, the function writes one `tracing` event at the same level. The fields are `action`, `error_message` and `context`. `context` is the JSON text of the context. The function does not mask the context in this case.
- The framework code that must log an error uses `Logger::error`. It does not use `tracing::error!`.

### Direct send

`Logger::direct_send(events)` is an async associated function. `events` is `&[LogEvent]`. It uses the global logger.

- When no logger is set, it does nothing, writes no log and gives `true`.
- It masks a copy of each event and calls `LogShipper::ship` with the copies. The caller keeps the original events.
- It gives `true` only when `ship` gives `true`.
- It does not buffer the events when `ship` gives `false`. The caller does that.

### Record

`Logger::record` stores one event in the local buffer.

- Before it stores the event, `Logger::record` masks the event `context` values and `extra` values.
- The logger does not write `app` into the event.
- The logger stores the full `action`, `id`, and `context`.
- The buffer keeps a maximum of 20000 events.
- When the buffer is full, a new event removes the oldest event.

### Capacity warning

After `Logger::record` stores the event, the logger reads the buffer length.

- The 80 percent limit is the smallest whole number that is at least 80 percent of the capacity.
- The length meets that limit when five times the length is at least four times the capacity.

| Capacity | 80 percent limit |
| --- | --- |
| 1 | 1 |
| 4 | 4 |
| 5 | 4 |
| 20000 | 16000 |

When the length meets the limit, `Logger::record` stores one new capacity warning.

- The logger does not change the other events.
- The warning is one event at the end of the buffer.
- When the warning makes the buffer larger than the capacity, the logger first removes the oldest event.
- The logger stores the warning on the record that makes the length meet the limit.
- The logger does not store the warning again while the length stays at or above the limit.

The warning has these fields:

| Field | Value |
| --- | --- |
| `ts` | Current HKT time, from `ian_framework::time::hkt_now()` |
| `id` | A new UUID v7 text |
| `action` | `SHIPPER/capacity meets 80%` |
| `latency_ms` | 0 |
| `level` | `warn` |
| `error_message` | null |
| `context` | Empty object |
| `extra` | Empty object |

### Ship loop

`Logger::spawn_ship_loop` sends buffered events to the log server. The loop calls `LogShipper::ship` on an interval. One interval sends one batch. The request rules of `ship` are in `spec/framework_rs/logging/shipper.md`.

Batch rules:

- The logger takes the oldest events out of the buffer before `ship`.
- One batch has a maximum of 1000 events.
- The other events stay in the buffer. The next interval sends the next oldest events.
- A `ship` result of `true` keeps that batch out of the buffer.
- A `ship` result of `false` puts that batch back at the front of the buffer.
- When that return makes the buffer larger than the capacity, the logger removes the oldest events above the capacity.
- An empty buffer does not call `ship`.

Capacity warning after a ship:

- A successful ship that makes the buffer length lower than the 80 percent limit lets `Logger::record` store the capacity warning again. The next record that makes the length meet the limit stores one new warning.
- A successful ship that keeps the length at or above the limit does not store another warning.
- A failed ship does not store a second warning.
- When a failed ship puts the batch back and the length meets the limit, the next record does not store another warning.

Loop task:

- The caller runs `Logger::spawn_ship_loop` on a Tokio runtime.
- `Logger::spawn_ship_loop` takes one `ian_framework::shutdown::ShutdownReceiver`. The shutdown module is in `spec/framework_rs/shutdown/receiver.md`.
- It returns `tokio::task::JoinHandle<()>`.
- The interval is 60 seconds.
- A ship failure does not stop the loop.
- When the shutdown receiver is triggered, the loop stops and makes one final flush.
- The final flush sends batches until the buffer is empty.
- The final flush stops at the first batch that does not leave the buffer. Thus a log server that is down does not keep the process open.
- The task ends after the final flush.
- The project awaits the task before `main` returns. Thus the logger sends the logs of the last interval.
