# framework_rs coding style

The Rust library is the crate `ian-framework-rs` in `framework_rs/`.

## General

- After a refactor, fix all the warnings.
- Readability is more important than strict DRY. Do not add a helper, a generic type or an abstraction only to remove duplication.
- Do not write a comment in the code. Put a decision in `catalog/framework_rs.md`.
- When the text is a reason and not a decision, write a test that prevents that reason.
- Return early when a check fails. Keep the success path at the end of the function.
- Write tests only for functions that hold important logic. For each of these functions, test each edge case. Do not test a trivial function, a getter or a pass-through.
- Name each test by the behavior that it checks.
- Use JSON for the request and the response. Use a different format only for a route that needs a body that is not JSON: a file upload, a stream or a webhook that has its own format.
- Keep the Rust field name as the JSON name. Add a serde rename only when the contract needs a different name.
- Run `cargo fmt` in `framework_rs/` after you change the code.
- Run `cargo run --quiet --manifest-path lint/Cargo.toml -- framework_rs` from the repo root after you change the code. Fix each error.
- When a cargo feature only adds a step to a function, write one function. Put the step in a `#[cfg(feature = "...")]` block, with an early return when the step gives the result. Do not write a second copy of the function for `#[cfg(not(feature = "..."))]`.

## Configuration

- The library never reads an env variable by a name of its own for an app setting. The project reads its env variables, chooses the names and gives the values to the library. A helper that reads a variable by a name that the caller gives is allowed. That helper is `framework_rs/src/env.rs`.
- The `gcp` module may pass the Google Cloud standard env names `GOOGLE_APPLICATION_CREDENTIALS` and `GOOGLE_CLOUD_PROJECT` to the helper in `framework_rs/src/env.rs`. These names are part of the Application Default Credentials rules, not app settings.

## Modules

- Declare a child module as `pub(crate)` when the parent re-exports the items that a caller uses.
- Re-export those items from the parent module file.

## Files

- In a source file, put the items in this order: `mod` and `pub(crate) mod`, `use` imports, `pub mod`, `pub use` and `pub(crate) use`, a private `static`, constants, public items, private items.
- For a public type, put its inherent `impl` block with the public methods first, then its trait impls (`Debug`, `Display`).
- A `static` is private. Do not write `pub` or `pub(crate)` on a `static`.
- Do not use `super::super::` in a path. Use a `crate::` path for an item that is not in the current module or its parent module.
- To use a module and an item of that module, import only the module. Then use the module name to refer to the item.
- To use the `exception!` macro, import `crate::exception`. This import can stay next to the import of `crate::exception::Exception`, because the macro and the module have the same name.
- Keep a type and its inherent `impl` blocks in one file. Do not write an inherent `impl` block for a type in a different file. A trait impl for a type can be in a different file.
- A `mod.rs` has only `mod` lines and `pub use` re-exports. It has no other item, with one exception: the item that has the same name as the module folder can be in the `mod.rs`. The name of the item is the name of the folder in PascalCase for a type and a trait, and in snake_case for a function and a constant. The private items and the `impl` blocks of that item can also be in the `mod.rs`.
- Put each other type, function, constant and `impl` block in a file that is not a `mod.rs`.
- When the sibling files of a module folder use the main type of that folder, and the type has the same name as the folder, put the type in the `mod.rs` of the folder. The sibling files import it from the parent module. If the type has a different name, put it in its own file and re-export it from the `mod.rs`.

## Errors

- A function that can fail returns `Result<T, Exception>`.
- Build each `Exception` with the `exception!` macro in `framework_rs/src/exception.rs`.
- Put each error code in a `pub const` of type `&str` in the file that owns the failure.
- Write the message as a short sentence. Do not put a secret, a token, a query string or an env value in the message.
- When another error is the cause, pass that error as `source`.

## Tests

- In the tests, do not use a name from a project of the user. This includes repo names, app ids, env variable names, secret names, GCP project ids, service accounts and bucket names. Use generic names: `my-app`, `my-project` and `APP_KEY`.
- Put each test in `framework_rs/src/test/`. Do not put a test in the source file of the tested item.
- The folder `framework_rs/src/test/` has the same shape as the crate. The tests of `framework_rs/src/a/b.rs` are in `framework_rs/src/test/a/b.rs`. The tests of an item in `framework_rs/src/a/mod.rs` are in `framework_rs/src/test/a/mod.rs`. Each test folder has a `mod.rs` that declares its child modules, with the same `#[cfg(feature = "...")]` attributes as the source.
- A test reaches the tested item with a `crate::` path. When a test needs a private item, change the visibility of the item to `pub(crate)`. Do not use `pub` for this.
- Keep all test-only code in a test module. This includes test constructors, test-only methods and test doubles.
- Name each `#[test]` function with the behavior that it checks.

## Safety

- A log record may derive `Debug`. The log records are `LogEvent` in `framework_rs/src/logging/event.rs`, `ClientLogEvent` in `framework_rs/src/logging/client_log/event.rs`, and `LogRequest` in `framework_rs/src/logging/log_request.rs`.
- For a type that holds a secret, write its `Debug` impl by hand. Do not show a secret, a token or a query string in that `Debug` output or in an error message.
- Do not use an index that can be out of range outside tests. This includes startup code.
- When an input is invalid or an operation fails, return an `Exception`. The project decides how to handle it.
