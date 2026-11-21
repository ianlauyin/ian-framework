# lint coding style

- After a change, run `cargo fmt` and `cargo test` in `lint/`. Both must pass with no warnings.
- Do not write a comment in the code. Put a decision in `catalog/lint.md`. When the text is a reason and not a decision, write a test that prevents that reason.
- Do not put a name of a project in the code or in a test. Use a generic name.
- The comment at the top of the file is the text of the rule. It has a line that starts with `Why a lint rule:`.
- Do not put a test in the source file of the tested item.
- A test reaches the tested item with a `crate::` path. When a test needs a private item, change the visibility of the item to `pub(crate)`. Do not use `pub` for this.
- For each rule, test a case that passes and a case that fails.
- In a file two or more levels deep, use `crate::` paths. Do not use `super::super::`.
- Do not use `self` in an import.
