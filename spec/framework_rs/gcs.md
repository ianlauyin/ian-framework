# GCS client

This document describes the Google Cloud Storage client in the framework.
This client is not product business logic.

## Crate

- The package name is `ian_framework`.
- The crate path is `framework_rs/`.
- The module path is `ian_framework::gcs`.
- `rust-toolchain.toml` gives the Rust version for this repository.

## Feature `gcs`

- The Cargo feature name is `gcs`.
- The `gcs` feature is optional.
- The default build does not compile this client.
- A project sets the Cargo feature `gcs` to compile this client.
- A project can select the dependency feature `ian_framework/gcs`.
- The logging module is in `spec/framework_rs/logging.md`.

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

- `GcsClient`
- `GcsError` and `GcsErrorKind`
- `ListPage` and `ListQuery`
- `ObjectMeta` and `ObjectReader`
- `PutBody`, `PutOptions` and `PutRequest`

These items are private to the crate:

- The URL encoding functions.
- The IAM `signBlob` call.

A project uses `GcsClient::signed_get_url` for a signed URL.

## Constructors

`GcsClient::from_adc` reads Application Default Credentials for one bucket.

## Authentication

The client accepts only Application Default Credentials.

- If there is no credentials file, metadata auth uses the service account on the VM metadata server.
- The client rejects a service account JSON key.
- The client rejects impersonated application default credentials.
- User application default credentials need `GCS_SIGNER_EMAIL`.
- `GCS_SIGNER_EMAIL` is the service account email that signs URLs.

Scopes:

- The metadata token requests the `cloud-platform` scope.
- That scope gives access to object calls and to `signBlob`.
- A user refresh token keeps the scopes from the original login.
- The command `gcloud auth application-default login` includes the `cloud-platform` scope.

## Object data

An object result has these fields:

- `name`
- `size`
- `content_type`
- `updated`
- The raw metadata map

A list page also has `prefixes` and `next_page_token`.

`GcsClient::list` takes one `ListQuery`. A `ListQuery` has these fields:

| Field | Type | Query parameter |
| --- | --- | --- |
| `prefix` | `Option<String>` | `prefix` |
| `delimiter` | `Option<String>` | `delimiter` |
| `page_token` | `Option<String>` | `pageToken` |
| `max_results` | `Option<u32>` | `maxResults` |

- The client sends a query parameter only when its field has a value.
- When `max_results` has a value, the client sends that value as `maxResults`. GCS returns a maximum of that number of items on the page.
- When `max_results` is `None`, the client sends no `maxResults`. Then GCS uses its own maximum page size.
- The client does not change the `max_results` value. The client has no limit for that value.
- The client does not remove prefixes.
- An empty `next_page_token` from GCS is `None`.
- The client does not follow `next_page_token`.
- A project writes its own page loop with its own limits.

## Put

`GcsClient::put` takes one `PutRequest`. A `PutRequest` has `name`, `body` and `options`.

| Body | Content |
| --- | --- |
| `PutBody::Bytes` | The object bytes in memory. |
| `PutBody::File` | A file path. |

- The client reads a file one chunk at a time.
- The client does not read the full file into memory.
- A path that is not a file is a configuration error.

## Metadata

The put options include a metadata map.

- The client does not write `uploaded-by`.
- The client does not write `uploaded-by-id`.
- A project can put those keys in the map.

## Signed GET

The client builds a V4 signed GET URL.

- The signature comes from IAM `signBlob`.
- The client percent-encodes each path segment.
- Only the RFC 3986 unreserved characters stay unencoded.

### Lifetime

`GcsClient::signed_get_url` takes an optional lifetime.

- A missing lifetime is an authentication error.
- A lifetime of zero is an authentication error.
- A lifetime below 1 second is an authentication error.
- The client does not sign `X-Goog-Expires=0`.
- The maximum lifetime is 604800 seconds.
- A lifetime above that maximum is a configuration error.

### Attachment filename

`GcsClient::signed_get_url` takes an optional attachment filename.

