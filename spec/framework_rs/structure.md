# Rust framework structure

This file is the index of the feature folders of the Rust library in `framework_rs/`.

The `spec/framework_rs/context.md` file gives the parts that apply to all of the library. Each feature folder has a `context.md` file. It gives the parts that apply to all of the feature and the list of the sub-feature documents.

| Feature | Document |
| --- | --- |
| Request and event logging, and the browser log route `POST /log` | `spec/framework_rs/logging/context.md` |
| Google Cloud Storage client | `spec/framework_rs/gcs/context.md` |
| HTTP boundary error, request header helpers, same-origin middleware, and process version route | `spec/framework_rs/http/context.md` |
| Cookie sessions and password hashes | `spec/framework_rs/auth/context.md` |
| Shutdown signal `ShutdownReceiver` | `spec/framework_rs/shutdown/context.md` |
| Environment variable read helpers. With the `gcp` feature, `required` and `optional` read Secret Manager first. | `spec/framework_rs/env/context.md` |
| Shared Google Cloud code (Cargo feature `gcp`): credentials, access tokens, HTTP client settings and Secret Manager access | `spec/framework_rs/gcp/context.md` |
| Time and time zone helpers, Hong Kong time `hkt_now` | `spec/framework_rs/time/context.md` |
