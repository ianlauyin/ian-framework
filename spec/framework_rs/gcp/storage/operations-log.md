# GCS operations log

The parts that apply to all of this feature are in `spec/framework_rs/gcp/storage/context.md`.

## Operations log

The client writes operations records through the logging module. The log route is in `spec/framework_rs/logging/shipper.md`.

Logger:

- `GcsClient::attach_ops_log` connects one logger to one client.
- A second call replaces that logger.
- A client with no logger writes no operations record.
- A logging failure does not change the storage result.

When a record is written:

- The client writes one record when an operation fails. The operations are `open`, `stat`, `list`, `put`, `rewrite`, `patch`, `delete` and `sign`. `get_bytes` is an `open` and a read. `move_object` is a `rewrite`, a `delete` and a `stat`.
- The failure of an official call is final. The official client has already done the retries.
- A `GCS_NOT_FOUND` failure writes no record, because an `_opt` call and `delete_object` use it as a normal result.
- A success writes no record.
- A failure of the body read of an `ObjectReader` writes no record.

Record content:

- The level is `warn`.
- An operations record has no `app` field.
- An operations record has no `project` field.
- The client does not write product rules in these records.
- The record `action` is `GCS/{operation}:/{object name}`. For `list`, the name is the prefix.
- An operations record has a new UUID v7 `id`.
- An operations record has an empty `context`.
- The record has `latency_ms` of the failed operation.
- Each record has these keys in `extra`: `message` with the value `gcs operation failed`, `bucket` with the bucket name, and `error` with the log text of the exception, with every source, its code, its file and its line.
- The record `ts` is the current HKT time, from `ian_framework::time::hkt_now()`. The time rules are in `spec/framework_rs/time/hkt.md`.
