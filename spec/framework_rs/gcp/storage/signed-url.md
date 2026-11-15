# GCS signed URL

The parts that apply to all of this feature are in `spec/framework_rs/gcp/storage/context.md`.

## Signed GET

The client builds a V4 signed GET URL. The signer is in `spec/framework_rs/gcp/storage/authentication.md`.

- The client builds the canonical request, the percent-encoding and the signature.
- The client checks the object name, the lifetime and the filename before it signs.
- The host is `storage.googleapis.com`.
- The credential scope ends with `/auto/storage/goog4_request`. The date in that scope is the UTC day of the signature.
- The signed header is `host`.
- The payload line is `UNSIGNED-PAYLOAD`.
- A query name and a query value keep the RFC 3986 unreserved characters. Every other byte is `%HH` with uppercase hex. `*` is `%2A`. A `%` byte that is already in the value is `%25`.

### Lifetime

`GcsClient::sign_download_url` takes a lifetime of type `Duration`. The lifetime is not optional.

- A lifetime below 1 second is a `SIGNED_URL_LIFETIME_TOO_SHORT` error. This includes zero.
- The maximum lifetime is 604800 seconds.
- A lifetime above that maximum is a `SIGNED_URL_LIFETIME_TOO_LONG` error.
- The client cuts the lifetime to whole seconds.

### Attachment filename

`GcsClient::sign_download_url` takes an optional attachment filename.

- With a filename, the URL signs the query parameter `response-content-disposition`. GCS then sends that value as the `Content-Disposition` response header.
- The value is `attachment; filename="{fallback}"; filename*=UTF-8''{encoded}`.
- `{fallback}` is the filename with `_` in place of each non-ASCII character, control character, `"`, and `\`.
- `{encoded}` is the percent-encoded UTF-8 filename. Only the RFC 3986 unreserved characters stay unencoded.
- An empty filename is a `ATTACHMENT_FILENAME_EMPTY` error.
- Without a filename, the URL has no `response-content-disposition`.

A failure to sign is a `SIGN_URL_FAILED` error.
