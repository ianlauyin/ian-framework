# GCS client

This document describes the Google Cloud Storage client in the framework.
This client is not product business logic.

## Sub-features

| Sub-feature | Document |
| --- | --- |
| Authentication | `spec/framework_rs/gcs/authentication.md` |
| Object data, rewrite, move and operations | `spec/framework_rs/gcs/objects.md` |
| Put, metadata and resumable upload | `spec/framework_rs/gcs/upload.md` |
| Signed GET URL | `spec/framework_rs/gcs/signed-url.md` |
| Operations log | `spec/framework_rs/gcs/operations-log.md` |

## Crate

- The package name is `ian_framework`.
- The crate path is `framework_rs/`.
- The module path is `ian_framework::gcs`.
- The client module path is `ian_framework::gcs::gcs_client`.
- `rust-toolchain.toml` gives the Rust version for this repository.

## Feature `gcs`

- The Cargo feature name is `gcs`.
- The `gcs` feature is optional.
- The default build does not compile this client.
- The `gcs` feature enables the `gcp` feature. The shared Google Cloud code is in `spec/framework_rs/gcp/context.md`.
- A project sets the Cargo feature `gcs` to compile this client.
- A project can select the dependency feature `ian_framework/gcs`.
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

| Command | Effect |
| --- | --- |
| `cargo check` | Checks the build with no optional feature. |
| `cargo check --features gcs` | Checks the build with the `gcs` feature. |
| `cargo test --features gcs` | Runs the tests for the `gcs` feature. |

## Public items

`ian_framework::gcs` exports only these items:

- `GcsError` and `GcsErrorKind`
- The module `gcs_client`

`ian_framework::gcs::gcs_client` exports only these items:

- `GcsClient`
- `ListPage` and `ListQuery`
- `ObjectMeta` and `ObjectReader`
- `PutBody`, `PutOptions` and `PutRequest`

Each item has only one public path.

These items are private to the crate:

- The URL encoding functions.
- The IAM `signBlob` call.

A project uses `GcsClient::signed_get_url` for a signed URL.

## Constructors

`GcsClient::from_adc` reads Application Default Credentials for one bucket.

## HTTP client

One `GcsClient` has one HTTP client. The storage calls, the resumable upload, the token requests and the IAM `signBlob` call use that client.

- The HTTP client uses the shared settings in `spec/framework_rs/gcp/context.md`. Thus the connect time limit is 10 seconds, and the client does not follow redirects.
- The client uses a redirect response as the final response.
- On Linux, the client also sets `TCP_USER_TIMEOUT`. The rule is in `spec/framework_rs/gcs/upload.md`.

## Error kind

The client gives the `GcsErrorKind` from the GCS response status:

| GCS response status | Kind |
| --- | --- |
| 401, 403 | `Auth` |
| 404 | `NotFound` |
| A different status that is not 2xx | `Api` |

- This table applies to the storage calls and to the IAM `signBlob` call.
- A transport failure is `Transport`.
- A token request with a status that is not 2xx is `Auth`. This includes the metadata server.
- A resumable chunk response or a probe response with status 308 gives the next offset. It does not give an error kind.

## HTTP error

`GcsError` implements `ian_framework::http::IntoApiError`. A handler that returns `Result<T, ApiError>` can use `?` on a `GcsError`.

| Kind | Status | Code | Message |
| --- | --- | --- | --- |
| `NotFound` | 404 | `gcs_not_found` | `storage object not found` |
| `Auth` | 500 | `gcs_auth` | `storage is not available` |
| `Config` | 500 | `gcs_config` | `storage is not available` |
| `Api` | 502 | `gcs_api` | `storage request failed` |
| `Transport` | 502 | `gcs_transport` | `storage request failed` |

- The status depends only on the kind.
- The GCS response status does not change the HTTP status.
- The message is fixed for each kind.
- The message does not contain the internal `GcsError` message.

## Out of scope

The client does not include these items:

- Trash rules.
- Browse caps.
- Upload gates.
- Part stores and Cloudflare uploads.
- Thumbnails and media rules.
- Rename policy for a prefix or a folder.
- Axum and a product Store.
- Extra access-token caches. The client keeps one access-token cache.
