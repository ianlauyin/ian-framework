# Rust framework features

This file is the index of the feature documents for the Rust library in `framework_rs/`.

| Feature | Document |
| --- | --- |
| Request and event logging | `spec/framework_rs/logging.md` |
| Browser log route `POST /log` | `spec/framework_rs/logging.md` |
| Google Cloud Storage client | `spec/framework_rs/gcs.md` |
| HTTP boundary error, request header helpers, same-origin middleware, and process version route | `spec/framework_rs/http.md` |
| Cookie sessions and password hashes | `spec/framework_rs/auth.md` |
| Shutdown signal `ShutdownReceiver` | `spec/framework_rs/shutdown.md` |

The log shipper posts each batch to `/log/{app}`. `app` is the routing name.
