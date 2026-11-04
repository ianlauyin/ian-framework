# Log shipper

The parts that apply to all of this feature are in `spec/framework_rs/logging/context.md`.

## Shipper

### ShipperConfig

`ShipperConfig::new(app, endpoint, post_api_key, mask_patterns)` builds one `ShipperConfig` for one app. The default mask patterns are in `spec/framework_rs/logging/masking.md`.

- `ShipperConfig::new` does not read an environment variable or Secret Manager.
- The app reads its own environment variables or secrets, with the names that the app chooses. Then it gives the values to the config. The app can use `ian_framework::env::optional` for this read. The lookup rules of `optional` are in `spec/framework_rs/env/read.md`.
- `post_api_key` is the POST API key of the log server. It is a secret.
- The `Debug` output of `ShipperConfig` does not show the `post_api_key` value. The output shows `***`.
- The ship request time limit is 10 seconds.
- The connect time limit of the ship HTTP client is 10 seconds.
- The ship HTTP client does not follow redirects.

### LogShipper::new

`LogShipper::new` builds one shipper from that config. One shipper has one app. The app is the routing name.

- The buffer capacity is 20000 events. The ship interval is 60 seconds. The config does not set them.
- The app is not checked. The caller gives a valid app name.

`LogShipper::new` returns `LogShipper`. It panics in these conditions:

- A `mask_patterns` item is not a valid regex. The message names the pattern.
- The POST API key is not a valid HTTP header value.
- The HTTP client cannot be built.

`LogShipper::new` writes one `tracing` info event with the field `app` and the message `log shipping posts to the log server`.

### Endpoint and POST API key

- The shipper uses `endpoint` as given. It does not trim, parse or check it.
- The shipper uses `post_api_key` as given. It does not trim or check it, except that the header value must be valid.
- The `Debug` output of `LogShipper` does not show the POST API key. The output shows `***`.

### Record

`LogShipper::record` stores one event in the local buffer.

- Before it stores the event, `LogShipper::record` masks the event `context` values and `extra` values.
- The shipper does not write `app` into the event.
- The shipper stores the full `action`, `id`, and `context`.
- The buffer keeps a maximum of 20000 events.
- When the buffer is full, a new event removes the oldest event.

### Capacity warning

After `LogShipper::record` stores the event, the shipper reads the buffer length.

- The 80 percent limit is the smallest whole number that is at least 80 percent of the capacity.
- The length meets that limit when five times the length is at least four times the capacity.

| Capacity | 80 percent limit |
| --- | --- |
| 1 | 1 |
| 4 | 4 |
| 5 | 4 |
| 20000 | 16000 |

When the length meets the limit, `LogShipper::record` stores one new capacity warning.

- The shipper does not change the other events.
- The warning is one event at the end of the buffer.
- When the warning makes the buffer larger than the capacity, the shipper first removes the oldest event.
- The shipper stores the warning on the record that makes the length meet the limit.
- The shipper does not store the warning again while the length stays at or above the limit.

The warning has these fields:

| Field | Value |
| --- | --- |
| `ts` | Current HKT time, from `ian_framework::time::hkt_now()` |
| `id` | Empty string |
| `action` | `SHIPPER/capacity meets 80%` |
| `latency_ms` | null |
| `level` | `warn` |
| `error_message` | null |
| `context` | Empty object |
| `extra` | Empty object |

### Ship loop

`LogShipper::spawn_ship_loop` sends buffered events to the log server. The loop does the send again on an interval. One interval sends one batch.

The request has these properties:

- The HTTP method is POST.
- The URL is `{endpoint}/log/{app}`. `{app}` is the shipper app.
- The content type is `application/json`.
- The request has the header `Authorization: Bearer {key}`.
- The request time limit is 10 seconds.

The body is one JSON array of event objects.

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
- When that return makes the buffer larger than the capacity, the shipper removes the oldest events above the capacity.
- An empty buffer does not send an HTTP request.

Capacity warning after a ship:

- A successful ship that makes the buffer length lower than the 80 percent limit lets `LogShipper::record` store the capacity warning again. The next record that makes the length meet the limit stores one new warning.
- A successful ship that keeps the length at or above the limit does not store another warning.
- A failed ship does not store a second warning.
- When a failed ship puts the batch back and the length meets the limit, the next record does not store another warning.

Loop task:

- The caller runs `LogShipper::spawn_ship_loop` on a Tokio runtime.
- `LogShipper::spawn_ship_loop` takes one `ian_framework::shutdown::ShutdownReceiver`. The shutdown module is in `spec/framework_rs/shutdown/receiver.md`.
- It returns `tokio::task::JoinHandle<()>`.
- The interval is 60 seconds.
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
