# Hong Kong time

The parts that apply to all of this feature are in `spec/framework_rs/time/context.md`.

## Public items

`ian_framework::time` exports these items:

| Item | Kind |
| --- | --- |
| `hkt_now` | Function |

## hkt_now

`hkt_now() -> chrono::DateTime<chrono::FixedOffset>` gives the current time in Hong Kong time (HKT).

- HKT is the fixed offset UTC+08:00. HKT has no daylight saving time.
- The result has the offset `+08:00`.
- The RFC 3339 text of the result ends with `+08:00`.
- The function does not use the time zone of the host.

## Users in the framework

| User | Use |
| --- | --- |
| `spec/framework_rs/logging/event.md` | The `ts` of each event that the framework makes. |
| `spec/framework_rs/gcs/operations-log.md` | The `ts` of each operations record. |
