# Password hash

The parts that apply to all of this feature are in `spec/framework_rs/auth/context.md`.

## Password hash

- `hash_password` makes an Argon2id hash with the default parameters and a random salt. The result is one PHC string.
- `verify_password` checks a password against a PHC string.
- The two functions are `async`.
- The two functions run Argon2 with `tokio::task::spawn_blocking`. Thus the hash does not block the async workers.
- The two functions need a Tokio runtime.

`verify_password` returns `false` in these conditions:

- The PHC string is not valid.
- The blocking task fails.
- The password is longer than 256 bytes.

A password has a maximum of 256 bytes. `hash_password` returns an error for a longer password.

The project owns the other password rules.
