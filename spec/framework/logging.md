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

`event` builds one event.

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
| `level` | `debug`, `info`, `warn`, or `error` |
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
The HTTP header for `id` is `x-request-id`.

## Shipper

`shipper_config` builds one `ShipperConfig` for one app.
The project on that config starts empty.
A caller can set `project` on the config.
That project stays on the shipper.
That project is not an event field.
That config has no endpoint.
`minimum_level` starts at `info`.
`capacity` starts at 10000 events.
`batch_size` starts at 100 events.
`DEFAULT_CAPACITY` is 10000.
`DEFAULT_BATCH_SIZE` is 100.
`DEFAULT_SHIP_EVERY` is 5 seconds.
`SHIP_TIMEOUT` is 10 seconds.

`log_shipper` builds one shipper from that config.
One shipper has one app.
The app is the routing name.
An app name has 1 to 64 characters.
The app name starts with a letter or a digit.
The other characters are letters, digits, `.`, `_`, or `-`.
An empty app is a configuration error.
A forbidden character in the app is a configuration error.
An empty project is valid.
A project that is not empty uses the same character rules as the app.
A forbidden character in that project is a configuration error.
`capacity` of zero is a configuration error.
`batch_size` of zero is a configuration error.
`batch_size` above `capacity` is a configuration error.

`record` stores one event in the local buffer.
`record` does not write `app` into the event.
`record` does not write `project` into the event.
`record` does not store an event below `minimum_level`.
`record` stores the full `message`.
`record` stores the full `action`.
`record` stores the full `id`.
`record` stores the full `ip`.
`record` stores the full `user_agent`.
The buffer keeps at most `capacity` events.
When the buffer is full, a new event removes the oldest event.
`dropped` counts removed events.
`buffered_len` returns the number of buffered events.
`buffered_events` returns a copy of the buffered events.

`ship_batch` sends one batch to the log server.
The HTTP method is POST.
The URL is `{endpoint}/log/{app}`.
`{app}` is the shipper app.
For app `cs`, the path is `/log/cs`.
The content type is `application/json`.
The body is one JSON object.
The body shape is `{"events":[...]}`.
The body is not a bare JSON array.
`events` is one JSON array of event objects.
Each event object has every field in the event table.
Each event object has no `app` key.
Each event object has no `project` key.
The shipper does not put `project` in that URL.
The shipper does not send a project header.
One batch contains at most `batch_size` events.
The shipper takes the batch out of the buffer before the POST.
A response status from 200 through 299 leaves that batch out of the buffer.
A transport error puts that batch back at the front of the buffer.
An HTTP error status puts that batch back at the front of the buffer.
When that return makes the buffer larger than `capacity`, the shipper removes the newest events above the capacity.
The ship request time limit is 10 seconds.
`spawn_ship_loop` calls `ship_batch` on an interval.
An interval of zero is a configuration error.
The caller runs `spawn_ship_loop` on a Tokio runtime.
A ship error does not stop the loop.
An empty buffer with an endpoint does not send an HTTP request.
The result mode is `Empty`.
The result mode for a sent batch is `Sent`.

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
`shipper_config` selects buffer-only mode.
Buffer-only mode does not build an HTTP client.
Buffer-only mode does not open an HTTP connection.
`ship_batch` in buffer-only mode returns success.
The result mode is `BufferOnly`.
`ship_batch` in buffer-only mode does not remove buffered events.
`spawn_ship_loop` in buffer-only mode does not open an HTTP connection.
A missing log server does not panic the shipper.
An `http` or `https` endpoint sends batches.
The shipper removes a trailing slash from that URL.
An endpoint with a query is a configuration error.
An endpoint with a fragment is a configuration error.
An endpoint with whitespace is a configuration error.
A scheme other than `http` or `https` is a configuration error.

## Axum middleware

`log_request` is the Axum middleware function.
Use it with `axum::middleware::from_fn_with_state` and one shipper.

```rust
let shipper = log_shipper(shipper_config("cs"))?;
let router = Router::new().layer(middleware::from_fn_with_state(
    shipper,
    ian_framework::logging::log_request,
));
```

The middleware reads the header `x-request-id`.
A valid `x-request-id` has 1 to 128 characters.
Each character is a letter, a digit, `.`, `_`, `-`, or `:`.
A missing header gets a new id.
An invalid header gets a new id.
The middleware stores that id in the event field `id`.
The middleware writes that id into the response header `x-request-id`.
The middleware records one event after the inner handler returns.
The middleware does not set `app` on the event.
The middleware does not set `project` on the event.
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

## Out of scope

This feature does not include these items:

- A log server process.
- Authentication to the log server.
- A product log policy.
- Trash rules, browse caps, or upload gates.
- An adapter that maps the app to a different path.
