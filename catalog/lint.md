# Lint

This file is the catalog of the linter in `lint/`.

Each row has the entry path, a short description, and the open behaviour. Update that row in the same change when you make a new decision, the entry moves, or you add or remove a feature. When a code change would change the open behaviour and you did not make that decision, stop and ask. Leave the decision as it is.

| Path | Description | Decision |
| --- | --- | --- |
| `lint/Cargo.toml` | The crate `ian-lint` | The package name is `ian-lint`. The program name is `ian-lint`. |
| `lint/src/lib.rs` | Lint run for one root folder | The caller gives the root folder. The library does not read a configuration file. `check_files` sends a `.ts`, `.tsx`, or `.css` file under `src` to the frontend rules, and a `.rs` file under `src` to the Rust rules. There are no rules yet. An error of the walk is a tool failure of `ian-lint`. |
| `lint/src/main.rs` | The `ian-lint` program | The argument is the root folder. The default root is the current folder. The program prints the diagnostics on stdout. It prints a tool failure on stderr. It exits with code 1 when there is a diagnostic, with code 2 when the file walk did not finish, and with code 0 when there is none. |
| `lint/src/diagnostic.rs` | One error and the ignore comment | A diagnostic has a fix. The tool name is `code-style` for a frontend rule and `rs-style` for a Rust rule. The comment `code-style-lint-ignore` with a rule id stops that frontend rule on the same line and on the next line. The comment `rs-lint-ignore` with a rule id stops that Rust rule on the same line and on the next line. |
| `lint/src/files.rs` | The file tree and the file kinds | The walk obeys `.gitignore`, also when the root is not in a git repository. The module name of a path is the nearest folder that has a `module.tsx`. A `.d.ts` file is not a frontend source file. |
| `lint/src/rule.rs` | The two rule sets | A frontend rule is a file in `lint/src/rule/fe/`. A Rust rule is a file in `lint/src/rule/rs/`. |
| `lint/src/rule/fe/mod.rs` | The frontend rules | `check` is the call from `check_files` for a frontend file. The folder has no rules. |
| `lint/src/rule/rs/mod.rs` | The Rust rules | `check` is the call from `check_files` for a Rust file. The folder has no rules. |
