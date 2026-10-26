# Log event

The parts that apply to all of this feature are in `spec/framework_rs/logging/context.md`.

## Event

One event is one `LogEvent`. One event has these fields:

| Field | Value |
| --- | --- |
| `ts` | UTC time in RFC 3339 |
| `id` | Event id, or an empty string |
| `ip` | Client address, or an empty string |
| `user_agent` | `user-agent` header, or an empty string |
| `action` | `{method} {path}`, or an empty string |
| `status` | HTTP status, or null |
| `latency_ms` | Latency in milliseconds, or null |
| `level` | One `LogLevel` value |
| `message` | Short text |
| `extra` | One JSON object |

- The event has no `app` field.
- The event has no `project` field.

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
