# Password hash

The parts that apply to all of this feature are in `spec/framework_rs/auth/context.md`.

## Password hash

- `hash_password` makes an Argon2id hash with the default parameters and a random salt. The result is one PHC string.
- `verify_password` checks a password against a PHC string.
- The two functions are `async`.
- The two functions run Argon2 with `tokio::task::spawn_blocking`. Thus the hash does not block the async workers.
- The number of Argon2 runs at one time is not limited. The project can add a limit if it expects many logins.
- The two functions need a Tokio runtime.

`verify_password` returns `false` in these conditions:

- The PHC string is not valid.
- The blocking task fails.
- The password is longer than 256 bytes.

A password has a maximum of 256 bytes. For a longer password, `hash_password` returns the exception of `validation_error!` with the message `password must not be longer than 256 bytes`. The cause is the client, thus the HTTP status is 400.

The project owns the other password rules.
