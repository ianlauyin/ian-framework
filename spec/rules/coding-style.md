# Coding style

## Duplication and readability

- Readability is more important than strict DRY.
- If simple logic is in only a small number of locations, keep it as it is.
- Do not move duplicated logic into a shared helper, a generic type, or a new abstraction only to remove the duplication.
- Keep a test helper in the test module that uses it. Do not share test helpers between test modules.

## Order in a source file

- Put the public types at the top of the file, after the imports and the constants.
- Put the `impl` block of a public type immediately after that type.
- Put the public methods at the top of the `impl` block.
- Put the trait impls (`Debug`, `Display`) after the inherent `impl` block.

## Imports

- Group the imports in this order, with a blank line between the groups: `std`, external crates, `crate::` paths to other feature modules, then the current module.
- In a file two or more levels deep, use `crate::` paths. Do not use `super::super::`.

## Source files

- A `pub struct` can have many `pub` methods.
- If a `pub struct` has many `pub` methods, you can make a folder for the struct. Use the struct name in snake case as the folder name. Example: `log_shipper/` for `LogShipper`.
- In a struct folder, put each `pub` method in its own file. Use the method name as the file name. Example: `log_shipper/record.rs` for `LogShipper::record`.
- If the struct has many methods, group the method files into folders by purpose. Example: `gcs_client/read/`, `gcs_client/upload/`, `gcs_client/modify/`.
- A method file has one `impl Struct { … }` block. Put the private helpers of the method below that block.
- If only the files in one folder use a constant or a type, keep it in the file that owns it and make it `pub(super)`. Example: `CHUNK_SIZE` and `UploadStep` in `upload/resumable.rs`.
- Put the tests in one `#[cfg(test)] mod tests` at the end of the source file that holds the tested item. Start it with `use super::*;`.
- If only one source file uses a helper function, keep the helper in that source file.
- If more than one source file in a module uses a helper function, put the helper in the `mod.rs` file of that module.
- Use `mod.rs` to declare the child modules.
- Use `mod.rs` to declare the items that other modules use.
- In a struct folder, `mod.rs` holds the struct, its private state types, its trait impls, its small accessors, the shared constants and helper functions, and the tests for these items.
- In other folders, put only declarations, re-exports, shared constants and shared helper functions in `mod.rs`.

## Visibility and public API

- Declare the child modules as private `mod`. Re-export the public types from the `mod.rs` of the feature module with `pub use`. Example: `ian_framework::gcs::GcsClient`.
- Use `pub(super)` for an item that only the parent module uses.
- Use `pub(crate)` for an item that other modules use.
- Do not use `pub` for an internal item.
- Gate each feature module in `lib.rs` with `#[cfg(feature = "…")]`.
- Gate code that uses another feature with that feature. Example: `impl IntoApiError for GcsError` under `#[cfg(feature = "http")]`.

## Types

- A shared client is a handle that is cheap to clone: `#[derive(Clone)] pub struct X { inner: Arc<Inner> }`.
- If a type holds a secret or a token, write its `Debug` impl by hand. Show only the safe fields and end with `finish_non_exhaustive()`.
- Options and config types are plain structs with `pub` fields and `Default`. Do not use builders. Examples: `PutOptions`, `ListQuery`, `ShipperConfig`.

## Errors

- Each feature module has one error type with a `Kind` enum. The error type has `pub(crate)` constructors: `new`, `with_status` and `with_source`.
- Write error messages in lowercase, with no period at the end.
- Do not put a secret, a token or a query string in an error message.
- A domain error implements `IntoApiError`. The message for the client is generic and hides the internal detail.
- Do not use `unwrap`, `expect` or `panic!` outside tests.
- Recover a poisoned mutex with `lock().unwrap_or_else(|err| err.into_inner())`.

## Comments

- Do not write doc comments.
- Write a line comment only when the code cannot show the reason.
