# fe-lint

`fe-lint` lints one frontend package and prints one list of errors.

The crate has no rules yet. Add a rule as one file in `fe-lint/src/rule/`. Declare that file in `fe-lint/src/rule.rs`, then call it from `check_files` in `fe-lint/src/lib.rs`.

Run `cargo run --release` in `fe-lint/`. The argument is the package folder. With no argument, the root is the current folder.

A comment that contains `code-style-lint-ignore`, the rule id, and a reason stops that rule on that line and on the next line.
