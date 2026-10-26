# GCS client

This document describes the Google Cloud Storage client in the framework.
This client is not product business logic.

## Crate

The package name is `ian_framework`.
The crate path is `framework_rs/`.
The module path is `ian_framework::gcs`.
`rust-toolchain.toml` gives the Rust version for this repository.

## Feature `gcs`

The Cargo feature name is `gcs`.
The `gcs` feature is optional.
The default build does not compile this client.
A project sets the Cargo feature `gcs` to compile this client.
A project can select the dependency feature `ian_framework/gcs`.
The logging module is `spec/framework_rs/logging.md`.

Put this block in the project `Cargo.toml`:

```toml
[dependencies]
ian_framework = { version = "0.1", features = ["gcs"] }
```

Use these commands in `framework_rs/`:

```sh
cargo check
cargo check --features gcs
cargo test --features gcs
```

`cargo check` checks the build with no optional feature.
`cargo check --features gcs` checks the build with the `gcs` feature.
`cargo test --features gcs` runs the tests for the `gcs` feature.

## Public items

`ian_framework::gcs` exports only these items:

- `GcsClient`
- `GcsError` and `GcsErrorKind`
- `ListPage` and `ListQuery`
- `ObjectMeta` and `ObjectReader`
- `PutBody`, `PutOptions` and `PutRequest`

The URL encoding functions are private to the crate.
The IAM `signBlob` call is private to the crate.
A project uses `GcsClient::signed_get_url` for a signed URL.

## Constructors

`GcsClient::from_adc` reads Application Default Credentials for one bucket.

## Authentication

The client accepts only Application Default Credentials.

- If no credentials file is present, metadata auth uses the service account on the VM metadata server.
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
An empty `next_page_token` from GCS is `None`.
The client does not follow `next_page_token`.
A project writes its own page loop with its own limits.

## Put

`GcsClient::put` takes one `PutRequest`.
A `PutRequest` has `name`, `body` and `options`.
`PutBody::Bytes` holds the object bytes in memory.
`PutBody::File` holds a file path.
The client reads a file one chunk at a time.
The client does not read the whole file into memory.
A path that is not a file is a configuration error.

## Metadata

The put options include a metadata map.
The client does not write `uploaded-by`.
The client does not write `uploaded-by-id`.
A project can put those keys in the map.

## Signed GET

The client builds a V4 signed GET URL.
The signature comes from IAM `signBlob`.
Each path segment is percent-encoded.
Only the RFC 3986 unreserved characters stay unencoded.
`GcsClient::signed_get_url` takes an optional lifetime.
A missing lifetime is an authentication error.
A lifetime of zero is an authentication error.
A lifetime below 1 second is an authentication error.
The client does not sign `X-Goog-Expires=0`.
The maximum lifetime is 604800 seconds.
A lifetime above that maximum is a configuration error.

