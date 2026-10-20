# Request logging

This document describes request and event logging in the framework.
This feature is not product business logic.

## Crate

The package name is `ian_framework`.
The crate path is `framework/`.
The module path is `ian_framework::logging`.
`rust-toolchain.toml` gives the Rust version for this repository.

## Feature `logging`

The Cargo feature name is `logging`.
`logging` is a default feature.
A default build compiles this feature.
A project gets this feature with no extra feature flag.
The `gcs` feature is optional.
The `gcs` feature enables `logging`.
The Google Cloud Storage client is `spec/framework/gcs.md`.

Put this block in the project `Cargo.toml`:

```toml
[dependencies]
ian_framework = "0.1"
```

Use these commands in `framework/`:

```sh
cargo check
cargo test
cargo check --features gcs
cargo test --features gcs
```

`cargo check` checks the default build.
The default build includes `logging`.
`cargo test` runs the default tests.
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

`ShipperConfig::new` builds one `ShipperConfig` for one app and one endpoint.
The first argument is the app.
The second argument is the endpoint.
`capacity` starts at 10000 events.
The ship request time limit is 10 seconds.

`LogShipper::new` builds one shipper from that config.
One shipper has one app.
The app is the routing name.
The app name starts with a letter or a digit.
The other characters are letters, digits, `.`, `_`, or `-`.
An empty app returns no shipper.
A forbidden character in the app returns no shipper.
`capacity` of zero returns no shipper.

`LogShipper::record` stores one event in the local buffer.
The shipper does not write `app` into the event.
The shipper stores the full `message`.
The shipper stores the full `action`.
The shipper stores the full `id`.
The shipper stores the full `ip`.
The shipper stores the full `user_agent`.
The buffer keeps at most `capacity` events.
When the buffer is full, a new event removes the oldest event.

`LogShipper::spawn_ship_loop` sends every buffered event to the log server.
The loop repeats that send on an interval.
The HTTP method is POST.
The URL is `{endpoint}/log/{app}`.
`{app}` is the shipper app.
For app `cs`, the path is `/log/cs`.
The content type is `application/json`.
The body is one JSON object.
The body type is `RequestJson`.
The body shape is `{"events":[...]}`.
The body is not a bare JSON array.
`events` is one JSON array of event objects.
Each event object has every field in the event table.
Each event object has no `app` key.
Each event object has no `project` key.
The shipper takes those events out of the buffer before the POST.
A response status from 200 through 299 leaves that batch out of the buffer.
A transport error puts that batch back at the front of the buffer.
An HTTP error status puts that batch back at the front of the buffer.
When that return makes the buffer larger than `capacity`, the shipper removes the newest events above the capacity.
The ship request time limit is 10 seconds.
An interval of zero returns no task.
The caller runs `LogShipper::spawn_ship_loop` on a Tokio runtime.
A ship failure does not stop the loop.
An empty buffer with an endpoint does not send an HTTP request.

The path app selects the log stream.
A log server stores its project from the path app.
A consumer splits logs by the app in the path.

## Adapter

The default shipper posts to `{endpoint}/log/{app}`.
A consumer that maps the app to a different path uses an adapter.
The adapter is outside this shipper.

## Buffer-only mode

An unset endpoint selects buffer-only mode.
An empty endpoint selects buffer-only mode.
An endpoint that has only spaces selects buffer-only mode.
Buffer-only mode does not build an HTTP client.
Buffer-only mode does not open an HTTP connection.
`LogShipper::spawn_ship_loop` in buffer-only mode does not remove buffered events.
`LogShipper::spawn_ship_loop` in buffer-only mode does not open an HTTP connection.
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
let shipper = LogShipper::new(ShipperConfig::new("cs", "https://logs.example")).unwrap();
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

`RequestJson::sanitize_untrusted_client` checks one batch.
The input is one `RequestJson`.
The result is a list of events, or no batch.
An empty `events` list returns no batch.
The function returns no batch when one event gives no event.
The function does not call `LogShipper::record`.
The caller calls `LogShipper::record` for each kept event.
`body` is one `RequestJson`.
`shipper` is one `LogShipper`.

```rust
if let Some(events) = body.sanitize_untrusted_client() {
    for event in events {
        shipper.record(event);
    }
}
```

The app keeps the HTTP handler.
This function does not set an HTTP status.
This function does not set a body size limit.

## Out of scope

This feature does not include these items:

- A log server process.
- Authentication to the log server.
- A product log policy.
- Trash rules, browse caps, or upload gates.
- An adapter that maps the app to a different path.
