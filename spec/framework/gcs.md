# GCS client

This document describes the Google Cloud Storage client in the framework.
This client is not product business logic.
The repository map is `spec/00_structure.md`.

## Crate

The package name is `ian_framework`.
The crate path is `crates/ian_framework/`.
The module path is `ian_framework::gcs`.
`rust-toolchain.toml` gives the Rust version for this repository.

## Feature `gcs`

The Cargo feature name is `gcs`.
The default features are empty.
The default build does not compile this client.
A project sets the Cargo feature `gcs` to compile this client.
A project can select the dependency feature `ian_framework/gcs`.

Put this block in the project `Cargo.toml`:

```toml
[dependencies]
ian_framework = { version = "0.1", features = ["gcs"] }
```

Use these commands in this repository:

```sh
cargo check
cargo check --features gcs
cargo test --features gcs
```

`cargo check` checks the default build.
`cargo check --features gcs` checks the build with the `gcs` feature.
`cargo test --features gcs` runs the tests for the `gcs` feature.

## Constructors

The client has two constructors.

- `gcs_from_adc` reads Application Default Credentials for one bucket.
- `gcs_backend` takes one bucket and an explicit auth value.

## Authentication

- Metadata auth uses the service account on the VM metadata server.
- The client rejects a service account JSON key.
- The client rejects impersonated application default credentials.
- User application default credentials need `GCS_SIGNER_EMAIL`.
- `GCS_SIGNER_EMAIL` is the service account email that signs URLs.

The metadata token requests the `cloud-platform` scope.
That scope permits object calls and `signBlob`.
A user refresh token keeps the scopes from the original login.
The command `gcloud auth application-default login` includes the `cloud-platform` scope.

## Object data

An object result has these fields:

- `name`
- `size`
- `content_type`
- `updated`
- the raw metadata map

A list page also has `prefixes` and `next_page_token`.
The client does not remove prefixes.
The client does not set a maximum page size.

## Metadata

The put options include a metadata map.
The client does not write `uploaded-by`.
The client does not write `uploaded-by-id`.
A project can put those keys in the map.

## Signed GET

The client builds a V4 signed GET URL.
The signature comes from IAM `signBlob`.
Each path segment uses `encode_seg`.

## Operations

The client gives these operations:

- Put bytes, a file path, or a byte stream.
- Get the object bytes.
- Read object data.
- List one page.
- Delete one object.
- Patch the metadata map.
- Copy an object in the same bucket.
- Build a V4 signed GET URL.

## Out of scope

The client does not include these items:

- Trash rules.
- Browse caps.
- Axum.
- A product Store.
