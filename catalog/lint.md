# Lint

This file is the catalog of the linter in `lint/`.

- The decision records a behaviour that the code does not show. 
- Update that row in the same change when you make a new decision, the entry moves, or you add or remove a feature. 
- When a code change would change the decision and you did not make that decision, stop and ask. Leave the decision as it is.

| Path | Description | Decision |
| --- | --- | --- |
| `lint/Cargo.toml` | The crate `ian-lint` | |
| `lint/src/config.rs` | The configuration file | |
| `lint/src/lib.rs` | Lint run for one root folder | |
| `lint/src/main.rs` | The `ian-lint` program | |
| `lint/src/diagnostic.rs` | One error | |
| `lint/src/files.rs` | The file tree and the file kinds | |
| `lint/src/rule/mod.rs` | The two rule sets | |
| `lint/src/rule/finder.rs` | The diagnostics for one file | |
| `lint/src/rule/fe/mod.rs` | The frontend rules | |
| `lint/src/rule/fe/css/mod.rs` | The CSS rules | |
| `lint/src/rule/fe/folder/mod.rs` | The frontend folder rules | |
| `lint/src/rule/fe/folder/index_file.rs` | Each source folder has an `index.ts` | |
| `lint/src/rule/fe/folder/root_reexport.rs` | The root index re-exports each feature | A nested index can leave out an export for a sibling file. |
| `lint/src/rule/fe/folder/test_path.rs` | The test path matches the source path | A source file does not need a test file. |
| `lint/src/rule/fe/ts/mod.rs` | The TypeScript rules | |
| `lint/src/rule/fe/ts/declaration_order.rs` | The declaration order | A private `let` is in the same group as a constant. |
| `lint/src/rule/fe/ts/no_comments.rs` | A comment | |
| `lint/src/rule/fe/ts/no_default_export.rs` | No default export | |
| `lint/src/rule/fe/ts/no_restricted_imports.rs` | The import path rule | |
| `lint/src/rule/rs/mod.rs` | The Rust rules | |
| `lint/src/rule/rs/no_comments.rs` | A comment | |
| `lint/src/rule/rs/private_static.rs` | A private static | |
| `lint/src/rule/rs/folder/mod.rs` | The Rust folder rules | |
