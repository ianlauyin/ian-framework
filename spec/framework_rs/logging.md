# Request logging

This document describes request and event logging in the framework.
This feature is not product business logic.

## Crate

The package name is `ian_framework`.
The crate path is `framework_rs/`.
The module path is `ian_framework::logging`.
`rust-toolchain.toml` gives the Rust version for this repository.

## Build

This module has no Cargo feature.
Each build compiles this module.
The `gcs` feature is optional.
The Google Cloud Storage client is `spec/framework_rs/gcs.md`.

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

`cargo check` checks the build with no optional feature.
`cargo test` runs the tests with no optional feature.
`cargo check --features gcs` checks the build with the `gcs` feature.
`cargo test --features gcs` runs the tests with the `gcs` feature.

## Event

One event is one `LogEvent`.

One event has these fields:

| field | value |
| --- | --- |
| `ts` | UTC time in RFC 3339 |
| `id` | event id, or an empty string |
| `ip` | client address, or an empty string |
| `user_agent` | `user-agent` header, or an empty string |
| `action` | `{method} {path}`, or an empty string |
| `status` | HTTP status, or null |
| `latency_ms` | latency in milliseconds, or null |
| `level` | `info`, `warn`, or `error` |
| `message` | short text |
| `extra` | one JSON object |

The event has no `app` field.
The event has no `project` field.

`action` is one string.
The shape is `{method} {path}`.
One space separates the method and the path.
An example is `GET /api/foo`.
The path does not include the query string.
An empty path with a method is `/`.
An empty method gives an empty `action`.

## Shipper

`ShipperConfig::new` builds one `ShipperConfig` for one app.
The argument is the app.
`endpoint` comes from the environment variable `LOG_ENDPOINT`.
When `LOG_ENDPOINT` is not set, `endpoint` is unset.
The caller can set `endpoint` on the config before `LogShipper::new`.
`capacity` starts at 10000 events.
`mask_patterns` starts with the default patterns in the Masking section.
`ship_every` starts at 5 seconds.
The ship request time limit is 10 seconds.

`LogShipper::new` builds one shipper from that config.
One shipper has one app.
The app is the routing name.
The app name starts with a letter or a digit.
The other characters are letters, digits, `.`, `_`, or `-`.
An empty app returns no shipper.
A forbidden character in the app returns no shipper.
`capacity` of zero returns no shipper.
A `mask_patterns` item that is not a valid regex returns no shipper.
`ship_every` of zero returns no shipper.
`LogShipper::new` writes one `tracing` info event with the field `app`. The message is `log shipping posts to the log server` when the shipper has an endpoint, and `log shipping is trace-only` when it does not.

`LogShipper::record` stores one event in the local buffer.
Before it stores or traces the event, `LogShipper::record` masks the event `extra` values.
In trace-only mode, `LogShipper::record` writes the event with `tracing` and does not store it.
The shipper does not write `app` into the event.
The shipper stores the full `message`.
The shipper stores the full `action`.
The shipper stores the full `id`.
The shipper stores the full `ip`.
The shipper stores the full `user_agent`.
The buffer keeps at most `capacity` events.
When the buffer is full, a new event removes the oldest event.

After `LogShipper::record` stores the event, the shipper reads the buffer length.
The 80 percent limit is the smallest whole number that is at least 80 percent of `capacity`.
The length meets that limit when five times the length is at least four times `capacity`.

| capacity | 80 percent limit |
| --- | --- |
| 1 | 1 |
| 4 | 4 |
| 5 | 4 |
| 10000 | 8000 |

When the length meets the limit, `LogShipper::record` stores one new capacity warning.
The shipper does not change the other events.
The warning is one event at the end of the buffer.
The warning has these fields:

| field | value |
| --- | --- |
| `ts` | current UTC time |
| `id` | empty string |
| `ip` | empty string |
| `user_agent` | empty string |
| `action` | empty string |
| `status` | null |
| `latency_ms` | null |
| `level` | `warn` |
| `message` | `capacity meets 80%` |
| `extra` | empty object |

