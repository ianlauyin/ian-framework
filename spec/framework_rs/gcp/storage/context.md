# GCS client

This document describes the Google Cloud Storage client in the framework.
This client is not product business logic.

## Sub-features

| Sub-feature | Document |
| --- | --- |
| Authentication | `spec/framework_rs/gcp/storage/authentication.md` |
| Object data, put, rewrite, move and operations | `spec/framework_rs/gcp/storage/objects.md` |
| Signed GET URL | `spec/framework_rs/gcp/storage/signed-url.md` |
| Operations log | `spec/framework_rs/gcp/storage/operations-log.md` |

## Crate

- The package name is `ian_framework`.
- The crate path is `framework_rs/`.
- The module path is `ian_framework::gcp::storage`.
- The source files are in `framework_rs/src/gcp/storage/`. `mod.rs` declares the modules and re-exports the public items. `client.rs` has `GcsClient`. Each other file has one part of the client: `list.rs`, `object.rs`, `put_request.rs` (also the upload file check), `signed_url.rs` (also the signer build) and `validate.rs` (the bucket name, object name and metadata checks).
- The tests are in `framework_rs/src/test/gcp/storage/`. They have the same file shape.

## Feature `gcs`

- The Cargo feature name is `gcs`.
- The `gcs` feature is optional.
- The default build does not compile this client.
- The `gcs` feature enables the `gcp` feature. The shared Google Cloud code is in `spec/framework_rs/gcp/context.md`.
- The client uses the official crates `google-cloud-storage` and `google-cloud-auth`.
- The logging module is in `spec/framework_rs/logging/context.md`.

Put this block in the project `Cargo.toml`:

```toml
[dependencies]
ian_framework = { git = "https://github.com/ianlauyin/ian-framework", rev = "<commit>", features = ["gcs"] }
```

The crate is not on crates.io. A project adds the crate as a git dependency. The project pins `rev` to one commit.

Use these commands in `framework_rs/`:

```sh
cargo check
cargo check --features gcs
cargo test --features gcs
```

## Public items

`ian_framework::gcp::storage` exports only these items:

- `GcsClient`
- `ListPage` and `ListQuery`
- `ObjectMeta` and `ObjectReader`
- `PutBody`, `PutOptions` and `PutRequest`

Each item has only one public path.

## Constructors

`pub async fn connect_with_adc(bucket_name: impl Into<String>, signer_email: Option<&str>) -> Result<GcsClient, Exception>` makes a client for one bucket from Application Default Credentials.

- A bucket name that is empty, is not ASCII, or has `/`, `\`, a space or a line break is a `BUCKET_NAME_INVALID` error.
- `signer_email` is the service account email that signs URLs. The project reads it from its own env variable. The rules are in `spec/framework_rs/gcp/storage/authentication.md`.

## Clients and retries

One `GcsClient` has one `Storage` client for the data calls and one `StorageControl` client for the metadata calls. Both are from `google-cloud-storage`. The bucket path is `projects/_/buckets/{bucket}`.

- The official clients retry a transient failure of an idempotent call. The library sets no retry policy.
- The official clients refresh the access token.
- The library does not set an HTTP timeout, a redirect rule or a user agent. The official clients decide.

## Error code

The codes are in `spec/framework_rs/gcp/context.md`.

- A failure on the local host is `FILE_OPEN_FAILED` or `FILE_METADATA_FAILED`. The client does not retry it.
- An input that is not valid has its own code, such as `BUCKET_NAME_INVALID`, `OBJECT_NAME_INVALID` or `METADATA_INVALID`.
- `is_retryable` is true for the codes `TRANSPORT_FAILED` and `SERVICE_UNAVAILABLE`.

## HTTP error

`Exception` implements `IntoResponse`, as in `spec/framework_rs/http/api-error.md`. A handler that returns `Result<T, Exception>` can use `?` on an `Exception` from this client.

- No code of this client is a well-known code. Thus the status is 500 and the message is `internal server error`, as in `spec/framework_rs/http/api-error.md`.
- The message does not contain the internal message or the trace.

## Out of scope

The client does not include these items:

- Trash rules.
- Browse caps.
- Upload gates.
- Part stores and Cloudflare uploads.
- Thumbnails and media rules.
- Rename policy for a prefix or a folder.
- Axum and a product Store.
- Code for tokens, HTTP, retries, resumable upload or request signing. The official crates do it.
