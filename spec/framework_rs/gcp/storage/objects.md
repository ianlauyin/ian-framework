# GCS objects

The parts that apply to all of this feature are in `spec/framework_rs/gcp/storage/context.md`.

## Object data

An `ObjectMeta` has these fields:

- `name`
- `size`. A size that is below zero is 0.
- `content_type`. An empty value is `None`.
- `updated`
- The raw metadata map

A list page also has `prefixes` and `next_page_token`.

`GcsClient::list` takes one `ListQuery`. A `ListQuery` has these fields:

| Field | Type | Request field |
| --- | --- | --- |
| `prefix` | `Option<String>` | `prefix` |
| `delimiter` | `Option<String>` | `delimiter` |
| `page_token` | `Option<String>` | `page_token` |
| `max_results` | `Option<u32>` | `page_size` |

- The client sends a field only when it has a value.
- A `max_results` above `i32::MAX` is `i32::MAX`. The client has no other limit for that value.
- When `max_results` is `None`, GCS uses its own maximum page size.
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

- The client uses `Storage::write_object` with `send_unbuffered`. The official client chooses a single request or a resumable upload, and does the retries.
- The client reads a file with the file source of the official crate. It does not read the full file into memory.
- A path that is not a file is a `NOT_A_FILE` error. A file that cannot be opened is a `FILE_OPEN_FAILED` error. A file whose metadata cannot be read is a `FILE_METADATA_FAILED` error.
- The content type is `application/octet-stream` when the options have none. A content type that is empty or has a byte that is not printable ASCII is a `CONTENT_TYPE_INVALID` error.
- The options include a metadata map. A key that is empty, or a key or a value that has a line break, is a `METADATA_INVALID` error.
- The client does not write `uploaded-by` or `uploaded-by-id`. A project can put those keys in the map.

## Patch metadata

`GcsClient::patch_metadata` reads the object, adds the new keys to its metadata map, and writes the map with an `UpdateObject` call and the update mask `metadata`.

- A new key replaces the key with the same name. The other keys stay.
- The two calls are not atomic. A change of the metadata between them can be lost.

## Rewrite and move

`GcsClient::rewrite_object` copies an object in the same bucket with the `RewriteObject` call.

- It sends the rewrite token again until `done` is true.
- If the token does not change, it returns a `REWRITE_TOKEN_STUCK` error.
- After 10 000 calls without `done`, it returns a `REWRITE_STEP_LIMIT` error.
- A done response with no object is a `REWRITE_NO_OBJECT` error.

`GcsClient::move_object` rewrites the source to the destination. Then it deletes the source. It does not use the `MoveObject` call, because that call needs a bucket with a hierarchical namespace.

A failed delete does not prove that the source still exists. If that delete fails, `GcsClient::move_object` reads the source metadata. Then it does one of these steps:

| Metadata read result | Action | Error |
| --- | --- | --- |
| The source exists. | It deletes the destination. | It returns the source delete error. If the destination delete also fails, the error tells that the object exists at the two names. The error includes the two errors. |
| The source does not exist. | It keeps the destination. | The error tells that the object exists only at the destination. |
| The metadata read fails. | It keeps the destination. | The error tells that the object can exist at the two names. The error includes the two errors. |

- Each of these errors has the code of the source delete error. The source delete error is the source of the error.
- A source and a destination with the same name is a `MOVE_SAME_OBJECT` error.
- An object name that is empty, is longer than 1024 bytes, is `.` or `..`, or has a line break is a `OBJECT_NAME_INVALID` error. The check occurs before the request.

## Missing objects

When the object does not exist, the methods give these results:

| Methods | Result |
| --- | --- |
| `GcsClient::delete_object` | `Ok` |
| `get_bytes_opt`, `open_object_opt`, `stat_opt` | `None` |
| `get_bytes`, `open_object`, `stat` | A `RESOURCE_NOT_FOUND` error |

## Operations

These methods are on `GcsClient`:

| Method | Effect |
| --- | --- |
| `put` | Puts one `PutRequest`. |
| `get_bytes` | Gets the object bytes. An object larger than 256 MiB is a `OBJECT_TOO_LARGE` error. Use `open_object` and `byte_stream` for a large object. |
| `get_bytes_opt` | Gets the object bytes or `None`. |
| `open_object` | Opens the object data. |
| `open_object_opt` | Opens the object data or returns `None`. |
| `stat` | Reads the object metadata. |
| `stat_opt` | Reads the object metadata or returns `None`. |
| `list` | Lists one page. |
| `delete_object` | Deletes one object. |
| `patch_metadata` | Adds keys to the metadata map. |
| `rewrite_object` | Copies an object in the same bucket. |
| `move_object` | Moves an object in the same bucket. |
| `signed_get_url` | Builds a V4 signed GET URL. |
| `bucket` | Returns the bucket name. |
| `attach_ops_log` | Connects a logger. |

`ObjectReader` has one method. `byte_stream` reads the object data as a stream of `bytes::Bytes` chunks. The stream does not copy a chunk.

The `Debug` output of an `ObjectReader` shows the object size.
