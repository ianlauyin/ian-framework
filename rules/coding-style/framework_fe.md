# framework_fe coding style

After a refactor,
1. run `pnpm typecheck` in `framework_fe/`, fix all the errors that they show.
2. run `cargo run --manifest-path lint/Cargo.toml --quiet -- fe` on root, fix all the errors that they show.

## General

- Return early when a check fails. Keep the success path at the end of the function.
