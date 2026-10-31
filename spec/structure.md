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
| Google Cloud Storage client | `spec/framework_rs/gcs/context.md` |
| HTTP boundary error, request header helpers, same-origin middleware, and process version route | `spec/framework_rs/http/context.md` |
| Cookie sessions and password hashes | `spec/framework_rs/auth/context.md` |
| Shutdown signal `ShutdownReceiver` | `spec/framework_rs/shutdown/context.md` |
| Environment variable read helpers. With the `gcp` feature, `required` and `optional` read Secret Manager first. | `spec/framework_rs/env/context.md` |
| Shared Google Cloud code (Cargo feature `gcp`): credentials, access tokens, HTTP client settings and Secret Manager access | `spec/framework_rs/gcp/context.md` |
| Time and time zone helpers, Hong Kong time `hkt_now` | `spec/framework_rs/time/context.md` |

## Frontend features

| Feature | Document |
| --- | --- |
| Process version checker | `spec/framework_fe/version.md` |
| Ajax client | `spec/framework_fe/ajax.md` |
| App error boundary | `spec/framework_fe/error-boundary.md` |
| Browser logger | `spec/framework_fe/logger.md` |
| Time helpers, Hong Kong time `hktIsoString` | `spec/framework_fe/time.md` |
