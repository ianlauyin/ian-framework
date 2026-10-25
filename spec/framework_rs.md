# Rust framework features

This file is the index of the feature documents for the Rust library in `framework_rs/`.

Request and event logging is `spec/framework_rs/logging.md`.
The shipper posts each batch to `/log/{app}`.
`app` is the routing name.
The browser log route `POST /log` is in `spec/framework_rs/logging.md`.
The Google Cloud Storage client is `spec/framework_rs/gcs.md`.
The HTTP boundary error, the request header helpers, and the process version route are `spec/framework_rs/http.md`.
Cookie sessions and password hashes are `spec/framework_rs/auth.md`.
