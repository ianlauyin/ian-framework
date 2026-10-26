# Request logging

This document describes request and event logging in the framework.
This feature is not product business logic.

## Crate

- The package name is `ian_framework`.
- The crate path is `framework_rs/`.
- The module path is `ian_framework::logging`.
- `rust-toolchain.toml` gives the Rust version for this repository.

## Build

This module has no Cargo feature. Each build compiles this module.

The `gcs` feature is optional. The Google Cloud Storage client is in `spec/framework_rs/gcs.md`.

Put this block in the project `Cargo.toml`:

```toml
[dependencies]
ian_framework = "0.1"
```

Use these commands in `framework_rs/`:

```sh
cargo check
cargo test
cargo check --features gcs
cargo test --features gcs
```

| Command | Effect |
| --- | --- |
| `cargo check` | Checks the build with no optional feature. |
| `cargo test` | Runs the tests with no optional feature. |
| `cargo check --features gcs` | Checks the build with the `gcs` feature. |
| `cargo test --features gcs` | Runs the tests with the `gcs` feature. |

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
| `level` | `info`, `warn`, or `error` |
| `message` | Short text |
| `extra` | One JSON object |

- The event has no `app` field.
- The event has no `project` field.

`action` is one string with these rules:

- The shape is `{method} {path}`. One space separates the method and the path.
- The path does not include the query string.
- An empty path with a method is `/`.
- An empty method gives an empty `action`.

## Shipper

### ShipperConfig

`ShipperConfig::new` builds one `ShipperConfig` for one app. The argument is the app.

| Field | Start value |
| --- | --- |
| `endpoint` | The environment variable `LOG_ENDPOINT`. When `LOG_ENDPOINT` is not set, `endpoint` is not set. |
| `capacity` | 10000 events |
| `mask_patterns` | The default patterns in the Masking section |
| `ship_every` | 5 seconds |

- The caller can set `endpoint` on the config before `LogShipper::new`.
- The ship request time limit is 10 seconds.

### LogShipper::new

`LogShipper::new` builds one shipper from that config. One shipper has one app. The app is the routing name.

- The app name starts with a letter or a digit.
- The other characters of the app name are letters, digits, `.`, `_`, or `-`.

`LogShipper::new` returns no shipper in these conditions:

- The app is empty.
- The app has a forbidden character.
- `capacity` is zero.
- A `mask_patterns` item is not a valid regex.
- `ship_every` is zero.
- The endpoint is not valid. The endpoint rules are in the Trace-only mode section.

`LogShipper::new` writes one `tracing` info event with the field `app`. The message is one of these:

- `log shipping posts to the log server`, when the shipper has an endpoint.
- `log shipping is trace-only`, when the shipper has no endpoint.

### Record

`LogShipper::record` stores one event in the local buffer.

- Before it stores or traces the event, `LogShipper::record` masks the event `extra` values.
- In trace-only mode, `LogShipper::record` writes the event with `tracing`. It does not store the event.
- The shipper does not write `app` into the event.
- The shipper stores the full `message`, `action`, `id`, `ip`, and `user_agent`.
- The buffer keeps a maximum of `capacity` events.
- When the buffer is full, a new event removes the oldest event.

### Capacity warning

After `LogShipper::record` stores the event, the shipper reads the buffer length.

- The 80 percent limit is the smallest whole number that is at least 80 percent of `capacity`.
- The length meets that limit when five times the length is at least four times `capacity`.

| Capacity | 80 percent limit |
| --- | --- |
| 1 | 1 |
| 4 | 4 |
| 5 | 4 |
| 10000 | 8000 |

When the length meets the limit, `LogShipper::record` stores one new capacity warning.

- The shipper does not change the other events.
- The warning is one event at the end of the buffer.
- When the warning makes the buffer larger than `capacity`, the shipper first removes the oldest event.
- The shipper stores the warning on the record that makes the length meet the limit.
- The shipper does not store the warning again while the length stays at or above the limit.

The warning has these fields:

| Field | Value |
| --- | --- |
| `ts` | Current UTC time |
| `id` | Empty string |
| `ip` | Empty string |
| `user_agent` | Empty string |
| `action` | Empty string |
| `status` | null |
| `latency_ms` | null |
| `level` | `warn` |
| `message` | `capacity meets 80%` |
| `extra` | Empty object |

### Ship loop

`LogShipper::spawn_ship_loop` sends buffered events to the log server. The loop does the send again on an interval. One interval sends one batch.

