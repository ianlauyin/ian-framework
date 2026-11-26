# Framework

This repository is the framework.
Do not put business logic in this repository.

Before you read or change a file in `framework_rs/`, read `catalog/framework_rs.md`.
Before you read or change a file in `framework_fe/`, read `catalog/framework_fe.md`.
Before you read or change a file in `lint/`, read `catalog/lint.md`.

## Rules

| File | When to read |
| --- | --- |
| `rules/lint-crate.md` | Before the first edit in `lint/` |
| `rules/debugging.md` | Before you fix a bug or find the cause of an error |
| `rules/git-convention.md` | Before you make a git commit |
| `rules/writing.md` | Before you write or update a markdown file |

## Coding style

Always value performance/efficiency/simplicity over strict correctness/defensive design, let me know if there is any trade off to make.

### framework_fe

- After a refactor:
  1. cd to `framework_fe/`.
  2. Run `pnpm format --write`.
  3. Run `pnpm check`, fix all the errors that it shows.

### framework_rs

- Use JSON for the request and the response. Use a different format only for a route that needs a body that is not JSON.
- Do not put a secret, a token, a query string or an env value in a log message or an error message.
- After a refactor:
  1. cd to `framework_rs/`.
  2. Run `cargo fmt` and `cargo test`, fix all the errors that they show.
  3. Run `cargo run --manifest-path ../lint/Cargo.toml --quiet -- be ..`, fix all the errors that it