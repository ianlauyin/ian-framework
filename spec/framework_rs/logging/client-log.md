# Client log intake

The parts that apply to all of this feature are in `spec/framework_rs/logging/context.md`.

## Untrusted client intake

A client can send log events. The client is not trusted.

### LogEvent::sanitize_untrusted_client

`LogEvent::sanitize_untrusted_client` checks one event.

- The input is a reference to one `LogEvent`.
- The result is one event, or no event.
- The function does not change the input event.
- The function removes spaces at the start and at the end of `message`.
- An empty message after that removal gives no event.

The function sets the fields of the result as follows:

| Field | Stored value |
| --- | --- |
| `id` | Empty string. The client value is not kept. |
| `ip` | Empty string. The client value is not kept. |
| `user_agent` | Empty string. The client value is not kept. |
| `status` | null. The client value is not kept. |
| `latency_ms` | null. The client value is not kept. |
| `ts` | The current HKT time, from `hkt_now()`. The client value is not kept. |
| `level` | The client value. |
| `action` | The client value. |
| `message` | The client value, after the removal of the spaces. |
| `extra` | The client keys and values that obey the `extra` rules below. |

The function reads each `extra` key:

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

### LogBatch::sanitize_untrusted_client

`LogBatch::sanitize_untrusted_client` checks one batch.

- The input is one `LogBatch`.
- The result is a list of events, or no batch.
- An empty `events` list gives no batch.
- The function gives no batch when `events` has more than 1000 events.
- The function accepts 1000 events when each event gives an event.
- One ship batch uses this same limit of 1000 events.
- The function gives no batch when one event gives no event.
- The function does not call `LogShipper::record`.
- The caller calls `LogShipper::record` for each kept event.

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
- Then the client events and the request events share one buffer.
- The shipper ships them together, with the same capacity, warning, and batch rules.
- When the middleware wraps this route, the `POST /log` request also records one request event.
- When that body is a maximum of 65536 bytes, the request event `body` holds the client batch.
- The client events come first, because the middleware records after the handler returns.

Same-origin rules:

- The route accepts only a same-origin request.
- The `POST` method router has the layer `ian_framework::http::require_same_origin`. The rules are in `spec/framework_rs/http/same-origin.md`.
- Each other request gets status 403 with the body `{"code":"cross_origin","message":"cross-origin request"}`.
- The layer is on the `POST` method only. Thus a request with a different method still gets status 405.

Request handling:

- The body is one `LogBatch`.
- The route does not check the content type.
- The body size limit is 1 MiB.
- A body that is larger than 1 MiB gets status 413.
- When the server cannot read the body, the status is also 413.
- A body that is not a `LogBatch` gets status 400.
- The route calls `LogBatch::sanitize_untrusted_client`. When that call gives no batch, the status is 400 and the route records no event.
- The route sets `ip` and `user_agent` of each kept event from the request. The sources are the same as in the Axum middleware. The Axum middleware is in `spec/framework_rs/logging/middleware.md`.
- The route calls `LogShipper::record` for each kept event.
- The response status is 204 with an empty body.