When that warning would make the buffer larger than `capacity`, the shipper removes the oldest event first.
The shipper stores this warning on the record that makes the length meet the limit.
The shipper does not store the warning again while the length stays at or above the limit.

`LogShipper::spawn_ship_loop` sends buffered events to the log server.
The loop repeats that send on an interval.
One interval sends one batch.
The HTTP method is POST.
The URL is `{endpoint}/log/{app}`.
`{app}` is the shipper app.
For app `cs`, the path is `/log/cs`.
The content type is `application/json`.
The body is one JSON object.
The body type is `LogBatch`.
The body shape is `{"events":[...]}`.
The body is not a bare JSON array.
`events` is one JSON array of event objects.
Each event object has every field in the event table.
Each event object has no `app` key.
Each event object has no `project` key.
The shipper takes the oldest events out of the buffer before the POST.
One batch has at most 1000 events.
The other events stay in the buffer.
The next interval sends the next oldest events.
A response status from 200 through 299 leaves that batch out of the buffer.
A transport error puts that batch back at the front of the buffer.
An HTTP error status puts that batch back at the front of the buffer.
When that return makes the buffer larger than `capacity`, the shipper removes the newest events above the capacity.
A successful ship that leaves the buffer length lower than the 80 percent limit lets `LogShipper::record` store the capacity warning again.
The next record that makes the length meet the limit stores one new warning.
A successful ship that leaves the length at or above the limit does not store another warning.
A failed ship does not store a second warning.
When a failed ship puts the batch back and the length meets the limit, the next record does not store another warning.
The ship request time limit is 10 seconds.
The caller runs `LogShipper::spawn_ship_loop` on a Tokio runtime.
A ship failure does not stop the loop.

`LogShipper::spawn_ship_loop` takes one `ian_framework::shutdown::ShutdownReceiver`.
The interval is `ship_every` from the config.
It returns `tokio::task::JoinHandle<()>`.
When the shutdown receiver is triggered, the loop stops and makes one final flush.
The final flush sends batches until the buffer is empty.
The final flush stops at the first batch that does not leave the buffer, so a log server that is down does not keep the process open.
The task ends after the final flush.
The project awaits the task before `main` returns, so the logs of the last interval are sent.

```rust
let shutdown = ian_framework::shutdown::ShutdownReceiver::new();
let ship_task = shipper.spawn_ship_loop(shutdown.clone());
axum::serve(listener, app)
    .with_graceful_shutdown(shutdown.wait())
    .await?;
ship_task.await.ok();
```
An empty buffer with an endpoint does not send an HTTP request.

The path app selects the log stream.
A log server stores its project from the path app.
A consumer splits logs by the app in the path.

## Masking

`mask_patterns` is a list of regex patterns.
The syntax is the `regex` crate syntax.
An example is `password=\S+`.
For a case-insensitive match, start the pattern with `(?i)`.

The default patterns are `(?i)password`, `(?i)secret`, and `(?i)token`.
The middleware logs the query and the JSON body, so the defaults keep a credential field such as `password`, `newPassword`, `client_secret`, or `access_token` out of a log.
A project adds its own patterns to the list. A project that sets a new list drops the defaults.
The shipper masks only the `extra` values.
The shipper does not mask the other event fields.

When an `extra` key matches a pattern, the shipper replaces the whole value with the string `***`.
This rule applies to a value of any type.
This rule also applies to a key in a nested object.
The shipper does not change the key.

When the key does not match, the shipper replaces each match of each pattern in a string value with `***`.
The patterns apply in list order.
This rule also applies to a string in a nested array or object.
The shipper does not mask a number, a `bool`, or null by the value rule.
The shipper masks in buffered mode and in trace-only mode.
The shipper masks client events from `POST /log` and request events from the middleware.
The frontend logger also masks before it sends. The browser patterns and the shipper patterns are separate lists.
An empty `mask_patterns` list does not change the event.

```rust
let mut config = ShipperConfig::new("cs");
config.mask_patterns.push(r"^sig$".to_string());
let shipper = LogShipper::new(config).unwrap();
```

