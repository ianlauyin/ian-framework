# Lint

This file is the catalog of the linter in `lint/`.

- The decision records a behaviour that the code does not show. 
- Update that row in the same change when you make a new decision, the entry moves, or you add or remove a feature. 
- When a code change would change the decision and you did not make that decision, stop and ask. Leave the decision as it is.

| Path | Description | Decision |
| --- | --- | --- |
| `lint/Cargo.toml` | The crate `ian-lint` | |
| `lint/src/lib.rs` | The exported check, the report, and the failure | `args`, `config`, `file`, `files`, and `rule` are public. |
| `lint/tests/` | The tests | A test is in `check`, `css`, `folder`, `rs`, or `ts`. |
| `lint/src/config.rs` | The configuration file and the side | `ruleset` is `framework` or `project`. A side checks the frontend, Rust, or both. A path is a package folder. The key of the frontend package paths is `fe`. The key of the Rust package paths is `be`. A package path that is not a folder is a failure. |
| `lint/src/check.rs` | The check of one root folder | The linter checks the files in `src` and in `tests` of each package. It does not check the other files in a package. A folder rule starts only at a folder in `src`. A file that the linter cannot read is a failure. A TypeScript or Rust file that does not parse is a failure. The rules do not check that file. |
| `lint/src/main.rs` | The `ian-lint` program | |
| `lint/src/args.rs` | The arguments of the `ian-lint` program | `fe` checks only the frontend. `be` checks only Rust. No side argument checks both. The other argument is the root. The root is `.` when it is omitted. The arguments can be in either order. Two roots, or two sides, stops the run. `-h` or `--help` gives the usage. A different argument that starts with `-` stops the run. |
| `lint/src/files.rs` | The file tree and the file kinds | The walk skips each `node_modules` folder, also when no ignore file names it. |
| `lint/src/ruleset.rs` | The rules in each ruleset | `FRAMEWORK` and `PROJECT` each hold `css`, `ts`, `rs`, and `folder`. The `framework` ruleset runs `FRAMEWORK`. The `project` ruleset runs `PROJECT`. |
| `lint/src/file/mod.rs` | The data for one source file | |
| `lint/src/file/css/mod.rs` | The CSS parser | |
| `lint/src/file/css/source.rs` | The CSS source and statements for one file | |
| `lint/src/file/css/parse.rs` | The CSS statements | lightningcss gives the start of each rule that it knows. The byte scanner gives each span, each declaration, and each comment. The byte scanner also parses each rule that lightningcss drops. A rule from lightningcss that is outside its block, or that overlaps the previous item, is not used. When lightningcss fails, the byte scanner parses the full file. A stray `}` is skipped. A `{` or a `}` in parentheses or brackets does not open or close a block. A stray `)` or `]` does not change the depth. A statement that starts with `@` is an at-rule. A statement that starts with `--` and has a block is a declaration. A statement ends after `;`, before `}`, or at the end of the file. An unclosed block ends at the end of the file. |
| `lint/src/file/css/position.rs` | The byte offset of a lightningcss location | A line number starts at 0. A column starts at 1 and counts UTF-16 units. `\r\n`, `\r`, `\n`, and a form feed each end a line. |
| `lint/src/file/css/scan.rs` | The byte scanner that finds the end of a statement or a block | `/*` in a `url(...)` that has no quote does not start a comment. |
| `lint/src/file/css/sheet.rs` | The items for the rules that lightningcss gives | |
| `lint/src/file/ts/mod.rs` | The TypeScript parser | |
| `lint/src/file/ts/source.rs` | The TypeScript source and syntax tree for one file | |
| `lint/src/file/rs/mod.rs` | The Rust parser | |
| `lint/src/file/rs/item.rs` | The type name and the visibility of a Rust item | |
| `lint/src/file/rs/macro_body.rs` | The code in a macro call | The arguments are read as expressions with commas between them. When that fails, the arguments are read as statements. When the two fail, the code in the macro call is not checked. The rules `config-import`, `env-name`, `exception-code`, `exception-new`, `mutex-lock`, `sqlx-import`, and `super-path` check the code in a macro call. |
| `lint/src/file/rs/place.rs` | The src-relative place of a Rust file | A file in `tests` adjacent to the src folder has a place in the `tests` folder. |
| `lint/src/file/rs/source.rs` | The Rust source and syntax tree for one file | |
| `lint/src/rule/mod.rs` | The rule modules | The `mod.rs` in `lint/src/rule/css/`, `lint/src/rule/ts/`, `lint/src/rule/rs/`, and `lint/src/rule/folder/` holds the module declarations and the rule type. Each other `mod.rs` holds only module declarations. The rules that check one file are in `lint/src/rule/css/`, `lint/src/rule/ts/`, and `lint/src/rule/rs/`. The rules that check one folder are in `lint/src/rule/folder/`. CSS has no folder rule. The CSS file data is in `lint/src/file/css/`. The TypeScript file data is in `lint/src/file/ts/`. The Rust file data is in `lint/src/file/rs/`. There is no `support` folder. |
| `lint/src/rule/diagnostic.rs` | One error | |
| `lint/src/rule/report.rs` | The diagnostics of one rule in one file | |
| `lint/src/rule/folder/mod.rs` | The folder rules and the folder rule type | |
| `lint/src/rule/folder/fe_index_file.rs` | Each source folder has an `index.ts` or an `index.tsx` | |
| `lint/src/rule/folder/fe_root_reexport.rs` | The root index re-exports each feature | A nested index can leave out an export for a sibling file. |
| `lint/src/rule/folder/fe_test_tree.rs` | The test tree matches the source tree | A source file does not need a test file. |
| `lint/src/rule/folder/rs_test_tree.rs` | The test tree matches the crate | A source file does not need a test file. The test folder is `tests` in the src folder or `tests` adjacent to the src folder. The two are checked. In `tests` in the src folder, the test root is `mod.rs`. In `tests` adjacent to the src folder, each file directly in `tests` is a test root, and `tests` does not need a `mod.rs`. A test root does not need a source file. A test root follows `lib.rs`. When `lib.rs` is absent, it follows `main.rs`. In `tests` adjacent to the src folder, a `mod.rs` can follow the source file that has the name of its folder. |
| `lint/src/rule/css/mod.rs` | The CSS rules and the CSS rule type | A class selector, an id selector, and descending specificity are not checked. |
| `lint/src/rule/css/empty_line_before.rs` | An empty line before a rule | The first node of a file is not checked. The first child of a block has no empty line before it. A rule after a single-line comment has no empty line before it. |
| `lint/src/rule/css/selector_nested_pattern.rs` | A nested selector | Each selector in a list is checked on its own. A rule in an at-rule is not checked. |
| `lint/src/rule/run.rs` | The run of the frontend rules on one file | A TypeScript file in `tests` adjacent to the src folder is a test file. The rules `declaration-order` and `test-path` do not check a test file. |
| `lint/src/rule/ts/mod.rs` | The TypeScript rules and the TypeScript file rule type | |
| `lint/src/rule/ts/declaration_order.rs` | The declaration order | A private `let` is in the same group as a constant. A statement that has no group is an error with a different message. |
| `lint/src/rule/ts/import_order.rs` | The import order | The groups are the other packages, then `solid-js`, its subpaths, and `@solidjs/`, then `ian-framework-fe` and its subpaths, then the relative paths, then the imports with no names. A relative path is `.`, `..`, or starts with `./` or `../`. An import and a re-export with `from` are each checked on their own. Only the statements at the top of the file are checked. The order in one group is not checked. An empty line between the groups is not checked. |
| `lint/src/rule/ts/no_arrow_method.rs` | A class property that is an arrow function | Only the `project` ruleset runs it. It checks a class that extends `PageContext` or `Module` by a plain name. A function expression is not an error. A test file is not checked. |
| `lint/src/rule/ts/no_comments.rs` | A comment | |
| `lint/src/rule/ts/no_default_export.rs` | No default export | |
| `lint/src/rule/ts/no_restricted_imports.rs` | The import path rule | A path with one or more segments after `node_modules` is an error. |
| `lint/src/rule/ts/test_path.rs` | A test file is in the source folder | |
| `lint/src/rule/rs/mod.rs` | The Rust rules and the Rust file rule type | |
| `lint/src/rule/rs/run.rs` | The run of the Rust rules on one file | |
| `lint/src/rule/rs/config_import.rs` | The framework env helpers and Secret Manager outside `config.rs` | Only the `project` ruleset runs it. `config.rs` is directly in the src folder. A `use` and a path are each checked. A path that starts with `ian_framework_rs::env` is an error. `secret_manager` or `SecretManager` directly after `gcp` is an error. `std::env` is not checked, because `env-name` checks it. A `use` that ends with `*` is checked only to the `*`. A test file is not checked. |
| `lint/src/rule/rs/env_name.rs` | An env value and an env-name literal | A test file can read an env value and can contain an env-name literal. The framework ruleset allows `GOOGLE_CLOUD_PROJECT` under `gcp`. The project ruleset allows no env name. |
| `lint/src/rule/rs/exception_code.rs` | A string literal error code in `exception!` | A test file is not checked. |
| `lint/src/rule/rs/exception_new.rs` | `Exception::__new` outside `exception.rs` | The framework ruleset allows `exception.rs`. The project ruleset checks every file. |
| `lint/src/rule/rs/impl_order.rs` | The inherent impl before trait impls | |
| `lint/src/rule/rs/import_order.rs` | The import order | The groups are the other crates, then `ian_framework_rs`, then `super`, `self`, and the child modules, then `crate`. A child module is a `mod` in the same item list. A private `use` and a re-export are each checked on their own. A `use` in an inline module is checked. A `use` in a function body is not checked. A `use` that starts with braces or with `*` is not checked. The order in one group is not checked. An empty line between the groups is not checked. |
| `lint/src/rule/rs/inherent_impl.rs` | An inherent impl outside the file that defines the type | |
| `lint/src/rule/rs/mod_file.rs` | The items in a `mod.rs` | A `use` can be in `mod.rs`. A test file is not checked. |
| `lint/src/rule/rs/mutex_lock.rs` | `lock().unwrap`, `lock().unwrap_or`, or `lock().expect` on a standard mutex | `mutex.rs` is not checked. A test file is not checked. |
| `lint/src/rule/rs/no_comments.rs` | A comment | |
| `lint/src/rule/rs/no_nested_import.rs` | A path or braces in the braces of a `use` | A name, a rename, and `*` can be in the braces. Each `use` is checked, also a `use` in a function body. |
| `lint/src/rule/rs/private_static.rs` | A private static | |
| `lint/src/rule/rs/result_type.rs` | A fallible function returns `Result<T, Exception>` | A test file is not checked. A `Result` with no type argument is an error. |
| `lint/src/rule/rs/source_order.rs` | The source order | A private `macro_rules!` is in the same group as a constant. A `macro_rules!` must come before the items that use it. |
| `lint/src/rule/rs/sqlx_import.rs` | `sqlx` outside the `db` folder | Only the `project` ruleset runs it. The `db` folder is `db` directly in the src folder. `db.rs` is not in the `db` folder. A `use` of `sqlx`, a path that starts with `sqlx`, and a `sqlx` path in a `derive` are each an error. A test file is not checked. |
| `lint/src/rule/rs/super_path.rs` | A `super::super` path | |
| `lint/src/rule/rs/test_attr.rs` | A test outside the `tests` folder | `mod tests` in `lib.rs` or `main.rs` stays. A `cfg` that names `test` is a test. A `test` in `not(...)` is not a test. An attribute with the last segment `test`, `rstest`, or `test_case` is a test. |
