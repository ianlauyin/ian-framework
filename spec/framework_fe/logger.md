# Browser logger

This document describes the browser logger in the frontend framework.
This feature is not product business logic.

## Package

- The package is `ian-framework-fe`. The package setup is in `spec/framework_fe/version.md`.
- The import path is `ian-framework-fe`.
- The root source file is `framework_fe/src/logger/index.ts`. It re-exports the `Logger` class from `logger.ts`, `withLog` from `with-log.ts` and the types.
- The test file is `framework_fe/test/logger/index.test.ts`.
- The logger uses only browser APIs. The logger does not use a UI library.

## API

| Item | Kind | Meaning |
| --- | --- | --- |
| `Logger` | class | The logger. All methods are static. |
| `LogLevel` | type | `"info"`, `"warn"` or `"error"`. |
| `LogFields` | type | `Record<string, string \| number \| boolean>`. |
| `LogEvent` | type | The event that `Logger.record` and `withLog` take: `{ action, level, errorMessage?, context?, extra?, latencyMs? }`. |
| `withLog` | function | Runs one piece of work and records one event for it. The rules are in the section "With log". |

`Logger` has these methods:

| Method | Effect |
| --- | --- |
| `Logger.info(action, context?)` | Records one event with the level `info`, with no `error_message`. |
| `Logger.warn(action, context?, error?)` | Records one event with the level `warn`. When `error` is `undefined`, the event has no `error_message`. |
| `Logger.error(action, context, error)` | Records one event with the level `error`. |
| `Logger.record(event)` | Records one `LogEvent`. The rules are in the section "Record". |
| `Logger.addContext(key, value)` | Sets one key in the context of the logger. `value` is a string, a number or a `bool`. A key that exists gets the new value. |
| `Logger.removeContext(...keys)` | Removes each named key from the context of the logger. A key that is not there is ignored. |
| `Logger.setMaskPatterns(patterns)` | Sets the mask patterns. `patterns` is a `RegExp[]`. |
| `Logger.watchWindowErrors()` | Records the window errors and the unhandled promise rejections. |

## Behaviour

The method shape is the same as `Logger` in `spec/framework_rs/logging/logger.md`.

- `context` is `LogFields`. It holds the facts about where the event came from. The call `context` keys are put over the context of the logger. The masking rules apply to it.
- `error` is `unknown`. The text of `error` is `message` when `error` is an `Error`, else `String(error)`. The text is the `error_message` of the event. For an `error` event, an empty or blank text becomes `unknown error`.
- `extra` is not an argument of these methods. The logger sets `extra` for its own data: `page`, and the `extra` of a `LogEvent`.

### Record

- The logger keeps one queue in memory for the page.
- When `action` has only whitespace or is empty, the logger does not record the event.
- The first recorded event starts the send timers.
- The queue keeps a maximum of 1000 events. When the queue is full, a new event removes the oldest event.
- An `error` event is not put in the queue when the last event in the queue is an `error` event with the same `action` and the same `error_message`, and the two `ts` values are 5 seconds or less apart. The check uses the last event only.

The event has these fields:

| Field | Value |
| --- | --- |
| `ts` | The current Hong Kong time (HKT) in ISO 8601 text with the offset `+08:00`, from `hktIsoString` of `ian-framework-fe`. The time rules are in `spec/framework_fe/time.md`. The browser time zone does not change the value. |
| `action` | The `action` argument of the method, with no change. |
| `latency_ms` | `null`, or the rounded `latencyMs` of the `LogEvent`. `withLog` sets it. |
| `level` | The level of the method. |
| `error_message` | `null` for `Logger.info`, and for `Logger.warn` when `error` is `undefined`. The text of `error` after the masking. For an `error` event, an empty or blank text becomes `unknown error` before the masking. |
| `context` | A copy of the context of the logger, with the key `request_url` before it, then the call `context` after the masking. The value of `request_url` is `location.href` after the masking. A `request_url` key from `Logger.addContext` or from the call replaces it. Each value is a string, a number or a `bool`. |
| `extra` | The data of the logger after the masking. |

