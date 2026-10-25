# Sessions and passwords

This document describes cookie sessions and password hashes in the framework.
This feature is not product business logic.

## Crate

The package name is `ian_framework`.
The crate path is `framework_rs/`.
The module path is `ian_framework::auth`.
`rust-toolchain.toml` gives the Rust version for this repository.

## Feature `auth`

The Cargo feature name is `auth`.
`auth` is not a default feature.
The `auth` feature enables `http`.
The `auth` feature uses Tokio for the password hash.
The `auth` feature does not enable `logging`.
The `auth` feature does not enable `gcs`.
The HTTP boundary error and the cookie header helpers are `spec/framework_rs/http.md`.

Put this block in the project `Cargo.toml`:

```toml
[dependencies]
ian_framework = { version = "0.1", features = ["auth"] }
```

Use these commands in `framework_rs/`:

```sh
cargo check --features auth
cargo test --features auth
```

The session tests need a Redis server.
The tests read the Redis URL from `REDIS_URL`.
When `REDIS_URL` is not set, the tests use `redis://127.0.0.1:6379`.
When the tests cannot connect, the session tests write a skip note and pass.

## Session store

A session joins one random token to one subject.
A session ends when it is idle for `ttl_secs`.
It also ends `max_age_secs` after `create`, even when it is in use.
The subject is one string, for example a user id.
The project owns the subject.
The framework does not read the user table.

`SessionConfig` gives the session settings.
It is a plain struct with `pub` fields and `Default`.

| field | default | value |
| --- | --- | --- |
| `prefix` | `session:` | start of each Redis key |
| `cookie_name` | `session` | name of the session cookie |
| `ttl_secs` | 604800 | idle session life in seconds |
| `max_age_secs` | 2592000 | longest session life in seconds, from `create` |

`SessionStore::connect` takes one Redis URL and one `SessionConfig`.
It opens one Redis connection manager.
It sends `PING`.
It returns an error when the URL is not a Redis URL.
It returns an error when Redis does not reply `PONG`.
It returns an error when `ttl_secs` is zero.
It returns an error when `max_age_secs` is zero.
It returns an error when `max_age_secs` is more than 400 days, 34560000 seconds. Browsers cap a cookie lifetime at 400 days.
It returns an error when `ttl_secs` is more than `max_age_secs`.
It returns an error when `cookie_name` is not an RFC 6265 cookie name.
A cookie name has one or more visible ASCII characters and no separator character `()<>@,;:\"/[]?={}`.
The connection manager stops early when Redis is down:

| setting | value |
| --- | --- |
| connection timeout | 2 seconds |
| response timeout | 5 seconds |
| retries | 2 |
| max delay between retries | 500 milliseconds |

`SessionStore` is a handle that is cheap to clone.
All clones use the same connection manager.
The error messages do not contain the Redis URL.

The Redis key of a session is `{prefix}{hash}`.
`hash` is the SHA-256 of the token, in lowercase hex.
Redis does not hold the token, so a copy of Redis does not give working tokens.
The value is `{deadline}:{subject}`.
`deadline` is the creation time plus `max_age_secs`, in Unix seconds.
The key expires after `ttl_secs`.

| method | result |
| --- | --- |
| `create(subject)` | makes a new token, stores the session, and returns the token |
| `subject(token)` | returns the subject, or no subject when the session is missing, expired, or past its deadline |
| `touch(token)` | sets the expiry to the smaller of `ttl_secs` and the time to the deadline; returns `false` when the session is missing or past its deadline |
| `remove(token)` | removes one session |
| `remove_subject(subject)` | removes each session of that subject |
| `ttl_secs()` | returns the configured `ttl_secs` |

A token is 32 random bytes from the OS random source, in lowercase hex.
A token has 64 characters.
`remove_subject` scans the keys that start with `prefix`.
It removes each key whose value has the subject.
A project with many sessions must use a prefix that only the session store uses.

## Cookie

`SessionStore::token` reads the session token from the `Cookie` request headers with `ian_framework::http::read_cookie`.
It takes the value of the first cookie with the name `cookie_name`.
It returns no token when no cookie has the name.
It returns no token when the value is not 64 lowercase hex characters, so a bad value does not reach Redis.

`SessionStore::cookie` returns one `Set-Cookie` value for a token.
The value has these attributes:

- `HttpOnly`
- `Secure`
- `SameSite=Lax`
- `Path=/`
- `Max-Age={ttl_secs}`
- `Expires` at the current UTC time plus `ttl_secs`, in the HTTP date format

`SessionStore::clear_cookie` returns one `Set-Cookie` value that removes the cookie.
It has an empty value, `Max-Age=0`, and `Expires=Thu, 01 Jan 1970 00:00:00 GMT`.
It has the same other attributes.

## Sliding session

`slide_session` is the Axum middleware function.
Use it with `axum::middleware::from_fn_with_state` and one `SessionStore`.

```rust
let router = Router::new().layer(middleware::from_fn_with_state(
    sessions.clone(),
    ian_framework::auth::slide_session,
));
```

The middleware reads the token before the inner handler runs.
After the handler returns, the middleware does these steps:

1. When the response status is 401, it returns the response with no change.
2. When the request has no token, it returns the response with no change.
3. When the response has a `Set-Cookie` for `cookie_name`, it returns the response with no change. It checks this with `ian_framework::http::sets_cookie`.
   This keeps the cookie of a login or a logout handler.
4. It calls `touch` for the token.
5. When `touch` returns `true`, it adds one `Set-Cookie` with `cookie(token)`.
   It keeps the other `Set-Cookie` values of the response.

A Redis failure in the middleware does not change the response.

## Password hash

`hash_password` makes an Argon2id hash with the default parameters and a random salt.
The result is one PHC string.
`verify_password` checks a password against a PHC string.
Both functions are `async`.
They run Argon2 with `tokio::task::spawn_blocking`, so the hash does not block the async workers.
They need a Tokio runtime.
It returns `false` when the PHC string is not valid.
It returns `false` when the blocking task fails.

A password has at most 256 bytes.
`hash_password` returns an error for a longer password.
`verify_password` returns `false` for a longer password.
The project owns the other password rules, for example the minimum length.

## Error

`AuthError` is the error of this module.
`AuthError::kind` returns one `AuthErrorKind`.

| kind | cause | status | code | message for the client |
| --- | --- | --- | --- | --- |
| `Config` | bad Redis URL or bad `SessionConfig` | 500 | `auth_config` | `session store is not available` |
| `Store` | Redis connection or command failure | 503 | `auth_store` | `session store is not available` |
| `Password` | password too long, hash failure, or blocking task failure | 500 | `auth_password` | `password hash failed` |
| `Random` | OS random source failure | 500 | `auth_random` | `session token failed` |

`AuthError` implements `IntoApiError`.
The message for the client does not contain the internal detail.

## Out of scope

This feature does not include these items:

- A user table or a user lookup.
- Login, logout, or sign-up routes.
- Username, display name, or password length rules.
- One session for each user. A project removes the old sessions with `remove_subject` before `create`.
