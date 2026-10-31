# Disk monitor

The parts that apply to all of this feature are in `spec/framework_rs/monitor/context.md`.

The disk monitor checks the storage usage of one path on a Unix host on an interval. When the usage meets a threshold, it makes one `LogEvent` and gives the event to a sink that the caller supplies. The monitor does not know the shipper or the log server store.

## DiskMonitorConfig

`DiskMonitorConfig::new` takes the threshold in percent.

| Field | Start value |
| --- | --- |
| `threshold_percent` | The argument |
| `path` | `/` |
| `interval` | 60 seconds |

- The fields are public.
- `DiskMonitorConfig::new` does not read an environment variable. The app reads its own settings and gives the values to the config.

## DiskMonitor

`DiskMonitor::new` returns no monitor in these conditions:

- `threshold_percent` is not more than 0 and not at most 100. A NaN value is not valid.
- `interval` is zero.

`DiskMonitor::spawn` takes a `ian_framework::shutdown::ShutdownReceiver` and a sink. The sink is `Fn(LogEvent) + Send + 'static`. It returns `tokio::task::JoinHandle<()>`. The caller runs it on a Tokio runtime.

| Caller | Sink |
| --- | --- |
| Project backend on the VM | Calls `LogShipper::record`. The shipper sends the event to the log server. |
| Log server | Writes the event to its SQLite store directly. It does not send the event to itself. |

## Sample

- The monitor reads the file system of `path` with `statvfs`.
- The usage is the same as `df`: used blocks divided by used blocks plus the blocks that a normal user can use.
- The monitor reads the first sample when the task starts and checks it at once. It takes the next samples on each interval.
- When the file system has no blocks, the monitor skips that interval.
- When the first sample cannot be read, the monitor writes one `tracing` warn event and the task ends. This is the case on a host that is not Unix, or when `path` does not exist.
- When a later sample cannot be read, the monitor skips that interval.

## Event

The monitor makes one event when the usage rises to the threshold or above it. It makes no new event while the usage stays at or above the threshold. When the usage falls below the threshold, the monitor can make a new event on the next rise.

| Field | Value |
| --- | --- |
| `ts` | Current HKT time |
| `id` | Empty string |
| `ip` | Empty string |
| `user_agent` | Empty string |
| `action` | `DISK/meets threshold` |
| `latency_ms` | null |
| `level` | `warn` |
| `extra.disk_percent` | The usage, rounded to one decimal, as a JSON number |
| `extra.threshold_percent` | The threshold, as a JSON number |
| `extra.path` | The monitored path |

## Stop

When the shutdown receiver is triggered, the task ends. It does not make a final sample.
