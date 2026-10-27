# Rust framework features

This file is the index of the feature documents for the Rust library in `framework_rs/`.

Each feature has one folder. The `context.md` file of each folder gives the parts that apply to all of the feature and the list of the sub-feature documents.

| Feature | Document |
| --- | --- |
| Request and event logging, and the browser log route `POST /log` | `spec/framework_rs/logging/context.md` |
| Google Cloud Storage client | `spec/framework_rs/gcs/context.md` |
| HTTP boundary error, request header helpers, same-origin middleware, and process version route | `spec/framework_rs/http/context.md` |
| Cookie sessions and password hashes | `spec/framework_rs/auth/context.md` |
| Shutdown signal `ShutdownReceiver` | `spec/framework_rs/shutdown/context.md` |
| Environment variable read helpers | `spec/framework_rs/env/context.md` |

The log shipper posts each batch to `/log/{app}`. `app` is the routing name.
