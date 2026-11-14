# Client log intake

The parts that apply to all of this feature are in `spec/framework_rs/logging/context.md`.

## Untrusted client intake

A client can send log events. The client is not trusted.

The app builds each event that does not come from a client. The app does not check that event. `ClientLog::log_events` checks the events from a client.

### ClientLogEvent

`ian_framework::logging::ClientLogEvent` is one event that a client sends. It has the same fields as `LogEvent` (`ts`, `action`, `latency_ms`, `level`, `error_message`, `context`, `extra`) except `id`. In the JSON from the client, `latency_ms` is `latencyMs` and `error_message` is `errorMessage`. The client does not send an `id`. A client `id` in the JSON is ignored.

### Check of one event

`ClientLog::log_events` checks each event with these rules.

- The result is one new event for each input event.
- The function does not change the input events.

The function sets the fields of the result as follows:

| Field | Stored value |
| --- | --- |
| `id` | A new UUID v7 text. The client value is not kept. |
| `latency_ms` | The client value, or 0 when the client sends none. |
| `ts` | The client value in HKT, when it is not later than `ian_framework::time::hkt_now()` and is not more than 24 hours before it. In all other cases, the current HKT time from `hkt_now()`. |
| `level` | The client value. |
| `action` | The client value, with each control character changed to a space, and cut to 128 characters. |
| `context` | The client keys and values that obey the key and value rules below, with the keys `ip` and `user_agent` read from `headers` with `client_ip` and `user_agent`. A client `ip` or `user_agent` is replaced. |
| `error_message` | For an `error` event: the client text with each control character changed to a space, and cut to 1024 characters, or the text `unknown error` when that text is null, missing or blank. For a `warn` event: the same text rules, or null when the text is null, missing or blank. For an `info` event: null. The client value is not kept. |
| `extra` | The client keys and values that obey the `extra` rules below. |

The function reads each `extra` key. The function reads each client `context` key with the same rules:

- The function stores at most 32 keys. It ignores the keys after the 32nd stored key.
- The function does not store a string value that is longer than 2048 bytes.
- The function does not store an empty key.
- The function stores a key only when each character is an ASCII letter, an ASCII digit, `_`, `-`, or `.`.
- The function stores a value only when it is a string, a number, or a `bool`.

The function does not store these values:

- An array
- An object
- Null
- Text that contains `\r`
- Text that contains `\n`

The function changes each stored key from camelCase to snake_case, after the key passes the key rule. `userId` becomes `user_id` and `HTTPServer` becomes `http_server`. A key that is already snake_case does not change. When two keys give the same snake_case key, the later key in key order replaces the earlier one. The function converts the keys of `context` before it sets `ip` and `user_agent`.

The function checks a value and does not change it. A stored value has the same type as the client value: a string, a number or a `bool`. The function stores the full string.

### ClientLog::log_events

`ian_framework::logging::ClientLog::log_events` checks one list of client events. `ClientLog` has one public field `events`.

- The input is `&self`, with a list of `ClientLogEvent` in `events`, and `headers: &axum::http::HeaderMap`.
- The result is a list of events, or no list.
- An empty list gives no list.
- An `error` event without a non-blank `error_message` does not reject the list. The check of one event gives that event the placeholder `unknown error`.
- The function keeps `error_message` with the `error_message` rules in the table above.
- The function gives no list when the list has more than 1000 events.
- The function accepts 1000 events.
- One ship batch uses this same limit of 1000 events.
- The function does not call `Logger::record` or `Logger::direct_send`.
- The caller sends the kept events with `Logger::direct_send`.

An app can keep its own HTTP handler. The client log route below is one such handler. The function has these limits:

- The function does not set an HTTP status.
- The function does not set a body size limit.

## Client log route

`ian_framework::logging::client_log_router()` gives an `axum::Router` with one route.

- The function has no argument. The route uses the global logger from `Logger::get()`. The logger rules are in `spec/framework_rs/logging/logger.md`.
- The route is `POST /log`.
- The project merges this router into its own router.
- A browser sends its logs to this route on the same origin.

Shared logger:

- This router and the Axum middleware use the same global logger.
- The client events go to the log server at once. They do not enter the buffer.
- The request events of the middleware still use the buffer and the ship loop.
- When the middleware wraps this route, the `POST /log` request also records one request event in the buffer. That event does not hold the client batch.

Same-origin rules:

- The route accepts only a same-origin request.
- The `POST` method router has the layer `ian_framework::http::middleware::require_same_origin`. The rules are in `spec/framework_rs/http/same-origin.md`.
- Each other request gets status 403 with the body `{"message":"cross-origin request"}`.
- The layer is on the `POST` method only. Thus a request with a different method still gets status 405.

Request handling:

- The handler takes `axum::Json<ClientLog>`. `ClientLog` has one field `events`, an array of event objects. `ClientLog::log_events` gives the checked events, or no list.
- The `Json` extractor sets the statuses of a bad body. The request needs the content type `application/json`, or the status is 415.
- The body size limit is 1 MiB, set with `DefaultBodyLimit`. A body that is larger than 1 MiB gets status 413.
- A body that is not valid JSON gets status 400.
- A body that is valid JSON but is not `{"events": [...]}` of event objects gets status 422. A bare JSON array gets status 422.
- The route calls `ClientLog::log_events`. When that call gives no list, the route calls `Logger::warn` with the action `CLIENT_LOG/rejected` and an empty context. It sends no event. The response status is 400 with an empty body.
- `ClientLog::log_events` sets `ip` and `user_agent` in the `context` of each kept event from the request headers, after it keeps the client `context`. The `ip` rule is the same as in the Axum middleware. The Axum middleware is in `spec/framework_rs/logging/middleware.md`.
- The route awaits one call of `Logger::direct_send` with all the kept events. The rules are in `spec/framework_rs/logging/logger.md`.
- When `direct_send` gives `true`, the response status is 204 with an empty body.
- When `direct_send` gives `false`, the response status is 502 with an empty body. The route does not call `Logger::record`. The client can send the batch again.
- The statuses of the `Json` extractor, the body limit and `require_same_origin` stay as above. They happen before the handler runs.
