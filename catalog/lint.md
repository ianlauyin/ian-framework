# Lint

This file is the catalog of the linter in `lint/`.

- The decision records a behaviour that the code does not show. 
- Update that row in the same change when you make a new decision, the entry moves, or you add or remove a feature. 
- When a code change would change the decision and you did not make that decision, stop and ask. Leave the decision as it is.

| Path | Description | Decision |
| --- | --- | --- |
| `lint/Cargo.toml` | The crate `ian-lint` | |
| `lint/src/config.rs` | The configuration file | `ruleset` is `framework` or `project`. |
| `lint/src/lib.rs` | Lint run for one root folder | A side checks the frontend, Rust, or both. |
| `lint/src/main.rs` | The `ian-lint` program | `fe` checks only the frontend. `rs` checks only Rust. No side argument checks both. The other argument is the root. The root is `.` when it is omitted. The arguments can be in either order. Two roots, or two sides, stops the run. |
| `lint/src/diagnostic.rs` | One error | |
| `lint/src/files.rs` | The file tree and the file kinds | |
| `lint/src/rule/mod.rs` | The two rule sets | |
| `lint/src/rule/ruleset.rs` | The rules in each ruleset | `FRAMEWORK` and `PROJECT` each hold `css`, `ts`, and `rs`. The `framework` ruleset runs `FRAMEWORK`. The `project` ruleset runs `PROJECT`. |
| `lint/src/rule/finder.rs` | The diagnostics for one file | |
| `lint/src/rule/fe/mod.rs` | The frontend rules | |
| `lint/src/rule/fe/css/mod.rs` | The CSS rules | A class selector, an id selector, and descending specificity are not checked. |
| `lint/src/rule/fe/css/parse.rs` | The CSS statements | |
| `lint/src/rule/fe/css/rule_empty_line_before.rs` | An empty line before a rule | The first node of a file is not checked. The first child of a block has no empty line before it. A rule after a single-line comment has no empty line before it. |
| `lint/src/rule/fe/css/selector_nested_pattern.rs` | A nested selector | Each selector in a list is checked on its own. A rule in an at-rule is not checked. |
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
| `lint/src/rule/rs/env_name.rs` | An env value and an env-name literal | A test file can read an env value and can contain an env-name literal. |
| `lint/src/rule/rs/exception_code.rs` | A string literal error code in `exception!` | |
| `lint/src/rule/rs/exception_new.rs` | `Exception::__new` outside `exception.rs` | |
| `lint/src/rule/rs/impl_order.rs` | The inherent impl before trait impls | |
| `lint/src/rule/rs/inherent_impl.rs` | An inherent impl outside the file that defines the type | |
| `lint/src/rule/rs/mod_file.rs` | The items in a `mod.rs` | A `use` can be in `mod.rs`. A test file is not checked. |
| `lint/src/rule/rs/mutex_lock.rs` | `lock().unwrap` on a standard mutex | |
| `lint/src/rule/rs/no_comments.rs` | A comment | |
| `lint/src/rule/rs/place.rs` | The src-relative place of a Rust file | |
| `lint/src/rule/rs/private_static.rs` | A private static | |
| `lint/src/rule/rs/reexport_mod.rs` | A child module re-exported with `pub use` | |
| `lint/src/rule/rs/result_type.rs` | A fallible function returns `Result<T, Exception>` | |
| `lint/src/rule/rs/source_order.rs` | The source order | |
| `lint/src/rule/rs/super_path.rs` | A `super::super` path | |
| `lint/src/rule/rs/test_attr.rs` | A test outside the test folder | `mod test` in `lib.rs` or `main.rs` stays. |
| `lint/src/rule/rs/folder/mod.rs` | The Rust folder rules | |
| `lint/src/rule/rs/folder/test_tree.rs` | The test tree matches the crate | A source file does not need a test file. The test root `mod.rs` follows `lib.rs`. When `lib.rs` is absent, it follows `main.rs`. |