## Adapter

The default shipper posts to `{endpoint}/log/{app}`.
A consumer that maps the app to a different path uses an adapter.
The adapter is outside this shipper.

## Trace-only mode

An unset endpoint selects trace-only mode.
An empty endpoint selects trace-only mode.
An endpoint that has only spaces selects trace-only mode.
In trace-only mode, `LogShipper::record` writes each event with `tracing` at the event level.
`info` uses `tracing::info!`.
`warn` uses `tracing::warn!`.
`error` uses `tracing::error!`.
The tracing message is the event `message`.
The tracing fields are `app`, `id`, `ip`, `user_agent`, `action`, `status`, `latency_ms`, and `extra`.
`extra` is the JSON text of the event `extra`.
The project installs a `tracing` subscriber to show these logs.
Trace-only mode does not store events in the buffer.
Trace-only mode does not store the capacity warning.
Trace-only mode does not build an HTTP client.
Trace-only mode does not open an HTTP connection.
`LogShipper::spawn_ship_loop` in trace-only mode does not open an HTTP connection.
A missing log server does not panic the shipper.
An `http` or `https` endpoint sends batches.
The shipper removes a trailing slash from that URL.
An endpoint with a query returns no shipper.
An endpoint with a fragment returns no shipper.
An endpoint with whitespace returns no shipper.
A scheme other than `http` or `https` returns no shipper.

## Axum middleware

`log_request` is the Axum middleware function.
Use it with `axum::middleware::from_fn_with_state` and one shipper.

```rust
let shipper = LogShipper::new(ShipperConfig::new("cs")).unwrap();
let router = Router::new().layer(middleware::from_fn_with_state(
    shipper,
    ian_framework::logging::log_request,
));
```

The middleware does not read the header `x-request-id`.
The middleware does not write the header `x-request-id`.
The middleware leaves the event field `id` empty.
The middleware records one event after the inner handler returns.
The middleware does not set `app` on the event.
`action` is `{method} {path}`.
One space separates the method and the path.
`method` is the request method.
`path` is the request path.
An empty path is `/`.
The path does not include the query string.
The middleware sets `ip` from the first source that is not empty.
The sources are in this order:

- The first item in the header `x-forwarded-for`
- The header `x-real-ip`
- The host of `ConnectInfo<SocketAddr>`

The middleware removes spaces at the start and at the end of a header value.
The host does not include the port.
A request with no source gets an empty `ip`.
`user_agent` is the header `user-agent`.
A missing header gives an empty `user_agent`.
The middleware skips a header value that is not text.
`status` is the response status.
`latency_ms` is the handler time in milliseconds.
`message` is `request`.
`extra` has these keys, and no other key:

| key | value |
| --- | --- |
| `query` | one object with each query field, when the query has a field |
| `body` | one object with each body field, when the body has a field |

The middleware leaves out `query` when the request has no query or the query has no field.
The middleware reads the query as `application/x-www-form-urlencoded` text and decodes each name and value.
A query field without `=` has an empty string value.
Each query value is a string.
A query name that is in the query more than once has one array of its string values, in query order.

The middleware reads the body only when all these rules are true:

- The header `content-length` is a number from 1 through 65536.
- The media type in the header `content-type` is `application/json` or a type that ends with `+json`. The match ignores case and the parameters after `;`.

`body` is the JSON object.
The middleware keeps each nested value.
A JSON body that is not an object, or that is not valid JSON, gives no `body`.
The middleware does not log a body of another type, such as a form, a multipart upload, or text.
An empty object gives no `body`.
The middleware gives the same body bytes to the inner handler.
The middleware does not read another body, so a large body or a body without `content-length` reaches the handler as a stream.
When the body read fails, the middleware does not call the inner handler, returns status 400, and records the event with status 400.
`latency_ms` includes the body read.
The shipper masks `query` and `body` with the masking rules, and a key in a nested object also counts.
A pattern such as `^(password|token)$` masks `query.token` and `body.password`.

A status of 500 or higher has level `error`.
A status from 400 through 499 has level `warn`.
A status below 400 has level `info`.
A log failure does not change the HTTP response.