`extra` rules:

- The logger adds the key `page`. The value is `location.pathname`.
- The logger adds the other keys as the sections below say.

### Context

- The logger keeps one context object in memory for the page. It is empty at the start.
- `Logger.addContext` and `Logger.removeContext` change the context for the events that the logger records after the call. They do not change the events that are already in the queue.
- The Rust log route keeps the `context` of a browser event, with the same key and value rules as `extra`. It sets `ip` and `user_agent` from the request. The route rules are in `spec/framework_rs/logging/client-log.md`.

### With log

`withLog(work, toEvent)` is an async function. The source file is `framework_fe/src/logger/with-log.ts`. It is the same as `with_log` in `spec/framework_rs/logging/logger.md`.

- `work` is a `Promise<T>`. `toEvent` is a function from the output to a `LogEvent`.
- It starts a timer with `performance.now()` when it is called, awaits `work`, and gives the output of `work` to the caller without a change.
- It calls `toEvent` with the output. It sets `latencyMs` of the event to the time of `work` after `toEvent`. Thus `toEvent` cannot set the latency. Then it calls `Logger.record` with the event.
- When `work` rejects, it does not call `toEvent`, records no event, and the rejection reaches the caller.

### Send

The logger posts the queue to the route `POST /log` on the same origin. The route is in `spec/framework_rs/logging/client-log.md`.

| Trigger | `keepalive` |
| --- | --- |
| Each 60 seconds | `false` |
| The page becomes hidden (`visibilitychange` with `document.visibilityState` equal to `hidden`) | `true` |
| The `pagehide` event of the window | Not used. The logger uses `navigator.sendBeacon`, with the same URL and body. Without `sendBeacon`, it sends as for a hidden page. |

The request has these properties:

- The HTTP method is POST.
- The URL is `/log`. The logger does not add the prefix `/ajax`.
- The request uses `credentials: "same-origin"`.
- The request has the header `Content-Type: application/json`.
- The body is one JSON object `{"events": [...]}`. The `events` array holds each event in the queue.

Send rules:

- An empty queue does not send a request.
- The logger empties the queue before the request. Only one request runs at a time. A send that starts while another runs is skipped.
- When the request fails, or the status is not 2xx and not 400 to 405, the logger puts the events back in front of the newer events. The queue still keeps only the newest 1000 events. The events go out in the next send.
- When the status is 400 to 405, the logger drops the events, because the same events would fail again.
- A `sendBeacon` call that returns `false` puts the events back in the same way. The logger cannot read the result of the beacon later.

### Window errors

`Logger.watchWindowErrors` adds two window listeners. The `error` listener uses the capture phase, so that it also sees a script, a style or an image that failed to load. Each listener records one `error` event. `action` is a fixed text, so the text of a window error does not become the action. The message goes only in `error_message`, which the masking applies to.

| Window event | `action` | `error_message` |
| --- | --- | --- |
| `error` | `WINDOW/error` | The event `message`. If that message is empty and `target` is a DOM element, the message is `DOM source error: <tag> source`, where `tag` is the lower case tag name and `source` is the `src` attribute, or the `href` attribute when there is no `src`. Else the message is `window error`. |
| `unhandledrejection` | `WINDOW/unhandledrejection` | `message` of the reason when the reason is an `Error`. Else the message is `unhandled rejection`. An empty message of an `Error` becomes `unknown error`. |

### Masking

The logger masks `extra`, the call `context` and `error_message` before it puts the event in the queue.

- The start list of patterns is empty. With no pattern, the logger does not change `extra` and the call `context`.
- `Logger.setMaskPatterns` replaces the full list.
- The logger adds the `g` flag to each pattern that does not have it. It keeps the other flags.
- The masking applies to the `page` key also.
- The masking also applies to `error_message`. The masking also applies to the value of `request_url` in `context`. The masking applies to the call `context`, with the key rule and the value rule. The masking does not apply to `action` and to the context of `Logger.addContext`.

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
- A delay between retries other than the 60 second send timer.
- Nested values in `extra`.
