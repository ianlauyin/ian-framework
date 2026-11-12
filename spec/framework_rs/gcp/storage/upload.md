# GCS upload

The parts that apply to all of this feature are in `spec/framework_rs/gcp/storage/context.md`.

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
- The HTTP client on each platform sets the read time limit of a response to the transfer time limit. A stalled read ends the call. A long transfer that keeps data flowing does not end.
- Each resumable chunk request.
- Each session probe.

## Resumable upload

The client uploads bytes with a resumable session. The client knows the object size before the upload starts.

The session URL is the `Location` header of the start response. It must start with `https://storage.googleapis.com/`, because each chunk and probe request carries the access token. Another value is an `Api` error.

Chunks:

- A chunk that is not the last chunk is 8 MiB.
- The last chunk can be shorter.
- The last chunk of an empty object has an empty body.
- Each chunk request sends `Content-Length`, also when the body is empty. GCS answers 411 to a chunk request that has no `Content-Length`.

The client retries a chunk in these conditions:

- The transport fails (the code `GCP_TRANSPORT`).
- The HTTP status is 408, 429, 500, 502, 503, or 504. The list is the same list that the send function uses.

A failure to open, to read or to seek the upload file has the code `GCS_LOCAL`. The client does not retry it, because the file is on the local disk and the same read fails again.

After one of those failures, the client probes the session:

- The probe is a chunk request with an empty body and `Content-Length: 0`.
- The probe uses `Content-Range: bytes */{total}`.
- A probe response that has the same offset as the failed request causes the client to send the chunk at that offset again.
- A failed probe is followed by a new probe.

Chunk and probe response:

- A response with status 308 can include `Range: bytes=START-END`. Then the next byte offset is `END + 1`.
- A response with status 308 and no `Range` header gives offset 0. GCS sends no `Range` header when it has kept no bytes of the session. The client does not think that GCS kept the bytes that it sent.
- The client reads the next chunk from the next offset. The chunk is 8 MiB, or the rest of the object when the rest is shorter.

Attempts:

- The client makes a maximum of 5 requests for one offset that fail. Chunk requests and probes both count.
- After the fifth failed request, the client returns the error of that request.
- The client resets that count when the offset increases.

Rewinds:

- A rewind occurs when a response gives a next offset that is lower than the current offset. Then the client reads the data again from the lower offset and sends it again.
- The client accepts a maximum of 5 rewinds.
- The client resets the rewind count when a response gives a higher offset.
- The sixth rewind before a higher offset stops the upload with an error that has the code `GCS_UNAVAILABLE`. The session is an upstream failure, so `is_retryable` is true for this error. The client does not retry this error. A caller that retries the whole upload can use `is_retryable`.
- A chunk response, not a probe response, that gives a next offset equal to the current offset stops the upload with an `Api` error.
- A next offset that is larger than the object size stops the upload with an `Api` error.
- A next offset equal to the object size, without object metadata, stops the upload with an `Api` error.
