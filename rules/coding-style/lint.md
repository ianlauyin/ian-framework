# lint coding style

The linter is the crate `ian-lint` in `lint/`.

## General

- After a change, run these commands in `lint/`. All must pass with no warnings:
  1. `cargo fmt`
  2. `cargo test`
- Readability is more important than strict DRY. Do not add a helper, a generic type, or an abstraction only to remove duplication.
- Do not write a comment in the code. Put a decision in `catalog/lint.md`.
- When the text is a reason and not a decision, write a test that prevents that reason.
- Return early when a check fails. Keep the success path at the end of the function.
- The library does not read a configuration file.
- Do not put a name of a project in the code or in a test. Use a generic name.

## Rules

- Put a frontend rule in one file in `lint/src/rule/fe/`. Put a Rust rule in one file in `lint/src/rule/rs/`. Both folders have no rules until you add one.
- The rule id in kebab-case is the file name in snake_case.
- The comment at the top of the file is the text of the rule. It has a line that starts with `Why a lint rule:`.
- Call a frontend rule from `check` in `lint/src/rule/fe/mod.rs`. Call a Rust rule from `check` in `lint/src/rule/rs/mod.rs`.
- `check_files` in `lint/src/lib.rs` calls the set for the file kind.
- A diagnostic of a rule has a fix.

## Tests

- Put a test of public behavior in `lint/tests/`.
- A test that reads a private item of a rule stays in that rule file.
- Name each test by the behavior that it checks.
- For each rule, test a case that passes and a case that fails.

## Files

- In a source file, put the imports and the constants first. Then put each public item. Put the private types and the private functions after the public items.
- In a file two or more levels deep, use `crate::` paths. Do not use `super::super::`.
- Do not use `self` in an import.
