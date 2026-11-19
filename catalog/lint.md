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
| `lint/src/rule/fe/ts/mod.rs` | The TypeScript rules | |
| `lint/src/rule/fe/ts/no_restricted_imports.rs` | The import path rule | |
| `lint/src/rule/rs/mod.rs` | The Rust rules | |
| `lint/src/rule/rs/private_static.rs` | A private static | |
| `lint/src/rule/rs/folder/mod.rs` | The Rust folder rules | |
