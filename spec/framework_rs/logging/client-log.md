# Client log intake

The parts that apply to all of this feature are in `spec/framework_rs/logging/context.md`.

## Untrusted client intake

A client can send log events. The client is not trusted.

The app builds each event that does not come from a client. The app does not check that event. `LogEvent::from_client` checks an event from a client.

### LogEvent::from_client

`LogEvent::from_client` checks one event.

- The input is a reference to one `LogEvent`.
- The result is one event.
- The function does not change the input event.

The function sets the fields of the result as follows:

| Field | Stored value |
| --- | --- |
| `id` | Empty string. The client value is not kept. |
| `latency_ms` | null. The client value is not kept. |
| `ts` | The client value in HKT, when it is not later than `ian_framework::time::hkt_now()` and is not more than 24 hours before it. In all other cases, the current HKT time from `hkt_now()`. |
| `level` | The client value. |
| `action` | The client value, with each control character changed to a space, and cut to 128 characters. |
| `context` | Empty object. The client value is not kept. |
| `error_message` | For an `error` event: the client text with each control character changed to a space, and cut to 1024 characters, or the text `no error message` when that text is null, missing or blank. For a `warn` event: the same text rules, or null when the text is null, missing or blank. For an `info` event: null. The client value is not kept. |
| `extra` | The client keys and values that obey the `extra` rules below. |

The function reads each `extra` key:

- The function stores at most 32 keys. It ignores the keys after the 32nd stored key.
- The function does not store a value that is longer than 2048 bytes.
- The function does not store an empty key.
- The function stores a key only when each character is an ASCII letter, an ASCII digit, `_`, `-`, or `.`.
- The function stores a value only when it is a string, a number, or a `bool`.

The function does not store these values:

- An array
- An object
- Null
- Text that contains `\r`
- Text that contains `\n`

Each stored value is a string:

| Client value | Stored value |
| --- | --- |
| String | That string |
| Number | Its number text |
| True | `true` |
| False | `false` |

The function stores the full value text.

### client_events

`ian_framework::logging::client_events` checks one list of client events.

- The input is a reference to a list of `LogEvent`.
- The result is a list of events, or no list.
- An empty list gives no list.
- An `error` event without a non-blank `error_message` does not reject the list. `LogEvent::from_client` gives that event the placeholder `no error message`.
- The function keeps `error_message` with the `error_message` rules in the table above.
- The function gives no list when the list has more than 1000 events.
- The function accepts 1000 events.
- One ship batch uses this same limit of 1000 events.
- The function does not call `LogShipper::record` or `LogShipper::send`.
- The caller sends the kept events, for example with `LogShipper::send`.

An app can keep its own HTTP handler. The client log route below is one such handler. The function has these limits:

- The function does not set an HTTP status.
- The function does not set a body size limit.

## Client log route

`ian_framework::logging::client_log_router(shipper)` gives an `axum::Router` with one route.

- The argument is one `LogShipper`.
- The route is `POST /log`.
- The project merges this router into its own router.
- A browser sends its logs to this route on the same origin.

Shared shipper:

- The project gives the same shipper to this router and to the Axum middleware.
- The client events go to the log server at once. They do not enter the buffer.
- The request events of the middleware still use the buffer and the ship loop.
- When the middleware wraps this route, the `POST /log` request also records one request event in the buffer.
- When that body is a maximum of 65536 bytes, the request event `body` holds the client batch.

Same-origin rules:

- The route accepts only a same-origin request.
- The `POST` method router has the layer `ian_framework::http::middleware::require_same_origin`. The rules are in `spec/framework_rs/http/same-origin.md`.
- Each other request gets status 403 with the body `{"code":"cross_origin","message":"cross-origin request"}`.
- The layer is on the `POST` method only. Thus a request with a different method still gets status 405.

Request handling:

- The body is one JSON array of event objects.
- The route does not check the content type.
- The body size limit is 1 MiB.
- A body that is larger than 1 MiB gets status 413.
- When the server cannot read the body, the status is also 413.
- A body that is not a JSON array of event objects gets status 400.
- The route calls `client_events`. When that call gives no list, the status is 400 and the route records no event.
- The route sets `ip` and `user_agent` in the `context` of each kept event from the request. The `ip` rule is the same as in the Axum middleware. The Axum middleware is in `spec/framework_rs/logging/middleware.md`.
- The route calls `LogShipper::send` once with all the kept events. It does not use the buffer.
- `LogShipper::send` masks each event, then posts the list to the log server with the same URL, headers and timeout as a ship batch. It gives `true` only when the log server answers with a success status.
- When `send` gives `true`, the response status is 204 with an empty body.
- When `send` gives `false`, the response status is 502 with an empty body. The route does not buffer the events and does not retry.
