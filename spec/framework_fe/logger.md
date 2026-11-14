# Browser logger

This document describes the browser logger in the frontend framework.
This feature is not product business logic.

## Package

- The package is `ian-framework-fe`. The package setup is in `spec/framework_fe/version.md`.
- The import path is `ian-framework-fe`.
- The root source file is `framework_fe/src/logger/index.ts`. It re-exports the `Logger` class and `capture` from `logger.ts` and the types.
- The test file is `framework_fe/test/logger/index.test.ts`.
- The logger uses only browser APIs. The logger does not use a UI library.

## API

| Item | Kind | Meaning |
| --- | --- | --- |
| `Logger` | class | The logger. All methods are static. |
| `LogLevel` | type | `"info"`, `"warn"` or `"error"`. |
| `LogFields` | type | `Record<string, string \| number \| boolean>`. |
| `capture` | function | `Logger.capture`. It runs one piece of work and records one event for it. The rules are in the section "Capture". |

`Logger` has these methods:

| Method | Effect |
| --- | --- |
| `Logger.init(patterns)` | Sets the mask patterns, then starts the send timers and the window error listeners. `patterns` is a `RegExp[]`. A second call has no effect. |
| `Logger.info(action, context?)` | Records one event with the level `info`, with no `error_message`. |
| `Logger.warn(action, context?, error?)` | Records one event with the level `warn`. `error` is an `Error`. When `error` is `undefined`, the event has no `error_message`. |
| `Logger.error(action, context, error)` | Records one event with the level `error`. `error` is an `Error`. |
| `Logger.capture(actionName, work)` | Runs one piece of work and records one event for it. The rules are in the section "Capture". |
| `Logger.addContext(key, value)` | Sets one key in the context of the logger. `value` is a string, a number or a `bool`. A key that exists gets the new value. |
| `Logger.removeContext(...keys)` | Removes each named key from the context of the logger. A key that is not there is ignored. |

## Behaviour

The method shape is the same as `Logger` in `spec/framework_rs/logging/logger.md`.

- `context` is `LogFields`. It holds the facts about where the event came from. The call `context` keys are put over the context of the logger. The masking rules apply to it.
- `error` is `unknown`. The text of `error` is `message` when `error` is an `Error`, else `String(error)`. The text is the `error_message` of the event. For an `error` event, an empty or blank text becomes `unknown error`.
- `extra` is not an argument of these methods. The logger sets `extra` to the key `page`. The value is `location.pathname`.

### Record

- The logger keeps one queue in memory for the page.
- When `action` has only whitespace or is empty, the logger does not record the event.

The event has these fields:

| Field | Value |
| --- | --- |
| `ts` | The current Hong Kong time (HKT) in ISO 8601 text with the offset `+08:00`, from `hktIsoString` of `ian-framework-fe`. The time rules are in `spec/framework_fe/time.md`. The browser time zone does not change the value. |
| `action` | The `action` argument of the method, with no change. |
| `latencyMs` | `null` for `Logger.info`, `Logger.warn` and `Logger.error`. For `capture`, the rounded time in milliseconds from the start of the call to the end of `work`. |
| `level` | The level of the method. |
| `errorMessage` | `null` for `Logger.info`, and for `Logger.warn` when `error` is `undefined`. The text of `error` after the masking. For an `error` event, an empty or blank text becomes `unknown error` before the masking. |
| `context` | A copy of the context of the logger, with the key `request_url` before it, then the call `context` after the masking. The value of `request_url` is `location.href` after the masking. A `request_url` key from `Logger.addContext` or from the call replaces it. Each value is a string, a number or a `bool`. |
| `extra` | The key `page` after the masking. The value is `location.pathname`. |

### Context

- The logger keeps one context object in memory for the page. It is empty at the start.
- `Logger.addContext` and `Logger.removeContext` change the context for the events that the logger records after the call. They do not change the events that are already in the queue.
- The Rust log route keeps the `context` of a browser event, with the same key and value rules as `extra`. It sets `ip` and `user_agent` from the request. The route rules are in `spec/framework_rs/logging/client-log.md`.

