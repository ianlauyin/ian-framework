# Coding style

## Duplication and readability

- Readability is more important than strict DRY.
- If simple logic is in only a small number of locations, keep it as it is.
- Do not move duplicated logic into a shared helper, a generic type, or a new abstraction only to remove the duplication.

## Order in a source file

- Put the public types at the top of the file, after the imports and the constants.
- Put the `impl` block of a public type immediately after that type.
- Put the public methods at the top of the `impl` block.

## Source files

- No file may have more than 400 lines.
- A `pub struct` can have many `pub` methods.
- If a `pub struct` has many `pub` methods, you can make a folder for the struct. Use the struct name in snake case as the folder name. Example: `log_shipper/` for `LogShipper`.
- Keep the tests for the `pub struct`, the `pub fn` or the `pub` method in the same source file, below that item.
- If only one source file uses a helper function, keep the helper in that source file.
- If more than one source file in a module uses a helper function, put the helper in the `mod.rs` file of that module.
- Use `mod.rs` to declare the child modules.
- Use `mod.rs` to declare the items that other modules use.
- Put only declarations and shared helper functions in `mod.rs`.
