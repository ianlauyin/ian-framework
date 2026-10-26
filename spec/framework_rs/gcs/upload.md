# GCS upload

The parts that apply to all of this feature are in `spec/framework_rs/gcs/context.md`.

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