The request has these properties:

- The HTTP method is POST.
- The URL is `{endpoint}/log/{app}`. `{app}` is the shipper app.
- The content type is `application/json`.
- The request time limit is 10 seconds.

The body is one JSON object of type `LogBatch`:

```json
{"events":[...]}
```

- The body is not a bare JSON array.
- `events` is one JSON array of event objects.
- Each event object has each field in the event table.
- Each event object has no `app` key.
- Each event object has no `project` key.

Batch rules:

- The shipper takes the oldest events out of the buffer before the POST.
- One batch has a maximum of 1000 events.
- The other events stay in the buffer. The next interval sends the next oldest events.
- A response status from 200 through 299 keeps that batch out of the buffer.
- A transport error puts that batch back at the front of the buffer.
- An HTTP error status puts that batch back at the front of the buffer.
- When that return makes the buffer larger than `capacity`, the shipper removes the newest events above the capacity.
- An empty buffer with an endpoint does not send an HTTP request.

Capacity warning after a ship:

- A successful ship that makes the buffer length lower than the 80 percent limit lets `LogShipper::record` store the capacity warning again. The next record that makes the length meet the limit stores one new warning.
- A successful ship that keeps the length at or above the limit does not store another warning.
- A failed ship does not store a second warning.
- When a failed ship puts the batch back and the length meets the limit, the next record does not store another warning.

Loop task:

- The caller runs `LogShipper::spawn_ship_loop` on a Tokio runtime.
- `LogShipper::spawn_ship_loop` takes one `ian_framework::shutdown::ShutdownReceiver`.
- It returns `tokio::task::JoinHandle<()>`.
- The interval is `ship_every` from the config.
- A ship failure does not stop the loop.
- When the shutdown receiver is triggered, the loop stops and makes one final flush.
- The final flush sends batches until the buffer is empty.
- The final flush stops at the first batch that does not leave the buffer. Thus a log server that is down does not keep the process open.
- The task ends after the final flush.
- The project awaits the task before `main` returns. Thus the shipper sends the logs of the last interval.

Log streams:

- The path app selects the log stream.
- A log server stores its project from the path app.
- A consumer splits logs by the app in the path.

## Masking

`mask_patterns` is a list of regex patterns.

- The syntax is the `regex` crate syntax.
- For a case-insensitive match, start the pattern with `(?i)`.
- The default patterns are `(?i)password`, `(?i)secret`, and `(?i)token`.
- The middleware logs the query and the JSON body. The default patterns keep credential fields out of a log.
- A project adds its own patterns to the list.
- A project that sets a new list drops the default patterns.
- An empty `mask_patterns` list does not change the event.

Scope:

- The shipper masks only the `extra` values. It does not mask the other event fields.
- The shipper masks in buffered mode and in trace-only mode.
- The shipper masks client events from `POST /log` and request events from the middleware.
- The frontend logger also masks before it sends. The browser patterns and the shipper patterns are different lists.

Key rule:

- When an `extra` key matches a pattern, the shipper replaces the full value with the string `***`.
- This rule applies to a value of each type.
- This rule also applies to a key in a nested object.
- The shipper does not change the key.

Value rule:

- When the key does not match, the shipper replaces each match of each pattern in a string value with `***`.
- The patterns apply in list order.
- This rule also applies to a string in a nested array or a nested object.
- The value rule does not mask a number, a `bool`, or null.

## Adapter

The default shipper posts to `{endpoint}/log/{app}`. A consumer that maps the app to a different path uses an adapter. The adapter is outside this shipper.

## Trace-only mode

These endpoints select trace-only mode:

- An endpoint that is not set.
- An empty endpoint.
- An endpoint that has only spaces.

In trace-only mode, `LogShipper::record` writes each event with `tracing` at the event level:

| Event level | Macro |
| --- | --- |
| `info` | `tracing::info!` |
| `warn` | `tracing::warn!` |
| `error` | `tracing::error!` |

- The tracing message is the event `message`.
- The tracing fields are `app`, `id`, `ip`, `user_agent`, `action`, `status`, `latency_ms`, and `extra`.
- `extra` is the JSON text of the event `extra`.
- The project installs a `tracing` subscriber to show these logs.

Trace-only mode does not do these things:

- Store events in the buffer.
- Store the capacity warning.
- Build an HTTP client.
- Open an HTTP connection. This also applies to `LogShipper::spawn_ship_loop`.