### Capture

`capture(actionName, work)` is an async function. The source is the `capture` method of `Logger` in `framework_fe/src/logger/logger.ts`. It follows `with_log` in `spec/framework_rs/logging/logger.md`.

- `actionName` is a `string`. It is the `action` of the event.
- `work` is a function `(event: CaptureEvent) => Promise<T>`. `CaptureEvent` is `Omit<ClientLogEvent, "level" | "latencyMs" | "action" | "errorMessage" | "context">`. The event has `ts` and `extra`. `extra` starts empty and `work` can set keys in it.
- It starts a timer with `performance.now()` when it is called, then awaits `work` in a `try` block.
- When `work` resolves, it records one event with the level `info`, and gives the output of `work` to the caller without a change.
- When `work` rejects, it records one event with the level `error`, and throws the same rejection again. `errorMessage` is the message of the error. An empty message, or a reason that is not an `Error`, becomes `unknown error`.
- `latencyMs` is the rounded time from the start of the call to the end of `work`. `work` cannot set `level` or `latencyMs`.
- The `context` is the context of the logger, with `request_url`. `extra` has `page` and the keys of `work`, after the masking.

### Send

The logger posts the queue to the route `POST /log` on the same origin, with `navigator.sendBeacon`. The route is in `spec/framework_rs/logging/client-log.md`.

`Logger.init` sets the mask patterns, then starts the send timer and the window error listeners. A second call has no effect. A recorded event does not start the timer.

| Trigger | Send |
| --- | --- |
| Each 60 seconds | `sendBeacon` |
| The page becomes hidden (`visibilitychange` with `document.visibilityState` equal to `hidden`) | `sendBeacon` |
| The `pagehide` event of the window | `sendBeacon` |

The request has these properties:

- The URL is `/log`. The logger does not add the prefix `/ajax`.
- The `Blob` body has the type `application/json`. The browser sends the cookies of the same origin.
- The body is one JSON object `{"events": [...]}`. The `events` array holds each event in the queue.

Send rules:

- An empty queue does not send a request. Without `navigator.sendBeacon`, the logger does not send.
- The logger empties the queue before the call.
- The logger cannot read the result of a beacon later. When `sendBeacon` returns `false`, the logger puts the events back in front of the newer events. The events go out in the next send.

### Window errors

`Logger.init` adds two window listeners. The `error` listener uses the capture phase, so that it also sees a script, a style or an image that failed to load. Each listener records one `error` event. `action` is a fixed text, so the text of a window error does not become the action. The message goes only in `errorMessage`, which the masking applies to.

| Window event | `action` | `errorMessage` |
| --- | --- | --- |
| `error` | `WINDOW/error` | The event `message`. If that message is empty and `target` is a DOM element, the message is `DOM source error: <tag> source`, where `tag` is the lower case tag name and `source` is the `src` attribute, or the `href` attribute when there is no `src`. Else the message is `window error`. |
| `unhandledrejection` | `WINDOW/unhandledrejection` | `message` of the reason when the reason is an `Error`. Else the message is `unhandled rejection`. An empty message of an `Error` becomes `unknown error`. |

### Masking

The logger masks `extra`, the call `context` and `errorMessage` before it puts the event in the queue.

- Before `Logger.init`, the pattern list is empty. With no pattern, the logger does not change `extra` and the call `context`.
- `Logger.init` sets the full list. A second call has no effect, so it does not replace the list.
- The logger adds the `g` flag to each pattern that does not have it. It keeps the other flags.
- The masking applies to the `page` key also.
- The masking also applies to `errorMessage`. The masking also applies to the value of `request_url` in `context`. The masking applies to the call `context`, with the key rule and the value rule. The masking does not apply to `action` and to the context of `Logger.addContext`.

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
