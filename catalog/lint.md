# Lint

This file is the catalog of the linter in `lint/`.

- The decision records a behaviour that the code does not show. 
- Update that row in the same change when you make a new decision, the entry moves, or you add or remove a feature. 
- When a code change would change the decision and you did not make that decision, stop and ask. Leave the decision as it is.

| Path | Description | Decision |
| --- | --- | --- |
| `lint/Cargo.toml` | The crate `ian-lint` | |
| `lint/src/config.rs` | The configuration file and the side | `ruleset` is `framework` or `project`. A side checks the frontend, Rust, or both. |
| `lint/src/check.rs` | The check of one root folder, and the start and end steps that the format also uses | |
| `lint/src/main.rs` | The `ian-lint` program, the report, and the failure | `fe` checks only the frontend. `rs` checks only Rust. No side argument checks both. The other argument is the root. The root is `.` when it is omitted. The arguments can be in either order. Two roots, or two sides, stops the run. One `format` is allowed. A second `format` stops the run. |
| `lint/src/files.rs` | The file tree and the file kinds | |
| `lint/src/format.rs` | The format of one root folder | `format` writes each changed file, then checks the root. |
| `lint/src/ruleset.rs` | The rules in each ruleset | `FRAMEWORK` and `PROJECT` each hold `css`, `ts`, and `rs`. The `framework` ruleset runs `FRAMEWORK`. The `project` ruleset runs `PROJECT`. Each rule has a check. The edit is optional. The formatter runs the edits in the order of the ruleset. Each edit uses the text from the previous edit. |
| `lint/src/rule/mod.rs` | The two rule sets | A `mod.rs` holds only module declarations. The Rust rules that check one file are in `lint/src/rule/rs/file/`. The Rust rules that check one folder are in `lint/src/rule/rs/folder/`. The rule type, the source file, and the other helpers are next to these folders. There is no `support` folder. |
| `lint/src/rule/diagnostic.rs` | One error | |
| `lint/src/rule/edit.rs` | An edit of one file | Overlapping edits in one file are a failure. That file is not written. An edit outside the file is a failure. That file is not written. |
| `lint/src/rule/report.rs` | The diagnostics of one rule in one file | |
| `lint/src/rule/fe/mod.rs` | The frontend rules | |
| `lint/src/rule/fe/run.rs` | The run of the frontend rules on one file or one folder | |
| `lint/src/rule/fe/css/mod.rs` | The CSS rules | A class selector, an id selector, and descending specificity are not checked. |
| `lint/src/rule/fe/css/file.rs` | The CSS source and statements for one file | |
| `lint/src/rule/fe/css/parse.rs` | The CSS statements | |
| `lint/src/rule/fe/css/rule.rs` | The CSS rule type and the CSS rules of each ruleset | |
| `lint/src/rule/fe/css/rule_empty_line_before.rs` | An empty line before a rule | The first node of a file is not checked. The first child of a block has no empty line before it. A rule after a single-line comment has no empty line before it. The formatter puts one empty line in the gap when the rule should have one. The formatter leaves one line break when the rule should have no empty line. The indent of the rule stays. |
| `lint/src/rule/fe/css/selector_nested_pattern.rs` | A nested selector | Each selector in a list is checked on its own. A rule in an at-rule is not checked. |
| `lint/src/rule/fe/ts/mod.rs` | The TypeScript rules | |
| `lint/src/rule/fe/ts/file.rs` | The TypeScript source and syntax tree for one file | |
| `lint/src/rule/fe/ts/rule.rs` | The TypeScript rule type and the TypeScript rules of each ruleset. A rule checks one file or one folder. | |
| `lint/src/rule/fe/ts/declaration_order.rs` | The declaration order | A private `let` is in the same group as a constant. The formatter rewrites the statement list when every statement has a group. A statement with no group leaves the file unchanged. |
| `lint/src/rule/fe/ts/no_comments.rs` | A comment | |
| `lint/src/rule/fe/ts/no_default_export.rs` | No default export | |
| `lint/src/rule/fe/ts/no_restricted_imports.rs` | The import path rule | |
| `lint/src/rule/fe/ts/folder/mod.rs` | The frontend folder rules | |
| `lint/src/rule/fe/ts/folder/index_file.rs` | Each source folder has an `index.ts` | |
| `lint/src/rule/fe/ts/folder/root_reexport.rs` | The root index re-exports each feature | A nested index can leave out an export for a sibling file. |
| `lint/src/rule/fe/ts/folder/test_path.rs` | The test path matches the source path | A source file does not need a test file. |
| `lint/src/rule/rs/mod.rs` | The Rust rules | |
| `lint/src/rule/rs/run.rs` | The run of the Rust rules on one file or one folder | |
| `lint/src/rule/rs/source.rs` | The Rust source and syntax tree for one file | |
| `lint/src/rule/rs/place.rs` | The src-relative place of a Rust file | |
| `lint/src/rule/rs/rule.rs` | The Rust rule type and the Rust rules of each ruleset. A rule checks one file or one folder. | |
| `lint/src/rule/rs/file/mod.rs` | The Rust file rules | |
| `lint/src/rule/rs/file/env_name.rs` | An env value and an env-name literal | A test file can read an env value and can contain an env-name literal. |
| `lint/src/rule/rs/file/exception_code.rs` | A string literal error code in `exception!` | |
| `lint/src/rule/rs/file/exception_new.rs` | `Exception::__new` outside `exception.rs` | |
| `lint/src/rule/rs/file/impl_order.rs` | The inherent impl before trait impls | The formatter puts an inherent impl before the trait impls of that public type. A nested list is inside that edit. |
| `lint/src/rule/rs/file/inherent_impl.rs` | An inherent impl outside the file that defines the type | |
| `lint/src/rule/rs/file/mod_file.rs` | The items in a `mod.rs` | A `use` can be in `mod.rs`. A test file is not checked. |
| `lint/src/rule/rs/file/mutex_lock.rs` | `lock().unwrap` on a standard mutex | |
| `lint/src/rule/rs/file/no_comments.rs` | A comment | |
| `lint/src/rule/rs/file/private_static.rs` | A private static | |
| `lint/src/rule/rs/file/reexport_mod.rs` | A child module re-exported with `pub use` | |
| `lint/src/rule/rs/file/result_type.rs` | A fallible function returns `Result<T, Exception>` | |
| `lint/src/rule/rs/file/source_order.rs` | The source order | The formatter sorts the item list. A nested list is inside that edit. |
| `lint/src/rule/rs/file/super_path.rs` | A `super::super` path | |
| `lint/src/rule/rs/file/test_attr.rs` | A test outside the test folder | `mod test` in `lib.rs` or `main.rs` stays. |
| `lint/src/rule/rs/folder/mod.rs` | The Rust folder rules | |
| `lint/src/rule/rs/folder/test_tree.rs` | The test tree matches the crate | A source file does not need a test file. The test root `mod.rs` follows `lib.rs`. When `lib.rs` is absent, it follows `main.rs`. |
