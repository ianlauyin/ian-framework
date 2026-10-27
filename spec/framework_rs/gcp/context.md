# Google Cloud credentials and Secret Manager access

This document describes the shared Google Cloud code in the framework.
This code is not product business logic.

## Sub-features

| Sub-feature | Document |
| --- | --- |
| Application Default Credentials and access tokens | `spec/framework_rs/gcp/credentials.md` |
| Secret Manager secret access | `spec/framework_rs/gcp/secret-manager.md` |

## Crate

- The package name is `ian_framework`.
- The crate path is `framework_rs/`.
- The module path is `ian_framework::gcp`.
- The source files are `framework_rs/src/gcp/mod.rs`, `framework_rs/src/gcp/error.rs`, `framework_rs/src/gcp/credentials.rs`, `framework_rs/src/gcp/http_client.rs` and `framework_rs/src/gcp/secret_manager.rs`.
- The tests are in the same source files. The shared test helpers are in the test module of `framework_rs/src/gcp/mod.rs`: a one-request local test server, a closed local address and a fake `SecretManager`.
- `rust-toolchain.toml` gives the Rust version for this repository.

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

- The `gcs` module uses the credentials and the access tokens. The GCS client is in `spec/framework_rs/gcs/context.md`.
- The `gcs` module and the Secret Manager access use the same HTTP client settings. The settings are in the section "HTTP client".
- The `env` module uses the Secret Manager access. The lookup of `env::required` and `env::optional` is in `spec/framework_rs/env/read.md`.

## HTTP client

`http_client_builder` gives the shared `reqwest` client builder for the Google Cloud calls. It is private to the crate.

- The connect time limit is 10 seconds.
- The HTTP client does not follow redirects.
- The user agent is `ian_framework/{crate version}`.
- Each user builds its client from this builder. The GCS client adds more settings. They are in `spec/framework_rs/gcs/context.md`.

## Error type

`GcpError` is private to the crate. It has a kind, a message and an optional source.

| Kind | Condition |
| --- | --- |
| `Auth` | A token request fails, the metadata server gives a project status that is not 2xx, or Secret Manager gives status 401 or 403. |
| `Config` | The credentials, the project id or the secret id are not valid, or the secret value is empty or not UTF-8. |
| `Api` | Secret Manager gives a different status that is not 2xx and not 404, or the response body is not valid. |
| `Transport` | The HTTP request fails. |

- `Display` gives the message.
- `Debug` gives the kind and the message.
- The `gcs` module changes a `GcpError` to a `GcsError` with the same kind and message. The source stays.

## Out of scope

This module does not include these items:

- The `google-cloud-*` crates.
- A service account JSON key.
- Secret Manager calls other than `access`.
- A cache of secret values.
