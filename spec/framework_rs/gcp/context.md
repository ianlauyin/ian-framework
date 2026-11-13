# Google Cloud shared code and Secret Manager access

This document describes the shared Google Cloud code in the framework.
This code is not product business logic.

## Sub-features

| Sub-feature | Document |
| --- | --- |
| Secret Manager secret access | `spec/framework_rs/gcp/secret-manager.md` |
| Cloud Storage client (`gcp::storage`, feature `gcs`) | `spec/framework_rs/gcp/storage/context.md` |

## Crate

- The package name is `ian_framework`.
- The crate path is `framework_rs/`.
- The module path is `ian_framework::gcp`.
- The source files are `framework_rs/src/gcp/mod.rs`, `framework_rs/src/gcp/error.rs`, `framework_rs/src/gcp/project.rs` and `framework_rs/src/gcp/secret_manager.rs`.
- The tests are in `framework_rs/src/test/gcp/`.

## Build

The Cargo feature `gcp` compiles this module.

- The `gcp` feature is optional. The default build does not compile it.
- The `gcs` feature enables the `gcp` feature.
- The module uses the official Google Cloud crates. The `gcp` feature uses `google-cloud-secretmanager-v1` and `google-cloud-gax`. The `gcs` feature adds `google-cloud-storage`, `google-cloud-auth` and `google-cloud-wkt`.
- The official crates do the authentication, the access tokens, the HTTP transport and the retries. The module has no code for them.

Use these commands in `framework_rs/`:

```sh
cargo check --features gcp
cargo test --features gcp
cargo test --all-features
cargo clippy --all-features --all-targets
```

## Credentials

The official crates read Application Default Credentials. The order is the variable `GOOGLE_APPLICATION_CREDENTIALS`, the well-known `gcloud` file, and then the metadata server.

- These Google Cloud standard variables are not app settings. The library can read them.
- The library does not read a service account key by its own code. The official crates decide which credential types they accept.

## Project

`framework_rs/src/gcp/project.rs` finds the project id for Secret Manager.

- The project id is the variable `GOOGLE_CLOUD_PROJECT` when it is not blank.
- Else the project id is the answer of `GET http://169.254.169.254/computeMetadata/v1/project/project-id` with the header `Metadata-Flavor: Google`. The time limit is 2 seconds.
- When the metadata server does not answer or answers a status that is not 2xx, there is no project.
- A project id that is empty or has a character that is not an ASCII letter, a digit, `-`, `_` or `.` is a `PROJECT_ID_INVALID` error.
- The whitespace at the start and at the end of the project id is removed.

## Error type

The error of this module is `ian_framework::exception::Exception`. Its definition is in `spec/framework_rs/exception.md`.

`framework_rs/src/gcp/error.rs` maps an error of an official crate to a code. The other codes name one specific error, such as `BUCKET_NAME_INVALID`, `SECRET_VALUE_EMPTY` or `CLIENT_BUILD_FAILED`. They are in the feature documents. The check order is the authentication error, then the HTTP status, then the gRPC status, then the kind of the error.

| Code | Condition |
| --- | --- |
| `AUTH_FAILED` | The credentials fail, or the status is HTTP 401 or 403, or gRPC `Unauthenticated` or `PermissionDenied`. |
| `RESOURCE_NOT_FOUND` | The status is HTTP 404 or gRPC `NotFound`. |
| `SERVICE_UNAVAILABLE` | The status is HTTP 408, 429, 500, 502, 503 or 504, or gRPC `DeadlineExceeded`, `ResourceExhausted` or `Unavailable`. |
| `TRANSPORT_FAILED` | The call has a timeout, a connection failure, an I/O failure or no retry left. |
| `API_FAILED` | A different failure, such as a different status or a response that is not valid. |

- The error source is the error of the official crate.
- `RESOURCE_NOT_FOUND` and `SERVICE_UNAVAILABLE` are also used by the storage client. The rules are in `spec/framework_rs/gcp/storage/context.md`.

## Public items

`ian_framework::gcp::SecretManager` is public. The project calls it and decides the fallback to an environment variable. The `env` module does not use it.

The other items of the module are private to the crate. The `gcp::storage` module is public with the `gcs` feature.

## Out of scope

This module does not include these items:

- Code for tokens, HTTP, retries or request signing. The official crates do it.
- Secret Manager calls other than `access`.
- A cache of secret values.
