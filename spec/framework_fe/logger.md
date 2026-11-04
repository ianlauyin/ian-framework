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
| `LogFields` | type | `Record<string, string \| number \| boolean>`. |

`Logger` has these methods:

| Method | Effect |
| --- | --- |
| `Logger.info(action, info?)` | Records one event with the level `info`. |
| `Logger.warn(action, info?)` | Records one event with the level `warn`. |
| `Logger.error(action, errorMessage, info?)` | Records one event with the level `error`. When `errorMessage` is empty or has only whitespace, the logger uses the text `unknown error`. |
| `Logger.wrapAction(name, action)` | Returns a function that logs each run of `action`. The rules are in the section "Action log". |
| `Logger.contextWrapper(module, context)` | Returns a new object that has the keys of `context`. Each function value is wrapped with `Logger.wrapAction`. The rules are in the section "Action log". |
| `Logger.addContext(key, value)` | Sets one key in the context of the logger. `value` is a string. A key that exists gets the new value. |
| `Logger.removeContext(...keys)` | Removes each named key from the context of the logger. A key that is not there is ignored. |
| `Logger.setMaskPatterns(patterns)` | Sets the mask patterns. `patterns` is a `RegExp[]`. |
| `Logger.watchWindowErrors()` | Records the window errors and the unhandled promise rejections. |

## Behaviour

### Record

- The logger keeps one queue in memory for the page.
- When `action` has only whitespace or is empty, the logger does not record the event.
- The first recorded event starts the send timers.
- The queue keeps a maximum of 1000 events. When the queue is full, a new event removes the oldest event.

The event has these fields:

| Field | Value |
| --- | --- |
| `ts` | The current Hong Kong time (HKT) in ISO 8601 text with the offset `+08:00`, from `hktIsoString` of `ian-framework-fe/time`. The time rules are in `spec/framework_fe/time.md`. The browser time zone does not change the value. |
| `id` | Empty string |
| `action` | The `action` argument of the method, with no change. |
| `latency_ms` | `null`, or the time of the action for an event from `Logger.wrapAction`. |
| `level` | The level of the method. |
| `error_message` | `null` for an `info` event and a `warn` event. For an `error` event, the `errorMessage` argument after the masking. An empty or blank `errorMessage` becomes `unknown error` before the masking. |
| `context` | A copy of the context of the logger. Each value is a string. The context is empty until `Logger.addContext` adds keys. |
| `info` | The `info` values after the masking. |

`info` rules:

- The logger adds the key `page`. The value is `location.pathname`.
- A `page` key in the caller `info` replaces that value.
- The logger keeps the other caller keys.

### Context

- The logger keeps one context object in memory for the page. It is empty at the start.
- `Logger.addContext` and `Logger.removeContext` change the context for the events that the logger records after the call. They do not change the events that are already in the queue.
- The Rust log route does not keep the `context` of a browser event. The route rules are in `spec/framework_rs/logging/client-log.md`.

### Action log

A project wraps each of its context actions with `Logger.wrapAction(name, action)`. The returned function has the same arguments and the same result as `action`.

- Each call records one event, after the wrapped function succeeds or fails. The wrapper does not record an event before the wrapped function runs.
- The wrapper starts a timer with `performance.now()` before the wrapped function runs. `latency_ms` is the time to the end, in milliseconds, rounded to an integer.
- When the wrapped function returns, the event has the level `info`. `action` of the event is `name`.
- When the wrapped function throws, the event has the level `error`, and the wrapper throws the same error again. `info` has the key `error`, and `error_message` has the same text.
- When the wrapped function returns a promise, the wrapper records the event after the promise settles. The time includes the wait for the promise. A rejection still reaches the caller.
- `info` has the key `args`. The value is the JSON text of the array of the arguments of the call. When the arguments cannot be made into JSON text, the value is `unserializable`.
- The masking rules apply to `args`.
- The value of `error` is the `message` of the error when it is an `Error` and the message is not blank. Else the value is `unknown error`.

`Logger.contextWrapper(module, context)` wraps the actions of a context object.

- Each own key of `context` with a function value gets `Logger.wrapAction` with the name `module/key`. `key` is the name of the function in the object.
- The other values are copied with no change.
- `context` is not changed.

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
- The body is one JSON array. The array holds each event in the queue.

Send rules:

- An empty queue does not send a request.
- The logger empties the queue before the request.
- The logger ignores a failed request and the response status. It does not put the events back in the queue. It does not try again.

### Window errors

`Logger.watchWindowErrors` adds two window listeners. Each listener records one `error` event. `action` is a fixed text, so the text of a window error does not become the action. The message goes only in `error_message`, which the masking applies to.

| Window event | `action` | `error_message` |
| --- | --- | --- |
| `error` | `WINDOW/error` | The event `message`. If that message is empty, the message is `window error`. |
| `unhandledrejection` | `WINDOW/unhandledrejection` | `message` of the reason when the reason is an `Error`. Else the message is `unhandled rejection`. An empty message of an `Error` becomes `unknown error`. |

### Masking

The logger masks `info` and `error_message` before it puts the event in the queue.

- The start list of patterns is empty. With no pattern, the logger does not change `info`.
- `Logger.setMaskPatterns` replaces the full list.
- The logger adds the `g` flag to each pattern that does not have it. It keeps the other flags.
- The masking applies to the `page` key also.
- The masking also applies to `error_message`. The masking does not apply to `action` and `context`.

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
- Nested values in `info`.
