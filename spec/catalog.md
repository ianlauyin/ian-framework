# Framework structure

This file is the index of the specs of the two libraries.

| Library | Code | Context |
| --- | --- | --- |
| Rust | `framework_rs/` | `spec/framework_rs/context.md` |
| Frontend | `framework_fe/` | `spec/framework_fe/context.md` |

The `context.md` file of a library gives the parts that apply to all of the library.

## Rust features

Each feature has one folder. The `context.md` file of each folder gives the parts that apply to all of the feature and the list of the sub-feature documents.

| Feature | Document |
| --- | --- |
| Request and event logging, and the browser log route `POST /log` | `spec/framework_rs/logging/context.md` |
| CPU and disk monitors (Cargo feature `monitor`) that make a `LogEvent` when the usage meets a threshold | `spec/framework_rs/monitor/context.md` |
| Google Cloud Storage client | `spec/framework_rs/gcp/storage/context.md` |
| HTTP boundary error, request header helpers, middleware for safety and security (same-origin, rate limit, response headers, API key, allowed IP, timeout, body limit), the process version route, and the health route | `spec/framework_rs/http/context.md` |
| RFC 9110 `token68` check | `spec/framework_rs/token/context.md` |
| Redis helpers (Cargo feature `redis`): `connect` makes a checked connection manager | `spec/framework_rs/redis/context.md` |
| Cookie sessions and password hashes | `spec/framework_rs/auth/context.md` |
| Shared error type `Exception` with a boxed source and a trace for the log | `spec/framework_rs/exception.md` |
| Shutdown signal `ShutdownReceiver` | `spec/framework_rs/shutdown/context.md` |
| Environment variable read helpers. | `spec/framework_rs/env/context.md` |
| Shared Google Cloud code (Cargo feature `gcp`): project id, error codes and Secret Manager access, built on the official Google Cloud crates | `spec/framework_rs/gcp/context.md` |
| Time and time zone helpers, Hong Kong time `hkt_now` | `spec/framework_rs/time/context.md` |

## Frontend features

| Feature | Document |
| --- | --- |
| Process version checker | `spec/framework_fe/version.md` |
| Ajax client | `spec/framework_fe/ajax.md` |
| Handler for the errors of the ajax client that no code caught | `spec/framework_fe/exception-handler.md` |
| File upload and file download | `spec/framework_fe/file-transfer.md` |
| Small helpers: typed storage and `sleep` | `spec/framework_fe/util.md` |
| App error boundary | `spec/framework_fe/solid/error-boundary.md` |
| Browser logger | `spec/framework_fe/logger.md` |
| Time helpers, Hong Kong time `hktIsoString` | `spec/framework_fe/time.md` |
