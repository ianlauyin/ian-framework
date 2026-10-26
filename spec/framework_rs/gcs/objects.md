# GCS objects

The parts that apply to all of this feature are in `spec/framework_rs/gcs/context.md`.

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