A missing log server does not cause a panic in the shipper.

Endpoint rules:

- An `http` or `https` endpoint sends batches.
- The shipper removes a trailing slash from that URL.

`LogShipper::new` returns no shipper for these endpoints:

- An endpoint with a username or a password.
- An endpoint with a query.
- An endpoint with a fragment.
- An endpoint with whitespace.
- An endpoint with a scheme other than `http` or `https`.

## Axum middleware

`log_request` is the Axum middleware function. Use it with `axum::middleware::from_fn_with_state` and one shipper.

General rules:

- The middleware does not read the header `x-request-id`.
- The middleware does not write the header `x-request-id`.
- The middleware leaves the event field `id` empty.
- The middleware records one event after the inner handler returns.
- The middleware does not set `app` on the event.
- A log failure does not change the HTTP response.

### Event fields

| Field | Value |
| --- | --- |
| `action` | `{method} {path}`. One space separates the method and the path. `method` is the request method. `path` is the request path, with no query string. An empty path is `/`. |
| `ip` | The first source that is not empty. The sources are below. |
| `user_agent` | The header `user-agent`. A missing header gives an empty `user_agent`. |
| `status` | The response status. |
| `latency_ms` | The handler time in milliseconds. It includes the body read. |
| `message` | `request` |
| `level` | From the status. The rules are below. |
| `extra` | The keys `query` and `body`. The rules are below. |

The `ip` sources are in this order:

1. The first item in the header `x-forwarded-for`.
2. The header `x-real-ip`.
3. The host of `ConnectInfo<SocketAddr>`.

Header rules:

- The middleware removes spaces at the start and at the end of a header value.
- The host does not include the port.
- A request with no source gets an empty `ip`.
- The middleware skips a header value that is not text.

Level rules:

| Status | Level |
| --- | --- |
| 500 or higher | `error` |
| 400 through 499 | `warn` |
| Below 400 | `info` |

### Extra

`extra` has only these keys:

| Key | Value |
| --- | --- |
| `query` | One object with each query field, when the query has a field. |
| `body` | One object with each body field, when the body has a field. |

Query rules:

- The middleware leaves out `query` when the request has no query or the query has no field.
- The middleware reads the query as `application/x-www-form-urlencoded` text. It decodes each name and each value.
- A query field without `=` has an empty string value.
- Each query value is a string.
- A query name that occurs more than one time has one array of its string values, in query order.

The middleware reads the body only when all these rules are true:

- The header `content-length` is a number from 1 through 65536.
- The media type in the header `content-type` is `application/json` or a type that ends with `+json`. The match ignores case and the parameters after `;`.

Body rules:

- `body` is the JSON object.
- The middleware keeps each nested value.
- A JSON body that is not an object gives no `body`.
- A body that is not valid JSON gives no `body`.
- The middleware does not log a body of a different media type.
- An empty object gives no `body`.
- The middleware gives the same body bytes to the inner handler.
- The middleware does not read a different body. Thus a large body or a body without `content-length` goes to the handler as a stream.
- When the body read fails, the middleware does not call the inner handler. It returns status 400 and records the event with status 400.
- The shipper masks `query` and `body` with the masking rules. `query` and `body` are nested objects, so the key rule applies to their keys.

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
| `ts` | The current UTC time. The client value is not kept. |
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
- The `POST` method router has the layer `ian_framework::http::require_same_origin`. The rules are in `spec/framework_rs/http.md`.
- Each other request gets status 403 with the body `{"code":"cross_origin","message":"cross-origin request"}`.
- The layer is on the `POST` method only. Thus a request with a different method still gets status 405.

Request handling:

- The body is one `LogBatch`.
- The route does not check the content type.
- The body size limit is 1 MiB. A larger body gets status 413.
- A body that is not a `LogBatch` gets status 400.
- The route calls `LogBatch::sanitize_untrusted_client`. When that call gives no batch, the status is 400 and the route records no event.
- The route sets `ip` and `user_agent` of each kept event from the request. The sources are the same as in the Axum middleware.
- The route calls `LogShipper::record` for each kept event.
- The response status is 204 with an empty body.

## Out of scope

This feature does not include these items:

- A log server process.
- Authentication to the log server. The bearer API key check is in `spec/framework_rs/http.md`.
- A product log policy.
- Trash rules, browse caps, or upload gates.
- An adapter that maps the app to a different path.
