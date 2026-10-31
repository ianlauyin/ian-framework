# Log event

The parts that apply to all of this feature are in `spec/framework_rs/logging/context.md`.

## Event

One event is one `LogEvent`. One event has these fields:

| Field | Value |
| --- | --- |
| `ts` | Hong Kong time (HKT) in RFC 3339, with the offset `+08:00` |
| `id` | Event id, or an empty string |
| `ip` | Client address, or an empty string |
| `user_agent` | `user-agent` header, or an empty string |
| `action` | `{method} {path}`, a client text, or an empty string |
| `latency_ms` | Latency in milliseconds, or null |
| `level` | One `LogLevel` value |
| `extra` | One JSON object |

- The event has no `app` field.
- The event has no `project` field.

### Time

- The type of `ts` is `chrono::DateTime<chrono::FixedOffset>`.
- Each event that the framework makes gets `ts` from `ian_framework::time::hkt_now()`. The HKT rules are in `spec/framework_rs/time/hkt.md`.
- The JSON text of `ts` has the offset `+08:00`.
- Deserialize accepts each RFC 3339 offset. A `Z` value is valid. Deserialize keeps the offset of the text.

`action` is one string with these rules:

- The shape is `{method} {path}`. One space separates the method and the path.
- The path does not include the query string.
- An empty path with a method is `/`.
- An empty method gives an empty `action`.

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
