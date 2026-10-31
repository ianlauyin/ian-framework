# Log shipper

The parts that apply to all of this feature are in `spec/framework_rs/logging/context.md`.

## Shipper

### ShipperConfig

`ShipperConfig::new` builds one `ShipperConfig` for one app. The argument is the app.

| Field | Start value |
| --- | --- |
| `endpoint` | Not set |
| `post_api_key` | Not set |
| `capacity` | 10000 events |
| `mask_patterns` | The default patterns in `spec/framework_rs/logging/masking.md` |
| `ship_every` | 5 seconds |

- `ShipperConfig::new` does not read an environment variable or Secret Manager.
- The fields are public. The app sets `endpoint` and `post_api_key` on the config before `LogShipper::new`.
- The app reads its own environment variables or secrets, with the names that the app chooses. Then it gives the values to the config. The app can use `ian_framework::env::optional` for this read. The lookup rules of `optional` are in `spec/framework_rs/env/read.md`.
- `post_api_key` is the POST API key of the log server. It is a secret.
- The `Debug` output of `ShipperConfig` does not show the `post_api_key` value. When `post_api_key` is set, the output shows `***`.
- The ship request time limit is 10 seconds.
- The connect time limit of the ship HTTP client is 10 seconds.
- The ship HTTP client does not follow redirects.

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
- The POST API key is not valid. The POST API key rules are in the POST API key section.

`LogShipper::new` writes one `tracing` info event with the field `app`. The message is one of these:

- `log shipping posts to the log server`, when the shipper has an endpoint.
- `log shipping is trace-only`, when the shipper has no endpoint.

### POST API key

The shipper first removes the whitespace at the start and at the end of `post_api_key`. Then it reads the key.

These keys give a shipper with no POST API key:

- A key that is not set.
- A key that is empty after the removal of the whitespace.

`LogShipper::new` returns no shipper for these keys:

- A key with whitespace inside it, after the removal of the whitespace at the start and at the end.
- A key that is not a valid HTTP header value.

The `Debug` output of `LogShipper` does not show the POST API key. When the shipper has a key, the output shows `***`.

### Record

`LogShipper::record` stores one event in the local buffer.

- Before it stores or traces the event, `LogShipper::record` masks the event `extra` values.
- In trace-only mode, `LogShipper::record` writes the event with `tracing`. It does not store the event.
- The shipper does not write `app` into the event.
- The shipper stores the full `action`, `id`, `ip`, and `user_agent`.
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
| `ts` | Current HKT time, from `ian_framework::time::hkt_now()` |
| `id` | Empty string |
| `ip` | Empty string |
| `user_agent` | Empty string |
| `action` | `SHIPPER/capacity meets 80%` |
| `latency_ms` | null |
| `level` | `warn` |
| `extra` | Empty object |

### Ship loop

`LogShipper::spawn_ship_loop` sends buffered events to the log server. The loop does the send again on an interval. One interval sends one batch.

The request has these properties:

- The HTTP method is POST.
- The URL is `{endpoint}/log/{app}`. `{app}` is the shipper app.
- The content type is `application/json`.
- When the shipper has a POST API key, the request has the header `Authorization: Bearer {key}`.
- When the shipper has no POST API key, the request has no `Authorization` header.
- The request time limit is 10 seconds.

The body is one JSON object of type `LogBatch`:

```json
{"events":[...]}
```

- The body is not a bare JSON array.
- `events` is one JSON array of event objects.
- Each event object has each field in the event table. The event table is in `spec/framework_rs/logging/event.md`.
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
- `LogShipper::spawn_ship_loop` takes one `ian_framework::shutdown::ShutdownReceiver`. The shutdown module is in `spec/framework_rs/shutdown/receiver.md`.
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

## Adapter

The default shipper posts to `{endpoint}/log/{app}`. A consumer that maps the app to a different path uses an adapter. The adapter is outside this shipper.

## Trace-only mode

The shipper first removes the whitespace at the start and at the end of the endpoint. Then it reads the endpoint.

These endpoints select trace-only mode:

- An endpoint that is not set.
- An endpoint that is empty after the removal of the whitespace. Thus an endpoint that has only whitespace selects trace-only mode.

In trace-only mode, `LogShipper::record` writes each event with `tracing` at the event level:

| Event level | Macro |
| --- | --- |
| `info` | `tracing::info!` |
| `warn` | `tracing::warn!` |
| `error` | `tracing::error!` |

- The tracing fields are `app`, `id`, `ip`, `user_agent`, `action`, `latency_ms`, and `extra`.
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
- An endpoint with whitespace inside it, after the removal of the whitespace at the start and at the end.
- An endpoint that is not a valid URL.
- An endpoint with no host.
- An endpoint with a scheme other than `http` or `https`.
