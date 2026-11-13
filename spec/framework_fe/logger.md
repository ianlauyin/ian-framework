# Browser logger

This document describes the browser logger in the frontend framework.
This feature is not product business logic.

## Package

- The package is `ian-framework-fe`. The package setup is in `spec/framework_fe/version.md`.
- The import path is `ian-framework-fe`.
- The source file is `framework_fe/src/logger/index.ts`.
- The test file is `framework_fe/test/logger/index.test.ts`.
- The logger uses only browser APIs. The logger does not use a UI library.

## API

| Item | Kind | Meaning |
| --- | --- | --- |
| `Logger` | class | The logger. All methods are static. |
| `LogLevel` | type | `"info"`, `"warn"` or `"error"`. |
| `LogFields` | type | `Record<string, string \| number \| boolean>`. |
| `LogScope` | type | `{ addInfo(key, value): void }`. `value` is a string, a number or a `bool`. |

`Logger` has these methods:

| Method | Effect |
| --- | --- |
| `Logger.info(action, context?)` | Records one event with the level `info`, with no `error_message`. |
| `Logger.warn(action, context?, error?)` | Records one event with the level `warn`. When `error` is `undefined`, the event has no `error_message`. |
| `Logger.error(action, context, error)` | Records one event with the level `error`. |
| `Logger.wrapAction(name, action)` | Returns a function that logs each run of `action`. The rules are in the section "Action log". |
| `Logger.addContext(key, value)` | Sets one key in the context of the logger. `value` is a string, a number or a `bool`. A key that exists gets the new value. |
| `Logger.removeContext(...keys)` | Removes each named key from the context of the logger. A key that is not there is ignored. |
| `Logger.setMaskPatterns(patterns)` | Sets the mask patterns. `patterns` is a `RegExp[]`. |
| `Logger.watchWindowErrors()` | Records the window errors and the unhandled promise rejections. |
| `Logger.scope()` | Returns the scope of the wrapped action that runs. The rules are in the section "Action scope". |

## Behaviour

The method shape is the same as `Logger` in `spec/framework_rs/logging/logger.md`.

- `context` is `LogFields`. It holds the facts about where the event came from. The call `context` keys are put over the context of the logger. The masking rules apply to it.
- `error` is `unknown`. The text of `error` is `message` when `error` is an `Error`, else `String(error)`. The text is the `error_message` of the event. For an `error` event, an empty or blank text becomes `unknown error`.
- `extra` is not an argument of these methods. The logger sets `extra` for its own data: `page`, and the keys of `Logger.wrapAction`, and `Logger.scope()`.

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
| `latency_ms` | `null`, or the time of the action for an event from `Logger.wrapAction`. |
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

### Action log

A project wraps each of its actions with `Logger.wrapAction(name, action)`. The returned function has the same arguments and the same result as `action`.

- Each call records one event, after the wrapped function succeeds or fails. The wrapper does not record an event before the wrapped function runs.
- The wrapper starts a timer with `performance.now()` before the wrapped function runs. `latency_ms` is the time to the end, in milliseconds, rounded to an integer.
- When the wrapped function returns, the event has the level `info`. `action` of the event is `name`.
- When the wrapped function throws, the event has the level `error`, and the wrapper throws the same error again. `extra` has the key `error`, and `error_message` has the same text.
- When the wrapped function returns a promise, the wrapper records the event after the promise settles. The time includes the wait for the promise. A rejection still reaches the caller.
- `extra` has the key `args`. The value is the JSON text of the array of the arguments of the call. When the arguments cannot be made into JSON text, the value is `unserializable`.
- The masking rules apply to `args`.
- The value of `error` is the `message` of the error when it is an `Error` and the message is not blank. Else the value is `unknown error`.

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

### Action scope

- `Logger.wrapAction` makes one scope for each call. `Logger.scope()` returns the scope of the call that runs now. The call must be in the part of the action before the first `await`. The scope still works after an `await`, until the action ends.
- `scope.addInfo(key, value)` adds a key to the `extra` of the event that the wrapper records for that call. The keys `args` and `error` of the wrapper replace a key with the same name. The masking rules apply.
- `Logger.scope()` outside a wrapped action returns a scope that ignores the calls. A call to `addInfo` on a scope after its action ended is also ignored. In both cases the logger writes a `console.warn`.
- When the action throws, the event keeps the info that was added before the error.

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
