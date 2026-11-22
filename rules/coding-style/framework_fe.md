# framework_fe coding style

After a refactor,
1. cd to `framework_fe/`.
1. run `pnpm format`
2. run `pnpm typecheck`, fix all the errors that they show.
3. run `cargo run --manifest-path lint/Cargo.toml --quiet -- fe ..`, fix all the errors that they show.

## General

- Return early when a check fails. Keep the success path at the end of the function.
