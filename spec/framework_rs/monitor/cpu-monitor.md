# CPU monitor

The parts that apply to all of this feature are in `spec/framework_rs/monitor/context.md`.

The CPU monitor checks the CPU usage of a Linux host on an interval. When the usage meets a threshold, it makes one `LogEvent` and gives the event to a sink that the caller supplies. The monitor does not know the shipper or the log server store.

## CpuMonitor

`CpuMonitor::new` takes `threshold_percent` in percent and `interval`. It does not read an environment variable. The app reads its own settings and gives the values. It returns no monitor in these conditions:

- `threshold_percent` is not more than 0 and not at most 100. A NaN value is not valid.
- `interval` is zero.

When it returns no monitor, it first calls `Logger::warn_with` one time with the action `CPU/invalidConfig`, an error whose display text is `the threshold must be more than 0 and at most 100, and the interval must not be zero`, and the context keys `threshold_percent` and `interval_seconds`.

`CpuMonitor::spawn` takes a `ian_framework::shutdown::ShutdownReceiver` and a sink. The sink is `Fn(LogEvent) + Send + 'static`. It returns `tokio::task::JoinHandle<()>`. The caller runs it on a Tokio runtime.

| Caller | Sink |
| --- | --- |
| Project backend on the VM | Calls `Logger::record`. The ship loop sends the event to the log server. |
| Log server | Writes the event to its SQLite store directly. It does not send the event to itself. |

## Sample

- The monitor reads the first line of `/proc/stat`, the total `cpu` line.
- The monitor takes the first sample when the task starts. It takes the next samples on each interval.
- The usage is the part of the total CPU time that is not idle time between two samples. Idle time is `idle` plus `iowait`.
- When the total time does not change, a counter goes down, or the idle time is more than the total time, the monitor skips that interval.
- When the sum of the fields overflows, the sample is not valid.
- When the first sample cannot be read, the monitor calls `Logger::error` one time with the action `CPU/cannotReadStat`, an error whose display text is `cannot read the cpu stat file` and the context key `path`, and the task ends. The logger rules are in `spec/framework_rs/logging/logger.md`.
- When a later sample cannot be read, the monitor skips that interval. On the first failure after a good read, it calls `Logger::error` with the same action, error text and context as above. It does not log again until a read works.

## Event

The monitor makes one event when the usage rises to the threshold or above it. It makes no new event while the usage stays at or above the threshold. When the usage falls below the threshold, the monitor can make a new event on the next rise.

| Field | Value |
| --- | --- |
| `ts` | Current HKT time |
| `id` | Empty string |
| `context` | Empty object |
| `action` | `CPU/meets threshold` |
| `latency_ms` | 0 |
| `error_message` | null |
| `level` | `warn` |
| `extra.percent` | The usage, rounded to one decimal, as a JSON number |
| `extra.threshold_percent` | The threshold, as a JSON number |

## Stop

When the shutdown receiver is triggered, the task ends. It does not make a final sample.
