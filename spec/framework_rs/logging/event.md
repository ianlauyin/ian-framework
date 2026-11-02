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
| `context` | One JSON object with the facts about where the event came from. Each value is a string, a number, or a `bool`. |
| `info` | One JSON object with the data of the event |

- The event has no `ip` field and no `user_agent` field. The route middleware and the client log route put them in `context`.
- The event has no `app` field.
- The event has no `project` field.

### Access

- The fields of `LogEvent` are not public. A caller outside the crate cannot make an event with a struct literal and cannot read a field directly.
- A caller outside the crate gets an event from deserialize. An event from an untrusted client goes through `LogEvent::new`.
- A caller outside the crate reads an event with `ts()`, `id()`, `action()`, `latency_ms()`, `level()`, `context()` and `info()`.
- A caller outside the crate sets the event id with `set_id`. The log server sets the id of each event that it stores.

### Values

- The type of a `context` value is `ian_framework::logging::LogValue`. It is a string, a number, or a `bool`.
- Deserialize of an event rejects an array, an object, or null in `context`.
- The type of `info` is `serde_json::Map<String, serde_json::Value>`. It is a JSON object, and its values can be nested.

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
- `LogLevel::as_str` returns the JSON text of the variant.
- `LogLevel::parse` takes a JSON text and returns `Some` variant. It returns `None` for a text that is not in the table. It does not change the letter case.
