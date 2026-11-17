# Rust Framework

This file is the catalog of the Rust library in `framework_rs/`.

Each row has the entry path, a short description, and the open behaviour. Update that row in the same change when you make a new decision, the entry moves, or you add or remove a feature. When a code change would change the open behaviour and you did not make that decision, stop and ask. Leave the decision as it is.

| Path | Description | Decision |
| --- | --- | --- |
| `framework_rs/src/logging/` | Request and event logging, and the browser log route `POST /log` | The project gives the log server endpoint and the POST API key. The library does not read those settings from the environment. An authorized POST is trusted, so the log server stores the `context` and the `extra` that the project sets. |
| `framework_rs/src/monitor/` | CPU and disk monitors | The module compiles only with the Cargo feature `monitor`, and only on Linux. A monitor makes one `LogEvent` when the host value meets a threshold, and it gives the event to a sink that the caller supplies. |
| `framework_rs/src/gcp/storage/` | Google Cloud Storage client | The client compiles only with the Cargo feature `gcs`. The official crates own authentication, token refresh, and retries. The project gives the signer email. |
| `framework_rs/src/http/` | HTTP boundary error, request header helpers, safety and security middleware, the version route, the health route, and the SPA route | A handler can return `Result<T, Exception>`, and the library turns that exception into the HTTP response. The project attaches the middleware. The library serves the version route, the health route, and the SPA route. |
| `framework_rs/src/token.rs` | RFC 9110 `token68` check | `is_token68` accepts a text only when that text is a `token68`. A project can use the check to reject an API key at startup. |
| `framework_rs/src/redis.rs` | Redis connection helper | The module compiles only with the Cargo feature `redis`. `connect` returns a connection manager only when Redis replies `PONG` to `PING`. The error does not contain the Redis URL. A `rediss://` URL connects with TLS. |
| `framework_rs/src/auth/` | Cookie sessions and password hashes | The module compiles only with the Cargo feature `auth`, and that feature enables `redis`. A password has 8 to 256 bytes and contains an English letter and a digit. The library does not store users. |
| `framework_rs/src/exception.rs` | Shared error type `Exception` | `Exception` is the one error type of the library. Build one only with the `exception!` macro. The message and `Debug` do not show the source, because the source can hold a secret. |
| `framework_rs/src/shutdown.rs` | Shutdown signal `ShutdownReceiver` | All clones share one state. `SIGINT`, `SIGTERM`, or `trigger` sets that state. The state stays triggered. |
| `framework_rs/src/env.rs` | Environment variable read helpers | `required` and `optional` read only the environment variable of the name that the caller gives. They do not read Secret Manager. The error holds the name and does not hold the value. |
| `framework_rs/src/gcp/` | Shared Google Cloud code and Secret Manager access | The module compiles only with the Cargo feature `gcp`. The official crates read Application Default Credentials. The project chooses the fallback from Secret Manager to an environment variable. |
| `framework_rs/src/time/` | Hong Kong time `hkt_now` and `hkt_today`, and `Quarter` | The helpers use Hong Kong time only. Hong Kong time is the fixed offset UTC+08:00, and it has no daylight saving time. |
