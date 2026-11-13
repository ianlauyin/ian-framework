# GCS operations log

The parts that apply to all of this feature are in `spec/framework_rs/gcp/storage/context.md`.

## Operations log

The client writes operations records through the logging module. The log route is in `spec/framework_rs/logging/shipper.md`.

Logger:

- The client uses the global logger, `Logger::get()`. The client has no logger of its own.
- When the global logger is not set, the client writes no operations record.
- A logging failure does not change the storage result.

When a record is written:

- The client writes one record for each operation, when the operation succeeds and when it fails. The operations are `open`, `stat`, `list`, `put`, `rewrite`, `patch`, `delete` and `sign`. `move_object` is a `rewrite`, a `delete` and a `stat`.
- The failure of an official call is final. The official client has already done the retries.
- A `RESOURCE_NOT_FOUND` failure is a success record, because `open_object`, `stat` and `delete_object` use it as a normal result.
- A failure of the body read of an `ObjectReader` writes no record.

Record content:

- The level is `info` for a success. The level is `error` for a failure.
- A failure record has `error_message` with the log text of the exception, with every source, its code, its file and its line. A success record has no `error_message`.
- An operations record has no `app` field.
- An operations record has no `project` field.
- The client does not write product rules in these records.
- The record `action` is `GCS/{operation}:/{object name}`. For `list`, the name is the prefix.
- An operations record has a new UUID v7 `id`.
- An operations record has an empty `context`.
- The record has `latency_ms` of the failed operation.
- Each record has these keys in `extra`: `message` with the value `gcs operation succeeded` or `gcs operation failed`, and `bucket` with the bucket name.
- The record `ts` is the current HKT time, from `ian_framework::time::hkt_now()`. The time rules are in `spec/framework_rs/time/hkt.md`.