`GcsClient::signed_get_url` takes an optional attachment filename.
With a filename, the URL signs the query parameter `response-content-disposition`.
GCS then sends that value as the `Content-Disposition` response header.
The value is `attachment; filename="{fallback}"; filename*=UTF-8''{encoded}`.
`{fallback}` is the filename with `_` in place of each non-ASCII character, control character, `"`, and `\`.
`{encoded}` is the percent-encoded UTF-8 filename.
An empty filename is a configuration error.
Without a filename, the URL has no `response-content-disposition`.

## Transfer time limit

The transfer time limit is 180 seconds.
The Linux HTTP client sets `TCP_USER_TIMEOUT` to the transfer time limit.
Each resumable chunk request uses the transfer time limit.
Each session probe uses the transfer time limit.

## Resumable upload

The client uploads bytes with a resumable session.
The client knows the object size before the upload starts.
A chunk that is not the last chunk is 8 MiB.
The last chunk can be shorter.
The last chunk of an empty object has an empty body.
Each chunk request sends `Content-Length`, also when the body is empty.
GCS answers 411 to a chunk request that has no `Content-Length`.
The client retries a chunk in these cases:

- The transport fails.
- The HTTP status is 408, 429, 500, 502, 503, or 504.

The client probes the session after one of those failures.
The probe request has an empty body and `Content-Length: 0`.
The probe uses `Content-Range: bytes */{total}`.
A probe response with status 308 can include `Range: bytes=START-END`.
The next byte offset is `END + 1`.
A probe response with status 308 and no `Range` header means offset 0.
The client makes up to 5 attempts for one offset.
The client resets that count when the committed offset increases.

## Rewrite and move

`GcsClient::rewrite_object` copies an object in the same bucket with the JSON rewrite API.
`GcsClient::rewrite_object` sends `rewriteToken` again until `done` is true.
If `rewriteToken` does not change, `GcsClient::rewrite_object` returns an error.

`GcsClient::move_object` rewrites the source to the destination.
Then it deletes the source.
A failed delete does not prove that the source still exists.
If that delete fails, `GcsClient::move_object` reads the source metadata. Then it does one of these steps:

- If the source exists, it deletes the destination and returns the delete error. If the destination delete also fails, the error tells that the object exists at both names, and it includes the two errors.
- If the source does not exist, it keeps the destination. The error tells that the object exists only at the destination.
- If the metadata read fails, it keeps the destination. The error tells that the object can exist at both names, and it includes the two errors.

Each of these errors has the kind of the source delete error.
A source and a destination with the same name is a configuration error.

## Missing objects

`GcsClient::delete_object` returns `Ok` when the object does not exist.
`get_bytes_opt`, `open_object_opt` and `stat_opt` return `None` when the object does not exist.
`get_bytes`, `open_object` and `stat` return a `NotFound` error when the object does not exist.

## Operations

These methods are on `GcsClient`:

- `put` puts one `PutRequest`.
- `get_bytes` gets the object bytes.
- `get_bytes_opt` gets the object bytes or `None`.
- `open_object` opens the object data.
- `open_object_opt` opens the object data or returns `None`.
- `stat` reads the object metadata.
- `stat_opt` reads the object metadata or returns `None`.
- `list` lists one page.
- `delete_object` deletes one object.
- `patch_metadata` patches the metadata map.
- `rewrite_object` copies an object in the same bucket with `rewriteToken`.
- `move_object` moves an object in the same bucket.
- `signed_get_url` builds a V4 signed GET URL.
- `bucket` returns the bucket name.
- `signer_email` returns the signer email.

This method is on `ObjectReader`:

- `byte_stream` reads the object data as a byte stream.

## Operations log

The client writes operations records through the logging module.
`GcsClient::attach_ops_log` connects one log shipper to one client.
A second call replaces that shipper.
An operations record has no `app` field.
An operations record has no `project` field.
The log route is `spec/framework_rs/logging.md`.
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
| HTTP transport failure | `warn` | `gcs request failed` |
| first HTTP 401 for one call | `warn` | `gcs access token rejected` |
| resumable chunk failure | `warn` | `gcs resumable chunk failed` |
| resumable probe failure | `warn` | `gcs resumable probe failed` |
| token request failure | `warn` | `access token request failed` |

The first HTTP 401 does not also write `gcs response`.
The client requests a new access token and sends that call again.
A later HTTP 401 writes `gcs response`.
A failure record can include `error` in `extra`.
An `error` value does not contain a URL query.
A resumable chunk record includes `attempt` and `offset` in `extra`.
A response below 400 writes no operations record.
A successful resumable probe writes no operations record.
A successful token request writes no operations record.

## HTTP error

`GcsError` implements `ian_framework::http::IntoApiError`.
A handler that returns `Result<T, ApiError>` can use `?` on a `GcsError`.

| kind | status | code | message |
| --- | --- | --- | --- |
| `NotFound` | 404 | `gcs_not_found` | `storage object not found` |
| `Auth` | 500 | `gcs_auth` | `storage is not available` |
| `Config` | 500 | `gcs_config` | `storage is not available` |
| `Api` | 502 | `gcs_api` | `storage request failed` |
| `Transport` | 502 | `gcs_transport` | `storage request failed` |

The status depends only on the kind.
The GCS response status does not change the HTTP status.
The message is fixed for each kind.
The message does not contain the internal `GcsError` message.

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
