# GCS signed URL

The parts that apply to all of this feature are in `spec/framework_rs/gcs/context.md`.

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
