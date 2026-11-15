# GCS operations log

The parts that apply to all of this feature are in `spec/framework_rs/gcp/storage/context.md`.

## Operations log

The client writes operations records through the logging module. The log route is in `spec/framework_rs/logging/shipper.md`.

Logger:

- The client uses the global logger, `Logger::get()`. The client has no logger of its own.
- When the global logger is not set, the client writes no operations record.
- A logging failure does not change the storage result.

When a record is written:

- The client writes a record only when an operation fails. A success writes no record. The operations are `read`, `get_meta`, `list`, `write`, `copy`, `merge_metadata`, `delete` and `sign`. `move_object` is a `copy`, a `delete` and a `get_meta`.
- The failure of an official call is final. The official client has already done the retries.
- A `RESOURCE_NOT_FOUND` failure writes no record, because `read_object`, `get_object_meta` and `delete_object` use it as a normal result.
- A failure of the body read of an `ObjectReader` writes no record.

Record content:

- The level is `error`.
- The record has `error_message` with the log text of the exception, with every source, its code, its file and its line.
- An operations record has no `app` field.
- An operations record has no `project` field.
- The client does not write product rules in these records.
- The record `action` is `GCS/{operation}:/{object name}`. For `list`, the name is the prefix.
- An operations record has a new UUID v7 `id`.
- An operations record has an empty `context`.
- The record has `latency_ms` of the operation.
- Each record has one key in `extra`: `bucket` with the bucket name. The record has no `message` key.
- The record `ts` is the current HKT time, from `ian_framework::time::hkt_now()`. The time rules are in `spec/framework_rs/time/hkt.md`.
