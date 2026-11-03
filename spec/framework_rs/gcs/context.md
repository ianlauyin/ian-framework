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

`pub async fn from_adc(bucket_name: impl Into<String>, signer_email: Option<&str>) -> Result<GcsClient, Exception>` reads Application Default Credentials for one bucket.

- `signer_email` is the service account email that signs URLs. The project reads it from its own env variable.
- User application default credentials require `Some` signer email. The rules are in `spec/framework_rs/gcs/authentication.md`.

## HTTP client

One `GcsClient` has one HTTP client. The storage calls, the resumable upload, the token requests and the IAM `signBlob` call use that client.

- The HTTP client uses the shared settings in `spec/framework_rs/gcp/context.md`. Thus the connect time limit is 10 seconds, and the client does not follow redirects.
- The client uses a redirect response as the final response.
- On Linux, the client also sets `TCP_USER_TIMEOUT`. The rule is in `spec/framework_rs/gcs/upload.md`.
- On each platform, the client also sets a read time limit. The rule is in `spec/framework_rs/gcs/upload.md`.

## Call retry

`GcsClient` makes each storage call, token-bound call and `signBlob` call through one send function. The send function retries a failed call.

- A call has at most 4 attempts.
- The send function retries a transport failure and a response with status 408, 429, 500, 502, 503 or 504.
- The wait before a retry is 250 milliseconds times the number of attempts made. The first wait is 250 milliseconds.
- A retry writes the same operations log record as the first failure.
- The resumable chunk request and the session probe make one attempt in the send function. They have their own retry loop in `spec/framework_rs/gcs/upload.md`.
- The retry for a 401 response is separate. It does not count as an attempt.

## Error code

The client gives the exception code from the GCS response status:

| GCS response status | Code |
| --- | --- |
| 401, 403 | `gcs_auth` |
| 404 | `gcs_not_found` |
| A different status that is not 2xx | `gcs_api` |

- This table applies to the storage calls and to the IAM `signBlob` call.
- A transport failure is `gcs_transport`. This includes only a failure of the network call or of the response body.
- A failure on the local host is `gcs_local`. This includes a failure to read the upload file and a failure to build the HTTP client. The client does not retry it. An upstream session failure is not a local failure. It has a `gcs_api` code and a retryable status.
- The message of a response error has the detail of the response body. The detail has no query string, no control character and at most 200 bytes. The client splits the text into tokens. A token ends at whitespace or at one of `"`, `'`, `<`, `>`, `)`, `\`, `,`, `;`, `]` and `}`. The client changes the tokens as follows:

  - A token with a `?` and a `/` before the first `?` (an absolute URL, a scheme-less URL or a path) loses the `?` and the text after it.
  - The value of a `key=value` pair becomes `***` when the key is `upload_id`, `access_token`, `refresh_token`, `id_token`, `token`, `signature`, `sig`, `client_secret`, `password`, `secret` or `authorization`, or starts with `X-Goog-`. The client does this also for a pair with no `?`. The comparison ignores the letter case.
  - The token after `Authorization:`, `Bearer` or `Basic` becomes `***`.
  - Other text, such as a `?` in a plain sentence, stays.
- A token request with a status that is not 2xx is `gcs_auth`. This includes the metadata server.
- A resumable chunk response or a probe response with status 308 gives the next offset. It does not give an error code.

## HTTP error

`Exception` implements `ian_framework::http::IntoApiError`. The table is in `spec/framework_rs/exception.md`. A handler that returns `Result<T, ApiError>` can use `?` on an `Exception` from this client.

- The status depends only on the code.
- The GCS response status does not change the HTTP status. The stored status of a `gcs` exception is the status of the upstream call.
- The message is fixed for each code.
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
- Extra access-token caches. The client keeps one access-token cache.
