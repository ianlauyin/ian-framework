# framework_rs coding style

The Rust library is the crate `ian-framework-rs` in `framework_rs/`.

## General

- After a refactor, fix all the warnings.
- Readability is more important than strict DRY. Do not add a helper, a generic type or an abstraction only to remove duplication.
- Put a decision in `catalog/framework_rs.md`.
- When the text is a reason and not a decision, write a test that prevents that reason.
- Return early when a check fails. Keep the success path at the end of the function.
- Write tests only for functions that hold important logic. For each of these functions, test each edge case. Do not test a trivial function, a getter or a pass-through.
- Name each test by the behavior that it checks.
- Use JSON for the request and the response. Use a different format only for a route that needs a body that is not JSON: a file upload, a stream or a webhook that has its own format.
- Keep the Rust field name as the JSON name. Add a serde rename only when the contract needs a different name.
- Run `cargo fmt` in `framework_rs/` after you change the code.
- Run `cargo run --quiet --manifest-path lint/Cargo.toml -- rs` from the repo root after you change the code. Fix each error.
- When a cargo feature only adds a step to a function, write one function. Put the step in a `#[cfg(feature = "...")]` block, with an early return when the step gives the result. Do not write a second copy of the function for `#[cfg(not(feature = "..."))]`.

## Files

- To use a module and an item of that module, import only the module. Then use the module name to refer to the item.
- When a macro has the same name as its module, import the module to use the macro. That import can stay next to an import of an item from that module.
- When the sibling files of a module folder use the main type of that folder, and the type has the same name as the folder, put the type in the `mod.rs` of the folder. The sibling files import it from the parent module. If the type has a different name, put it in its own file and re-export it from the `mod.rs`.

## Errors

- Write the message as a short sentence. Do not put a secret, a token, a query string or an env value in the message.
- When another error is the cause, pass that error as `source`.

## Tests

- In the tests, do not use a name from a project of the user. This includes repo names, app ids, env variable names, secret names, GCP project ids, service accounts and bucket names. Use generic names: `my-app`, `my-project` and `APP_KEY`.
- A test reaches the tested item with a `crate::` path. When a test needs a private item, change the visibility of the item to `pub(crate)`. Do not use `pub` for this.
- Keep all test-only code in a test module. This includes test constructors, test-only methods and test doubles.
- Name each `#[test]` function with the behavior that it checks.

## Safety

- A log record may derive `Debug`. The log records are `LogEvent` in `framework_rs/src/logging/event.rs`, `ClientLogEvent` in `framework_rs/src/logging/client_log/event.rs`, and `LogRequest` in `framework_rs/src/logging/log_request.rs`.
- For a type that holds a secret, write its `Debug` impl by hand. Do not show a secret, a token or a query string in that `Debug` output or in an error message.
- Do not use an index that can be out of range outside tests. This includes startup code.
- When an input is invalid or an operation fails, return an `Exception`. The project decides how to handle it.
- A poisoned mutex in `framework_rs/src/mutex.rs` calls `clear_poison` and the caller continues with the guard.
- `trace_event` in `framework_rs/src/logging/logger/trace_event.rs` writes an empty context when the context does not encode.
- `spa` in `framework_rs/src/http/router/spa.rs` returns the HTTP response. A missing dist and an unreadable file are logged with `NOT_FOUND`. The response is an internal error.
- `CpuMonitor::new` in `framework_rs/src/monitor/cpu_monitor.rs` and `DiskMonitor::new` in `framework_rs/src/monitor/disk_monitor.rs` return `None` for a threshold of 0, less than 0, more than 100, or not a number, and for a zero interval. They write a warning. They do not return an `Exception`.
- The first failed read in a monitor task writes a log record and stops the task. A later failed CPU read writes one error and the loop continues. A later failed disk read skips that sample and writes no log record.
