# GCS operations log

The parts that apply to all of this feature are in `spec/framework_rs/gcs/context.md`.

## Operations log

The client writes operations records through the logging module. The log route is in `spec/framework_rs/logging/shipper.md`.

Shipper:

- `GcsClient::attach_ops_log` connects one log shipper to one client.
- A second call replaces that shipper.
- A client with no shipper writes no operations record.
- A logging failure does not change the storage result.

Record content:

- An operations record has no `app` field.
- An operations record has no `project` field.
- The client does not write product rules in these records.
- The client does not write access tokens.
- The record `action` is `GCS/{method}:{path}`. The path starts with `/`.
- The path is the URL path. The client does not write the URL query.
- A token record has an empty `action`.
- An operations record has an empty `id`.
- An operations record has an empty `ip`.
- An operations record has an empty `user_agent`.
- Each record has `bucket` in `extra`. The value is the bucket name.
- Each record has `message` in `extra`. The values are in the table below.
- A record with an HTTP status has `status` in `extra`. The value is the status as text.
- The record `ts` is the current HKT time, from `ian_framework::time::hkt_now()`. The time rules are in `spec/framework_rs/time/hkt.md`.

| Case | Level | `extra.message` |
| --- | --- | --- |
| HTTP status 500 or higher | `error` | `gcs response` |
| HTTP status 400 through 499 | `warn` | `gcs response` |
| HTTP transport failure | `warn` | `gcs request failed` |
| First HTTP 401 for one call | `warn` | `gcs access token rejected` |
| Resumable chunk failure | `warn` | `gcs resumable chunk failed` |
| Resumable probe failure | `warn` | `gcs resumable probe failed` |
| Token request failure | `warn` | `access token request failed` |

HTTP 401 rules:

- The first HTTP 401 does not also write `gcs response`.
- The client requests a new access token and sends that call again.
- A later HTTP 401 writes `gcs response`.

Failure records:

- A failure record can include `error` in `extra`.
- An `error` value does not contain a URL query.
- A `GcsError` source does not contain the request URL.
- A resumable chunk failure record includes `attempt` and `offset` in `extra`.
- A resumable probe failure record also includes `attempt` and `offset` in `extra`.

These results write no operations record:

- A response below 400.
- A successful resumable probe.
- A successful token request.
