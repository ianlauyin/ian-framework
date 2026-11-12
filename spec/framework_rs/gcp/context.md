# Google Cloud credentials and Secret Manager access

This document describes the shared Google Cloud code in the framework.
This code is not product business logic.

## Sub-features

| Sub-feature | Document |
| --- | --- |
| Application Default Credentials and access tokens | `spec/framework_rs/gcp/credentials.md` |
| Secret Manager secret access | `spec/framework_rs/gcp/secret-manager.md` |
| Cloud Storage client (`gcp::storage`, feature `gcs`) | `spec/framework_rs/gcp/storage/context.md` |

## Crate

- The package name is `ian_framework`.
- The crate path is `framework_rs/`.
- The module path is `ian_framework::gcp`.
- The source files are `framework_rs/src/gcp/mod.rs`, `framework_rs/src/gcp/response.rs`, `framework_rs/src/gcp/credentials.rs`, `framework_rs/src/gcp/http_client.rs` and `framework_rs/src/gcp/secret_manager.rs`.
- The tests are in `framework_rs/src/test/gcp/`. The shared test helpers are in `framework_rs/src/test/gcp/mod.rs`: a one-request local test server, a scripted local test server that answers each request with the next reply, a closed local address, a fake `TokenSource` and a fake `SecretManager`.

## Build

The Cargo feature `gcp` compiles this module. The module has the credentials, the access tokens, the error type, the HTTP client settings and the Secret Manager access.

- The `gcp` feature is optional. The default build does not compile it.
- The `gcs` feature enables the `gcp` feature.
- The `gcp` feature does not use the `google-cloud-*` crates. It uses `reqwest`, `serde_json` and `base64`.

Use these commands in `framework_rs/`:

```sh
cargo check --features gcp
cargo test --features gcp
cargo test --all-features
cargo clippy --all-features --all-targets
```

## Public items

`ian_framework::gcp` has no public items. All items are private to the crate.

- The `gcp::storage` module uses the credentials and the access tokens. The GCS client is in `spec/framework_rs/gcp/storage/context.md`.
- The `gcp::storage` module and the Secret Manager access use the same HTTP client settings. The settings are in the section "HTTP client".
- `ian_framework::gcp::SecretManager` is public. The project calls it and decides the fallback to an environment variable. The `env` module does not use it.

## HTTP client

`http_client_builder` gives the shared `reqwest` client builder for the Google Cloud calls. It is private to the crate.

- The connect time limit is 10 seconds.
- The HTTP client does not follow redirects.
- The user agent is `ian_framework/{crate version}`.
- Each user builds its client from this builder. The GCS client adds more settings. They are in `spec/framework_rs/gcp/storage/context.md`.

## Response

`framework_rs/src/gcp/response.rs` has the shared response helpers. They are private to the crate.

- `read_limited` reads a response body with the `Limited` body of `http-body-util`. The limit is 256 KiB. The caller gives the error code and the name of the body.
- A body larger than the limit is an error with the message `{name} is larger than 256 KiB`. The client stops the read at the limit.
- A body read failure is an error with the message `{name} is not readable`. The error source is the reqwest error. The source does not have the URL.
- Each token response, metadata response, Secret Manager response and GCS error response uses `read_limited`.
- `is_retryable_status` is true for the status 408, 429, 500, 502, 503 and 504. Secret Manager and GCS use this list.
- `google_error_message` gives the `error.message` field or the `error_description` field of a Google error body.
- `truncate` cuts a text at a byte length. The cut is on a character boundary.
## Error type

The error of this module is `ian_framework::exception::Exception`. Its definition is in `spec/framework_rs/exception.md`. The module uses only the codes in the table.

| Code | Condition |
| --- | --- |
| `GCP_AUTH` | A token request fails, the metadata server gives a project status that is not 2xx, or Secret Manager gives status 401 or 403. |
| `GCP_CONFIG` | The credentials, the project id or the secret id are not valid, or the secret value is empty or not UTF-8. |
| `GCP_API` | Secret Manager gives a different status that is not 2xx and not 404, or the response body is not valid. |
| `GCP_TRANSPORT` | The HTTP request fails. |

- The `gcp::storage` module uses the same four codes with the same meaning. It adds the codes `GCS_NOT_FOUND`, `GCS_UNAVAILABLE` and `GCS_LOCAL`. The rules are in `spec/framework_rs/gcp/storage/context.md`.

## Out of scope

This module does not include these items:

- The `google-cloud-*` crates.
- A service account JSON key.
- Secret Manager calls other than `access`.
- A cache of secret values.
