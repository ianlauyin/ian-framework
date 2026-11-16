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
- The password does not pass `validate_password`.

## Password rules

`validate_password` (in `auth/password.rs`) holds the password rules. `hash_password` and `verify_password` both call it first.

A password has 8 to 256 bytes, and it must contain at least one English letter (`A-Z`, `a-z`) and at least one digit (`0-9`). When a rule fails, `validate_password` returns an exception with the code `VALIDATION_ERROR` and the severity `Warn`. The cause is the client, thus the HTTP status is 400. The messages are:

- `password must not be shorter than 8 bytes`
- `password must not be longer than 256 bytes`
- `password must mix english letters with digits`

`hash_password` returns this exception. `verify_password` turns it into `false`. A user whose old password breaks the rules cannot sign in until the project resets it.

The project owns the other password rules.