## Untrusted client intake

A client can send log events.
The client is not trusted.
`LogEvent::sanitize_untrusted_client` checks one event.
The input is a reference to one `LogEvent`.
The result is one event, or no event.
The function does not change the input event.
The function removes spaces at the start and at the end of `message`.
An empty message after that removal returns no event.
The function does not keep these client fields:

- `id`
- `ip`
- `user_agent`
- `status`
- `latency_ms`
- `ts`

The stored `id` is an empty string.
The stored `ip` is an empty string.
The stored `user_agent` is an empty string.
The stored `status` is null.
The stored `latency_ms` is null.
The stored `ts` is the current UTC time.
The function keeps the client `level`.
The function keeps the client `action`.
The function stores the `message` after it removes those spaces.
The function reads every `extra` key.
An empty key is not stored.
A key is stored only when each character is an ASCII letter, an ASCII digit, `_`, `-`, or `.`.
A value is stored only when it is a string, a number, or a `bool`.
The function does not store these values:

- An array
- An object
- Null
- Text that contains `\r`
- Text that contains `\n`

The function stores a string as that string.
The function stores a number as its number text.
The function stores a true value as `true`.
The function stores a false value as `false`.
Each stored value is a string.
The function stores the full value text.

`LogBatch::sanitize_untrusted_client` checks one batch.
The input is one `LogBatch`.
The result is a list of events, or no batch.
An empty `events` list returns no batch.
The function returns no batch when `events` has more than 1000 events.
The function accepts 1000 events when each event gives an event.
One ship batch uses this same limit of 1000 events.
The function returns no batch when one event gives no event.
The function does not call `LogShipper::record`.
The caller calls `LogShipper::record` for each kept event.
`body` is one `LogBatch`.
`shipper` is one `LogShipper`.

```rust
if let Some(events) = body.sanitize_untrusted_client() {
    for event in events {
        shipper.record(event);
    }
}
```

An app can keep its own HTTP handler.
The client log route below is one such handler.
This function does not set an HTTP status.
This function does not set a body size limit.

## Client log route

`ian_framework::logging::client_log_router(shipper)` gives an `axum::Router` with one route.
The argument is one `LogShipper`.
The route is `POST /log`.
The project merges this router into its own router.
A browser sends its logs to this route on the same origin.
The project gives the same shipper to this router and to the Axum middleware.
The client events and the request events then share one buffer.
The shipper ships them together, with the same capacity, warning, and batch rules.
When the middleware wraps this route, the `POST /log` request also records one request event.
When that body is at most 65536 bytes, the request event `body` holds the client batch.
The client events come first, because the middleware records after the handler returns.

```rust
let shipper = LogShipper::new(ShipperConfig::new("cs")).unwrap();
let router = Router::new()
    .merge(ian_framework::logging::client_log_router(shipper.clone()))
    .layer(middleware::from_fn_with_state(
        shipper,
        ian_framework::logging::log_request,
    ));
```

The route accepts only a same-origin request.
The `POST` method router has the layer `ian_framework::http::require_same_origin`. The rules are in `spec/framework_rs/http.md`.
Each other request gets status 403 with the body `{"code":"cross_origin","message":"cross-origin request"}`.
The layer is on the `POST` method only, so a request with another method still gets status 405.

The body is one `LogBatch`.
The body size limit is 1 MiB.
A larger body gets status 413.
A body that is not a `LogBatch` gets status 400.
The route calls `LogBatch::sanitize_untrusted_client`.
When that call returns no batch, the status is 400 and the route records no event.
The route sets `ip` and `user_agent` of each kept event from the request.
The sources are the same as in the Axum middleware.
The route calls `LogShipper::record` for each kept event.
The response status is 204 with an empty body.
The route does not check the content type.

## Out of scope

This feature does not include these items:

- A log server process.
- Authentication to the log server. The bearer API key check is in `spec/framework_rs/http.md`.
- A product log policy.
- Trash rules, browse caps, or upload gates.
- An adapter that maps the app to a different path.
