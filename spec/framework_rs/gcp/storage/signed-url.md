# GCS signed URL

The parts that apply to all of this feature are in `spec/framework_rs/gcp/storage/context.md`.

## Signed GET

The client builds a V4 signed GET URL with `SignedUrlBuilder` of `google-cloud-storage`. The signer is in `spec/framework_rs/gcp/storage/authentication.md`.

- The builder does the canonical request, the percent-encoding and the signature.
- The library checks the object name, the lifetime and the filename before it calls the builder.

### Lifetime

`GcsClient::signed_get_url` takes a lifetime of type `Duration`. The lifetime is not optional.

- A lifetime below 1 second is a `GCP_CONFIG` error. This includes zero.
- The maximum lifetime is 604800 seconds.
- A lifetime above that maximum is a `GCP_CONFIG` error.
- The client cuts the lifetime to whole seconds.

### Attachment filename

`GcsClient::signed_get_url` takes an optional attachment filename.

- With a filename, the URL signs the query parameter `response-content-disposition`. GCS then sends that value as the `Content-Disposition` response header.
- The value is `attachment; filename="{fallback}"; filename*=UTF-8''{encoded}`.
- `{fallback}` is the filename with `_` in place of each non-ASCII character, control character, `"`, and `\`.
- `{encoded}` is the percent-encoded UTF-8 filename. Only the RFC 3986 unreserved characters stay unencoded.
- An empty filename is a `GCP_CONFIG` error.
- Without a filename, the URL has no `response-content-disposition`.

A failure to sign is a `GCP_AUTH` error.