- With a filename, the URL signs the query parameter `response-content-disposition`. GCS then sends that value as the `Content-Disposition` response header.
- The value is `attachment; filename="{fallback}"; filename*=UTF-8''{encoded}`.
- `{fallback}` is the filename with `_` in place of each non-ASCII character, control character, `"`, and `\`.
- `{encoded}` is the percent-encoded UTF-8 filename.
- An empty filename is a configuration error.
- Without a filename, the URL has no `response-content-disposition`.

## HTTP client

One `GcsClient` has one HTTP client. The storage calls, the resumable upload, the token requests and the IAM `signBlob` call use that client.

- The connect time limit is 10 seconds.
- The HTTP client does not follow redirects. The client uses a redirect response as the final response.

## Transfer time limit

The transfer time limit is 180 seconds. These items use it:

- The Linux HTTP client sets `TCP_USER_TIMEOUT` to the transfer time limit.
- Each resumable chunk request.
- Each session probe.

## Resumable upload

The client uploads bytes with a resumable session. The client knows the object size before the upload starts.

Chunks:

- A chunk that is not the last chunk is 8 MiB.
- The last chunk can be shorter.
- The last chunk of an empty object has an empty body.
- Each chunk request sends `Content-Length`, also when the body is empty. GCS answers 411 to a chunk request that has no `Content-Length`.

The client retries a chunk in these conditions:

- The transport fails.
- The HTTP status is 408, 429, 500, 502, 503, or 504.

After one of those failures, the client probes the session:

- The probe request has an empty body and `Content-Length: 0`.
- The probe uses `Content-Range: bytes */{total}`.
- A probe response with status 308 can include `Range: bytes=START-END`. Then the next byte offset is `END + 1`.
- A probe response with status 308 and no `Range` header gives offset 0.

Attempts:

- The client makes a maximum of 5 attempts for one offset.
- The client resets that count when the committed offset increases.

Rewinds:

- A rewind occurs when a chunk step ends with a next offset that is lower than the start offset of that chunk. Then the client reads the data again from the lower offset and sends it again.
- The client accepts a maximum of 5 rewinds.
- The client resets the rewind count when a chunk step ends with a higher offset.
- The sixth rewind before a higher offset stops the upload with a `Transport` error.
- A chunk step that ends with a next offset equal to the start offset of that chunk stops the upload with an `Api` error.
- A next offset that is larger than the object size stops the upload with an `Api` error.

## Rewrite and move

`GcsClient::rewrite_object` copies an object in the same bucket with the JSON rewrite API.

- It sends `rewriteToken` again until `done` is true.
- If `rewriteToken` does not change, it returns an error.

`GcsClient::move_object` rewrites the source to the destination. Then it deletes the source.

A failed delete does not prove that the source still exists. If that delete fails, `GcsClient::move_object` reads the source metadata. Then it does one of these steps:

| Metadata read result | Action | Error |
| --- | --- | --- |
| The source exists. | It deletes the destination. | It returns the source delete error. If the destination delete also fails, the error tells that the object exists at the two names. The error includes the two errors. |
| The source does not exist. | It keeps the destination. | The error tells that the object exists only at the destination. |
| The metadata read fails. | It keeps the destination. | The error tells that the object can exist at the two names. The error includes the two errors. |

- Each of these errors has the kind of the source delete error.
- A source and a destination with the same name is a configuration error.

## Missing objects

When the object does not exist, the methods give these results:

| Methods | Result |
| --- | --- |
| `GcsClient::delete_object` | `Ok` |
| `get_bytes_opt`, `open_object_opt`, `stat_opt` | `None` |
| `get_bytes`, `open_object`, `stat` | A `NotFound` error |

## Operations

These methods are on `GcsClient`:

| Method | Effect |
| --- | --- |
| `put` | Puts one `PutRequest`. |
| `get_bytes` | Gets the object bytes. |
| `get_bytes_opt` | Gets the object bytes or `None`. |
| `open_object` | Opens the object data. |
| `open_object_opt` | Opens the object data or returns `None`. |
| `stat` | Reads the object metadata. |
| `stat_opt` | Reads the object metadata or returns `None`. |
| `list` | Lists one page. |
| `delete_object` | Deletes one object. |
| `patch_metadata` | Patches the metadata map. |
| `rewrite_object` | Copies an object in the same bucket with `rewriteToken`. |
| `move_object` | Moves an object in the same bucket. |
| `signed_get_url` | Builds a V4 signed GET URL. |
| `bucket` | Returns the bucket name. |
| `signer_email` | Returns the signer email. |

`ObjectReader` has one method. `byte_stream` reads the object data as a byte stream.

## Operations log

The client writes operations records through the logging module. The log route is in `spec/framework_rs/logging.md`.

Shipper:

- `GcsClient::attach_ops_log` connects one log shipper to one client.
- A second call replaces that shipper.
- A client with no shipper writes no operations record.
- A logging failure does not change the storage result.

Record content:

- An operations record has no `app` field.
- An operations record has no `project` field.
- The client does not write product rules in these records.
- The client does not write access tokens.
- The record `action` is `{method} {path}`. One space separates the method and the path.
- The path is the URL path. The client does not write the URL query.
- A token record has an empty `action`.
- An operations record has an empty `id`.
- An operations record has an empty `ip`.
- An operations record has an empty `user_agent`.
- Each record has `bucket` in `extra`. The value is the bucket name.

| Case | Level | Message |
| --- | --- | --- |
| HTTP status 500 or higher | `error` | `gcs response` |
| HTTP status 400 through 499 | `warn` | `gcs response` |
| HTTP transport failure | `warn` | `gcs request failed` |
| First HTTP 401 for one call | `warn` | `gcs access token rejected` |
| Resumable chunk failure | `warn` | `gcs resumable chunk failed` |
| Resumable probe failure | `warn` | `gcs resumable probe failed` |
| Token request failure | `warn` | `access token request failed` |

HTTP 401 rules:

- The first HTTP 401 does not also write `gcs response`.
- The client requests a new access token and sends that call again.
- A later HTTP 401 writes `gcs response`.

Failure records:

- A failure record can include `error` in `extra`.
- An `error` value does not contain a URL query.
- A `GcsError` source does not contain the request URL.
- A resumable chunk failure record includes `attempt` and `offset` in `extra`.
- A resumable probe failure record also includes `attempt` and `offset` in `extra`.

These results write no operations record:

- A response below 400.
- A successful resumable probe.
- A successful token request.

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
