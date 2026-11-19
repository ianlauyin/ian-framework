# Rust Framework

This file is the catalog of the Rust library in `framework_rs/`.

- The decision records a behaviour that the code does not show. 
- Update that row in the same change when you make a new decision, the entry moves, or you add or remove a feature. 
- When a code change would change the decision and you did not make that decision, stop and ask. Leave the decision as it is.

| Path | Description | Decision |
| --- | --- | --- |
| `framework_rs/src/logging/` | Request and event logging, and the browser log route `POST /log` | An authorized POST is trusted, so the log server stores the `context` and the `extra` that the project sets. A flush stops after three failed batches, so a down log server does not keep the process open. `with_log` sets the latency after it builds the event, so the builder cannot replace the latency. The default mask patterns hide a credential in a field name and in a name-value pair, because the access log records the query and the JSON body. |
| `framework_rs/src/monitor/` | CPU and disk monitors | Disk usage follows `df`. The blocks that root cannot use are not free, and the blocks that only root can use are not counted. |
| `framework_rs/src/gcp/storage/` | Google Cloud Storage client | The official crates own authentication, token refresh, and retries. With no signer email, the signer is the account of the credentials, and that account must be a service account or the metadata server. An object name longer than 1024 bytes is refused, because GCS refuses a longer name. |
| `framework_rs/src/http/` | HTTP boundary error, request header helpers, safety and security middleware, the version route, the health route, and the SPA route | The access log writes `ip`, `user_agent`, and `request_url` after the handler fields, so a handler cannot replace those keys. When `Sec-Fetch-Site` is absent, the check compares `Origin` with `Host`, because a browser that does not send `Sec-Fetch-Site` still sends `Origin` on a POST. The cookie reader reads every `Cookie` field, because an HTTP/2 client can send each cookie in its own field. A bearer key compare uses the SHA-256 hashes, because slices of different lengths compare in less time. A full rate-limit map drops a batch of the least recently used buckets at one time, so a full scan is rare. The `on_reject` function is `Send` and `Sync`, so two calls can run at the same time. |
| `framework_rs/src/token.rs` | RFC 9110 `token68` check | A project can use the check to reject an API key at startup. |
| `framework_rs/src/redis.rs` | Redis connection helper | The connection fails fast when Redis is down. It does not retry for many seconds. |
| `framework_rs/src/auth/` | Cookie sessions and password hashes | A Redis key holds the hash of the token, so a copy of Redis does not give a working token. The subject set key is a hash of the subject, so the subject cannot change that key. The session TTL is at most 400 days, because a browser caps a cookie lifetime at 400 days. The session cookie and the clear cookie use the same attributes, `HttpOnly`, `Secure`, `SameSite=Lax`, and `Path=/`, because a browser removes the cookie only when the attributes match. |
| `framework_rs/src/exception.rs` | Shared error type `Exception` | The message and `Debug` do not show the source, because the source can hold a secret. |
| `framework_rs/src/shutdown.rs` | Shutdown signal `ShutdownReceiver` | `wait` returns a `'static` future that owns its receiver, so the caller does not borrow `ShutdownReceiver`. |
| `framework_rs/src/mutex.rs` | Standard mutex lock | |
| `framework_rs/src/env.rs` | Environment variable read helpers | The error holds the name and does not hold the value, because the value can be a secret. |
| `framework_rs/src/gcp/` | Shared Google Cloud code and Secret Manager access | The official crates read Application Default Credentials. The project chooses the fallback from Secret Manager to an environment variable. The library reads `GOOGLE_CLOUD_PROJECT`, which is a Google Cloud standard variable and not an app setting. An error does not hold the secret value or a parser error that can quote it. |
| `framework_rs/src/time/` | Hong Kong time `hkt_now` and `hkt_today`, and `Quarter` | Hong Kong time is the fixed offset UTC+08:00, and it has no daylight saving time. `first_day` and `last_day` return text of the year, the month, and the day, because the stored day is that text. |
