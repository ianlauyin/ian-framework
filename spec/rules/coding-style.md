# Coding style

fix all warning after refactor

## Duplication and readability

- Readability is more important than strict DRY. Do not add a helper, a generic type or an abstraction only to remove duplication.
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

- If a `pub struct` has many `pub` methods, you can make a folder for the struct. Use the struct name in snake case as the folder name. Example: `log_shipper/` for `LogShipper`.
- In a struct folder, put each `pub` method in its own file. Use the method name as the file name. Example: `log_shipper/record.rs` for `LogShipper::record`. If there are many method files, group them into folders by purpose. Example: `gcs_client/read/`, `gcs_client/upload/`, `gcs_client/modify/`.
- Keep a helper, a constant or a type in the file that uses it. If more than one file in a folder uses it, keep it in the file that owns it and make it `pub(super)`. Example: `CHUNK_SIZE` and `UploadStep` in `upload/resumable.rs`. If no file owns it, put it in `mod.rs`.
- Put the tests in one `#[cfg(test)] mod tests` at the end of the source file that holds the tested item. Start it with `use super::*;`.
- In a struct folder, `mod.rs` holds the struct, its private state types, its trait impls, its small accessors, the shared items and the tests for these items.
- In other folders, `mod.rs` holds only the module declarations, the re-exports and the shared items.

## Visibility and public API

- Declare the child modules as private `mod`. Re-export the public types from the `mod.rs` of the feature module with `pub use`. Example: `ian_framework::gcs::GcsClient`.
- Use `pub(super)` for an item that only the parent module uses, `pub(crate)` for an item that other modules use, and `pub` only for the public API.
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

- Write a comment only when the code cannot show the reason.
- Do not write doc comments.
