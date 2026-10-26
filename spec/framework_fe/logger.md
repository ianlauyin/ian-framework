# Browser logger

This document describes the browser logger in the frontend framework.
This feature is not product business logic.

## Package

- The package is `ian-framework-fe`. The package setup is in `spec/framework_fe/version.md`.
- The import path is `ian-framework-fe/logger`.
- The source file is `framework_fe/src/logger.ts`.
- The test file is `framework_fe/test/logger.test.ts`.
- The logger uses only browser APIs. The logger does not use a UI library.

## API

| Item | Kind | Meaning |
| --- | --- | --- |
| `Logger` | class | The logger. All methods are static. |
| `LogLevel` | type | `"info"`, `"warn"` or `"error"`. |
| `LogExtra` | type | `Record<string, string \| number \| boolean>`. |

`Logger` has these methods:

| Method | Effect |
| --- | --- |
| `Logger.info(message, extra?)` | Records one event with the level `info`. |
| `Logger.warn(message, extra?)` | Records one event with the level `warn`. |
| `Logger.error(message, extra?)` | Records one event with the level `error`. |
| `Logger.setMaskPatterns(patterns)` | Sets the mask patterns. `patterns` is a `RegExp[]`. |
| `Logger.watchWindowErrors()` | Records the window errors and the unhandled promise rejections. |

## Behaviour

### Record

- The logger keeps one queue in memory for the page.
- When `message` has only whitespace or is empty, the logger does not record the event. The log route rejects the full batch when one message is empty.
- The first recorded event starts the send timers.
- The queue keeps a maximum of 1000 events. When the queue is full, a new event removes the oldest event.

The event has these fields:

| Field | Value |
| --- | --- |
| `ts` | The current time in ISO 8601 UTC text. |
| `id` | Empty string |
| `ip` | Empty string |
| `user_agent` | Empty string |
| `action` | Empty string |
| `status` | `null` |
| `latency_ms` | `null` |
| `level` | The level of the method. |
| `message` | `message`, with no change. |
| `extra` | The `extra` values after the masking. |

`extra` rules:

- The logger adds the key `page`. The value is `location.pathname`.
- A `page` key in the caller `extra` replaces that value.
- The logger keeps the other caller keys.

### Send

The logger posts the queue to the route `POST /log` on the same origin. The route is in `spec/framework_rs/logging/client-log.md`.

| Trigger | `keepalive` |
| --- | --- |
| Each 60 seconds | `false` |
| The page becomes hidden (`visibilitychange` with `document.visibilityState` equal to `hidden`) | `true` |

The request has these properties:

- The HTTP method is POST.
- The URL is `/log`. The logger does not add the prefix `/ajax`.
- The request uses `credentials: "same-origin"`.
- The request has the header `Content-Type: application/json`.
- The body is `{"events":[...]}`. `events` is the full queue.

Send rules:

- An empty queue does not send a request.
- The logger empties the queue before the request.
- The logger ignores a failed request and the response status. It does not put the events back in the queue. It does not try again.

### Window errors

`Logger.watchWindowErrors` adds two window listeners. Each listener records one `error` event.

| Window event | Message |
| --- | --- |
| `error` | The event `message`. If that message is empty, the message is `window error`. |
| `unhandledrejection` | `message` of the reason when the reason is an `Error`. Else the message is `unhandled rejection`. |

### Masking

The logger masks `extra` before it puts the event in the queue.

- The start list of patterns is empty. With no pattern, the logger does not change `extra`.
- `Logger.setMaskPatterns` replaces the full list.
- The logger adds the `g` flag to each pattern that does not have it. It keeps the other flags.
- The masking applies to the `page` key also.
- The masking does not apply to `message`.

Key rule:

- When a key matches a pattern, the logger replaces the value with the string `***`.
- This rule applies to a value of each type.
- The logger does not change the key.

Value rule:

- When the key does not match, the logger replaces each match of each pattern in a string value with `***`.
- The patterns apply in list order.
- The value rule does not mask a number or a `boolean`.

The Rust shipper also masks the events from the log route. The browser patterns and the shipper patterns are different lists. The shipper rules are in `spec/framework_rs/logging/masking.md`.

## Out of scope

This feature does not include these items:

- A default mask pattern.
- A retry of a failed send.
- A send on a page unload other than the `visibilitychange` event.
- Nested values in `extra`.
