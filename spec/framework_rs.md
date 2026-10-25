# Rust framework features

This file is the index of the feature documents for the Rust library in `framework_rs/`.

Request and event logging is `spec/framework_rs/logging.md`.
The shipper posts each batch to `/log/{app}`.
`app` is the routing name.
The Google Cloud Storage client is `spec/framework_rs/gcs.md`.
The HTTP boundary error and the request header helpers are `spec/framework_rs/http.md`.
Cookie sessions and password hashes are `spec/framework_rs/auth.md`.
The process version route is `spec/framework_rs/version.md`.
