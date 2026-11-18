# Frontend lint

This file is the catalog of the linter in `fe-lint/`.

Each row has the entry path, a short description, and the open behaviour. Update that row in the same change when you make a new decision, the entry moves, or you add or remove a feature. When a code change would change the open behaviour and you did not make that decision, stop and ask. Leave the decision as it is.

| Path | Description | Decision |
| --- | --- | --- |
| `fe-lint/src/lib.rs` | Lint run for one package root | The caller gives the root folder. The library does not read a configuration file. `check_files` runs the rules. There are no rules yet. An error of the walk is a tool failure of `code-style`. |
| `fe-lint/src/main.rs` | The `fe-lint` program | The argument is the root folder. The default root is the current folder. The program prints the diagnostics on stdout. It prints a tool failure on stderr. It exits with code 1 when there is a diagnostic, with code 2 when the file walk did not finish, and with code 0 when there is none. |
| `fe-lint/src/diagnostic.rs` | One error and the ignore comment | A diagnostic has a fix. The tool name is `code-style`. The comment `code-style-lint-ignore` with a rule id stops that rule on the same line and on the next line. |
| `fe-lint/src/files.rs` | The file tree | The walk obeys `.gitignore`, also when the root is not in a git repository. The module name of a path is the nearest folder that has a `module.tsx`. |
| `fe-lint/src/rule.rs` | The place for a rule | The folder has no rules. A rule is one file. Declare the file in `fe-lint/src/rule.rs`, then call it from `check_files` in `fe-lint/src/lib.rs`. |
