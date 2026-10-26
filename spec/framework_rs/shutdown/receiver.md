# Shutdown receiver

The parts that apply to all of this feature are in `spec/framework_rs/shutdown/context.md`.

## Public items

`ian_framework::shutdown` exports only `ShutdownReceiver`.

`ShutdownReceiver` implements `Clone` and `Debug`. It does not implement `Default`.

| Method | Signature | Effect |
| --- | --- | --- |
| `ShutdownReceiver::new` | `fn new() -> Self` | Makes one receiver and starts the signal listener. |
| `wait` | `fn wait(&self) -> impl Future<Output = ()> + Send + 'static` | Gives a future that completes when the receiver is triggered. |
| `is_shutdown` | `fn is_shutdown(&self) -> bool` | Returns `true` after the receiver is triggered. |
| `trigger` | `fn trigger(&self)` | Triggers the receiver. |

## Behaviour

### Shared state

- All clones of one receiver share one state.
- A trigger on one clone triggers all clones.
- A clone that the caller makes after the trigger is also triggered.
- The state does not go back to "not triggered".

### Signal listener

- `ShutdownReceiver::new` starts one Tokio task for the signal listener. Thus the caller calls `ShutdownReceiver::new` on a Tokio runtime.
- The listener waits for Ctrl+C (`SIGINT`) and for `SIGTERM`.
- When one of these signals arrives, the listener writes one `tracing` info event with the message `shutdown signal received`. Then it triggers the receiver.
- The listener triggers the receiver one time only. Then the task ends.
- If the listener cannot install a signal handler, it does not wait for that signal.
- If the listener cannot install the two signal handlers, the task ends. The task does not trigger the receiver.

### wait

- The future completes when the receiver is triggered.
- When the receiver is already triggered, the future completes immediately.
- A future that the caller makes before the trigger also completes after the trigger.
- The future owns its own state. It does not borrow the receiver.
- A project can give the future to `axum::serve(...).with_graceful_shutdown`.
- A project can use the future in `tokio::select!` or in `tokio::spawn`.

### trigger

- `trigger` sets the shared state to "triggered".
- A second call does not change the state.
- A project can call `trigger` to start the shutdown without a signal.
