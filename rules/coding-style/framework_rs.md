# framework_rs coding style

After a refactor,
1. cd to `framework_rs/`.
2. Run `cargo fmt`.
3. run `cargo run --manifest-path lint/Cargo.toml --quiet -- be ..`, fix all the errors that they show.
4. 
- Use JSON for the request and the response. Use a different format only for a route that needs a body that is not JSON.
- Do not put a secret, a token, a query string or an env value in the message.
