# Coding style

fix all warning after refactor

## Duplication and readability

- Readability is more important than strict DRY. Do not add a helper, a generic type or an abstraction only to remove duplication.
- Keep a test helper in the test module that uses it. Do not share test helpers between test modules.

## Comments

- Write a comment only when the code cannot show the reason.
- In Rust, do not write doc comments.

## Backend (Rust + Axum)

- Use JSON for the request and the response unless the route needs special handling.

### Order in a source file

- Put the public types at the top of the file, after the imports and the constants.
- Put the `impl` block of a public type immediately after that type.
- Put the public methods at the top of the `impl` block.
- Put the trait impls (`Debug`, `Display`) after the inherent `impl` block.

### Imports

- Group the imports in this order, with a blank line between the groups: `std`, external crates, `crate::` paths to other modules, then the current module.
- In a file two or more levels deep, use `crate::` paths. Do not use `super::super::`.

### Source files

- If a `pub struct` has many `pub` methods, you can make a folder for the struct. Use the struct name in snake case as the folder name. Example: `log_shipper/` for `LogShipper`.
- In a struct folder, put each `pub` method in its own file. Use the method name as the file name. If there are many method files, group them into folders by purpose.
- A method file has one `impl Struct { … }` block. Put the private helpers of the method below that block.
- Keep a helper, a constant or a type in the file that uses it. If more than one file in a folder uses it, keep it in the file that owns it and make it `pub(super)`. If no file owns it, put it in `mod.rs`.
- Put the tests in one `#[cfg(test)] mod tests` at the end of the source file that holds the tested item. Start it with `use super::*;`.
- In a struct folder, `mod.rs` holds the struct, its private state types, its trait impls, its small accessors, the shared items and the tests for these items.
- In other folders, `mod.rs` holds only the module declarations, the re-exports and the shared items.

### Visibility and public API

- Declare the child modules as private `mod`. Re-export the items that other modules use from `mod.rs` with `pub use`.
- Use `pub(super)` for an item that only the parent module uses, `pub(crate)` for an item that other modules use, and `pub` only for the public API.

### Types

- A shared client is a handle that is cheap to clone: `#[derive(Clone)] pub struct X { inner: Arc<Inner> }`.
- If a type holds a secret or a token, write its `Debug` impl by hand. Show only the safe fields and end with `finish_non_exhaustive()`.
- Options and config types are plain structs with `pub` fields and `Default`. Do not use builders.

### Errors

- Each module that can fail has one error type with a `Kind` enum. The error type has `pub(crate)` constructors, such as `new`, `with_status` and `with_source`.
- Write error messages in lowercase, with no period at the end.
- Do not put a secret, a token or a query string in an error message.
- A domain error implements `IntoApiError`. The message for the client is generic and hides the internal detail.
- Do not use `unwrap`, `expect` or `panic!` outside tests.
- Recover a poisoned mutex with `lock().unwrap_or_else(|err| err.into_inner())`.

## Frontend (SolidJS + TypeScript)

### Project Structure

- Put UI primitives and controls that have no domain logic in `src/component/**`.
- A module is one feature. It has its own state, its own actions and its own entry component.
- Put each module in `src/module/{module}`. Example: `src/module/login`.
- An area folder is optional. You can put the modules of the same business area in one folder: `src/module/{area}/{module}`. Example: `src/module/wallet/deposit`.

### Contexts

- The global context is in `src/context.tsx`.
- Each module has one context in `context.ts` in the module folder. All the components of the module use it.
- Put the actions that are not UI logic in the context of the module.

### Tests

- Put all tests in a `test/` folder next to `src/`. Do not put tests in `src/`.
- In `test/`, use the same folder paths as in `src/`.

### Components

- Give each page an id selector.
- If only one component uses a sub-component, put the sub-component in the folder of that component.
- Put constants before the component. For example: `const MIN_TRANSFER_AMOUNT = 0.01;`.
- If the component has no styles, use the file `Xxx.tsx`.
- If the component has styles, use the files `Xxx/index.tsx` and `Xxx/index.css`.
- Put the images for the component in an `asset/` folder next to the component.

### Styles

- Each `index.css` file has only one root selector.
- Nest each child selector inside the block of its parent.
- To check nested selectors, run `pnpm lint:css`.

### Class Names

- Give the outermost tag of each module component and sub-component a class name. Use this format: `{module}-{component}-{sub-component}...`.
- Give the outermost tag of each component in `src/component` a class name with the `g-` prefix.
- Also use the `g-` prefix for modifier classes on that outermost tag.
- If you override the internal parts of a different component, put the `g-` class of that component in the selector.
