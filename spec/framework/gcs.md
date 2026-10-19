# GCS client

This document describes the Google Cloud Storage client in the framework.
This client is not product business logic.

## Crate

The package name is `ian_framework`.
The crate path is `framework/`.
The module path is `ian_framework::gcs`.
`rust-toolchain.toml` gives the Rust version for this repository.

## Feature `gcs`

The Cargo feature name is `gcs`.
The `gcs` feature is optional.
The default build does not compile this client.
A project sets the Cargo feature `gcs` to compile this client.
A project can select the dependency feature `ian_framework/gcs`.
The `gcs` feature enables the `logging` feature.
The logging feature is `spec/framework/logging.md`.

Put this block in the project `Cargo.toml`:

```toml
[dependencies]
ian_framework = { version = "0.1", features = ["gcs"] }
```

Use these commands in `framework/`:

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

- `GcsBackend::from_adc` reads Application Default Credentials for one bucket.
- `GcsBackend::new` takes one bucket and an explicit auth value.

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
`GcsBackend::signed_get_url` takes an optional lifetime.
A missing lifetime is an authentication error.
A lifetime of zero is an authentication error.
A lifetime below 1 second is an authentication error.
The client does not sign `X-Goog-Expires=0`.
The maximum lifetime is 604800 seconds.
A lifetime above that maximum is a configuration error.

## Transfer time limit

The transfer time limit is 180 seconds.
The Linux HTTP client sets `TCP_USER_TIMEOUT` to the transfer time limit.
Each resumable chunk request uses the transfer time limit.
Each session probe uses the transfer time limit.

## Resumable upload

The client uploads bytes with a resumable session.
A chunk that is not the last chunk is 8 MiB.
The last chunk can be shorter.
The client retries a chunk in these cases:

- The transport fails.
- The HTTP status is 408, 429, 500, 502, 503, or 504.

The client probes the session after one of those failures.
The probe request has an empty body.
The probe uses `Content-Range: bytes */{total}` when the object size is known.
The probe uses `Content-Range: bytes */*` when the object size is not known.
A probe response with status 308 can include `Range: bytes=START-END`.
The next byte offset is `END + 1`.
A probe response with status 308 and no `Range` header means offset 0.
The client makes up to 5 attempts for one offset.
The client resets that count when the committed offset increases.
A byte stream cannot send bytes that it has already dropped.
If the session offset moves before those bytes, the upload fails.

## Copy and rewrite

`GcsBackend::copy_object` copies an object in the same bucket with one `copyTo` call.
`GcsBackend::rewrite_object` copies an object in the same bucket with the JSON rewrite API.
`GcsBackend::rewrite_object` sends `rewriteToken` again until `done` is true.
If `rewriteToken` does not change, `GcsBackend::rewrite_object` returns an error.

## Operations

These methods are on `GcsBackend`:

- `put_bytes` puts bytes.
- `put_file` puts a file path.
- `put_stream` puts a byte stream.
- `get_bytes` gets the object bytes.
- `open_object` opens the object data.
- `stat` reads the object metadata.
- `list` lists one page.
- `delete_object` deletes one object.
- `patch_metadata` patches the metadata map.
- `copy_object` copies an object in the same bucket with `copyTo`.
- `rewrite_object` copies an object in the same bucket with `rewriteToken`.
- `signed_get_url` builds a V4 signed GET URL.
- `sign_blob` signs bytes with IAM `signBlob`.
- `bucket` returns the bucket name.
- `signer_email` returns the signer email.

These methods are on `ObjectReader`:

- `read_to_vec` reads the object bytes.
- `byte_stream` reads the object data as a byte stream.

## Operations log

The client writes operations records through the logging feature.
`GcsBackend::attach_ops_log` connects one log shipper to one client.
A second call replaces that shipper.
An operations record has no `app` field.
An operations record has no `project` field.
The log route is `spec/framework/logging.md`.
A client with no shipper writes no operations record.
A logging failure does not change the storage result.
The client does not write product rules in these records.
The client does not write access tokens.
The record `action` is `{method} {path}`.
One space separates the method and the path.
The path is the URL path.
The client does not write the URL query.
A token record has an empty `action`.
An operations record has an empty `id`.
An operations record has an empty `ip`.
An operations record has an empty `user_agent`.
Each record has `bucket` in `extra`.
The value is the bucket name.

| case | level | message |
| --- | --- | --- |
| HTTP status 500 or higher | `error` | `gcs response` |
| HTTP status 400 through 499 | `warn` | `gcs response` |
| other HTTP response | `debug` | `gcs response` |
| HTTP transport failure | `warn` | `gcs request failed` |
| first HTTP 401 for one call | `warn` | `gcs access token rejected` |
| resumable chunk failure | `warn` | `gcs resumable chunk failed` |
| resumable probe failure | `warn` | `gcs resumable probe failed` |
| resumable probe | `debug` | `gcs resumable probe` |
| metadata token request | `debug` | `request metadata server access token` |
| user token refresh | `debug` | `refresh user access token` |
| token request failure | `warn` | `access token request failed` |

The first HTTP 401 does not also write `gcs response`.
The client requests a new access token and sends that call again.
A later HTTP 401 writes `gcs response`.
A failure record can include `error` in `extra`.
An `error` value does not contain a URL query.
A resumable chunk record includes `attempt` and `offset` in `extra`.
The `gcs resumable probe` record includes `attempt`, `offset`, and `next` in `extra`.
The shipper `minimum_level` selects which records stay in the buffer.
The default minimum is `info`.
A `debug` record stays out of the buffer.

## Out of scope

The client does not include these items:

- Trash rules.
- Browse caps.
- Upload gates.
- Part stores and Cloudflare uploads.
- Thumbnails and media rules.
- Move and rename policy.
- Axum and a product Store.
- Extra access-token caches. The client keeps one access-token cache.
- `Content-Disposition` for attachments.
