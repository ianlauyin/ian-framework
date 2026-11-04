# Log event

The parts that apply to all of this feature are in `spec/framework_rs/logging/context.md`.

## Event

One event is one `LogEvent`. One event has these fields:

| Field | Value |
| --- | --- |
| `ts` | Hong Kong time (HKT) in RFC 3339, with the offset `+08:00` |
| `id` | Event id, or an empty string |
| `action` | `API/{method}:{path}`, a client text, or an empty string |
| `latency_ms` | Latency in milliseconds, or null |
| `level` | One `LogLevel` value |
| `error_message` | Text of the error, or null |
| `context` | One JSON object with the facts about where the event came from. Each value is a string, a number or a `bool`. |
| `extra` | One JSON object with the data of the event. Each value is a string, a number or a `bool`. |

- The JSON of the event always has the key `error_message`. Its value is null when the event has no message.
- The event has no `ip` field and no `user_agent` field. The route middleware and the client log route put them in `context`.
- The event has no `app` field.
- The event has no `project` field.

### Access

- The fields of `LogEvent` are not public. A caller outside the crate cannot make an event with a struct literal and cannot read a field directly.
- A caller outside the crate makes an event with `LogEvent::info`, `LogEvent::warn`, `LogEvent::warn_with` or `LogEvent::error`. The rules are in the section "Constructors".
- A caller outside the crate also gets an event from deserialize. `LogEvent` uses the plain `Deserialize` derive, and `error_message` has `#[serde(default)]`. An event from an untrusted client goes through `LogEvent::from_client`.
- A caller outside the crate reads an event with `ts()`, `id()`, `action()`, `latency_ms()`, `level()`, `error_message()`, `context()` and `extra()`. `extra()` returns the `extra` map. The name `info` is the constructor.

### Changes that break a caller

- `LogEvent::info(action)` is the constructor of an `info` event. The accessor of the `extra` map is `extra()`.
- `LogEvent::new` is now `LogEvent::from_client`.
- The crate-private helper `event(action, level)` does not exist. A caller uses a constructor.
- `Logger::warn(action, context)` has no message. `Logger::warn_with(action, context, error)` and `Logger::error(action, context, error)` set `error_message` from `error`. The rules are in `spec/framework_rs/logging/logger.md`. `LogEvent::warn_with` and `LogEvent::error` give a message.
- `Logger::exception` does not exist. A caller uses `Logger::warn_with` or `Logger::error`.
- `ian_framework::logging::LogValue` is an untagged enum with the variants `Bool`, `Number` and `String`. Each `context` value and each `extra` value is a `LogValue`.
- `LogEvent::set_id` does not exist. A caller outside the crate cannot set `id`.
- `LogLevel::as_str` does not exist.
- `LogLevel::parse` does not exist.

### Constructors

| Constructor | Level | `error_message` |
| --- | --- | --- |
| `LogEvent::info(action)` | `info` | null |
| `LogEvent::warn(action)` | `warn` | null |
| `LogEvent::warn_with(action, error_message)` | `warn` | `error_message` is `impl Into<String>`. |
| `LogEvent::error(action, error_message)` | `error` | `error_message` is `impl Into<String>`, so the caller must give it. |

- `action` is `impl Into<String>`.
- A constructor sets `ts` to the current HKT time, `id` to an empty string, `latency_ms` to null, and `context` and `extra` to empty objects.
- A constructor does not change the `action` text or the message text. Only `LogEvent::from_client` cuts text.
- The app builds each event that does not come from a client. The app sets `extra` and does not check the keys or the values.
- `LogEvent::from_client` checks an event from a client. The checks are in `spec/framework_rs/logging/client-log.md`.

### Error message

- A `warn` event can have `error_message`. An `info` event can have it also, but the framework does not set it.
- An `error` event that the framework makes has `error_message`, because `LogEvent::error` requires it.
- Deserialize does not check `error_message`. A missing `error_message` reads as null on an event of each level. A null reads as null. A value that is not a string and not null, such as a number or an object, gives a deserialize error. One bad value rejects the batch. A client `error` event with a null, a missing or a blank `error_message` gets a placeholder in `LogEvent::from_client`. The rules are in `spec/framework_rs/logging/client-log.md`.
- The type of `error_message` is `Option<String>`. `error_message()` returns `Option<&str>`.
- The framework code that makes an `error` event sets `error_message` as follows:

| Source | `error_message` |
| --- | --- |
| `Logger::error` | When the error is an `Exception`, the display text, a line break, then the trace text. Else the display text of the error. The rules are in `spec/framework_rs/logging/logger.md`. |
| Request middleware, status 500 or higher | `response status {status}` |
| GCS ops record with the level `error` | The record message, followed by ` (status {status})` when the record has a status |

### Values

- The type of `context` is `std::collections::BTreeMap<String, LogValue>`.
- The type of `extra` is `std::collections::BTreeMap<String, LogValue>`.
- Deserialize of an event reads each `context` value and each `extra` value as a `LogValue`. A string, a number and a `bool` are valid. Null, an array and an object give a deserialize error.

### Time

- The type of `ts` is `chrono::DateTime<chrono::FixedOffset>`.
- Each event that the framework makes gets `ts` from `ian_framework::time::hkt_now()`. The HKT rules are in `spec/framework_rs/time/hkt.md`.
- The JSON text of `ts` has the offset `+08:00`.
- Deserialize accepts each RFC 3339 offset. A `Z` value is valid. Deserialize keeps the offset of the text.

`action` is one string with these rules:

- The shape of a request event is `API/{method}:{path}`. The path starts with `/`.
- The path does not include the query string.
- An empty path with a method is `/`.
- An empty method gives an empty `action`.

An event that the framework makes for itself, not for a request, uses the shape `{scope}/{action}`. `scope` is the feature name in upper case, such as `API`, `GCS`, `CPU`, `DISK`, `SHIPPER` or `ERROR-BOUNDARY`. `action` is a short lowercase phrase, such as `meets threshold`.

### LogLevel

`ian_framework::logging` exports the enum `LogLevel`. It is the type of the event `level`.

| Variant | JSON text |
| --- | --- |
| `LogLevel::Info` | `info` |
| `LogLevel::Warn` | `warn` |
| `LogLevel::Error` | `error` |

- `LogLevel` implements `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, `Serialize` and `Deserialize`.
- The JSON text is the lowercase variant name.
- A JSON text that is not in the table gives a deserialize error.
