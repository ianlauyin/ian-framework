# lint coding style

The linter is the crate `ian-lint` in `lint/`.

## General

- After a change, run these commands in `lint/`. All must pass with no warnings:
  1. `cargo fmt`
  2. `cargo test`
- Readability is more important than strict DRY. Do not add a helper, a generic type, or an abstraction only to remove duplication.
- Do not write a comment in the code. Put a decision in `catalog/lint.md`.
- When the text is a reason and not a decision, write a test that prevents that reason.
- Return early when a check fails. Keep the success path at the end of the function.
- The library reads `ian-linter.config` in the root folder.
- Do not put a name of a project in the code or in a test. Use a generic name.

## Rules

- Put a TypeScript rule in one file in `lint/src/rule/fe/ts/`. Put a CSS rule in one file in `lint/src/rule/fe/css/`. Put a frontend folder rule in one file in `lint/src/rule/fe/folder/`. Put a Rust rule in one file in `lint/src/rule/rs/`. Put a Rust folder rule in one file in `lint/src/rule/rs/folder/`. These folders have no rules until you add one.
- The rule id in kebab-case is the file name in snake_case.
- The comment at the top of the file is the text of the rule. It has a line that starts with `Why a lint rule:`.
- Call a TypeScript rule from `check` in `lint/src/rule/fe/ts/mod.rs`. Call a CSS rule from `check` in `lint/src/rule/fe/css/mod.rs`. Call a frontend folder rule from `check` in `lint/src/rule/fe/folder/mod.rs`.
- `check` in `lint/src/rule/fe/mod.rs` calls the `css` module for a `.css` file and the `ts` module for any other file.
- `check_folder` in `lint/src/rule/fe/mod.rs` calls the `folder` module.
- Call a Rust file rule from `check` in `lint/src/rule/rs/mod.rs`. Call a Rust folder rule from `check` in `lint/src/rule/rs/folder/mod.rs`.
- `check_folder` in `lint/src/rule/rs/mod.rs` calls the `folder` module.
- `check_files` in `lint/src/lib.rs` calls the frontend rules for a frontend source file or a style file, and the Rust rules for a Rust source file.
- `check_folders` in `lint/src/lib.rs` calls the frontend folder rules for a folder under a frontend src folder and the Rust folder rules for a folder under a backend src folder.
- A diagnostic of a rule has a fix.

## Tests

- Put each test in `lint/src/test/`. Do not put a test in the source file of the tested item.
- The folder `lint/src/test/` has the same shape as the crate. The tests of `lint/src/a/b.rs` are in `lint/src/test/a/b.rs`. The tests of an item in `lint/src/a/mod.rs` are in `lint/src/test/a/mod.rs`. Each test folder has a `mod.rs` that declares its child modules.
- A test reaches the tested item with a `crate::` path. When a test needs a private item, change the visibility of the item to `pub(crate)`. Do not use `pub` for this.
- Name each test by the behavior that it checks.
- For each rule, test a case that passes and a case that fails.

## Files

- In a source file, put the imports and the constants first. Then put each public item. Put the private types and the private functions after the public items.
- In a file two or more levels deep, use `crate::` paths. Do not use `super::super::`.
- Do not use `self` in an import.
