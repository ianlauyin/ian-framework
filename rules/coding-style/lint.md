# lint coding style

- After a change, run `cargo fmt` and `cargo test` in `lint/`. Both must pass with no warnings.
- Do not write a comment in the code. Put a decision in `catalog/lint.md`. When the text is a reason and not a decision, write a test that prevents that reason.
- For each rule, test a case that passes and a case that fails.
- In a file two or more levels deep, use `crate::` paths. Do not use `super::super::`.
- Do not use `self` in an import.
