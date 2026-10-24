# Sessions and passwords

This document describes cookie sessions and password hashes in the framework.
This feature is not product business logic.

## Crate

The package name is `ian_framework`.
The crate path is `framework/`.
The module path is `ian_framework::auth`.
`rust-toolchain.toml` gives the Rust version for this repository.

## Feature `auth`

The Cargo feature name is `auth`.
`auth` is not a default feature.
The `auth` feature enables `http`.
The `auth` feature does not enable `logging`.
The `auth` feature does not enable `gcs`.
The HTTP boundary error is `spec/framework/http.md`.

Put this block in the project `Cargo.toml`:

```toml
[dependencies]
ian_framework = { version = "0.1", features = ["auth"] }
```

Use these commands in `framework/`:

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
The subject is one string, for example a user id.
The project owns the subject.
The framework does not read the user table.

`SessionConfig` gives the session settings.
It is a plain struct with `pub` fields and `Default`.

| field | default | value |
| --- | --- | --- |
| `prefix` | `session:` | start of each Redis key |
| `cookie_name` | `session` | name of the session cookie |
| `ttl_secs` | 604800 | session life in seconds |

`SessionStore::connect` takes one Redis URL and one `SessionConfig`.
It opens one Redis connection manager.
It sends `PING`.
It returns an error when the URL is not a Redis URL.
It returns an error when Redis does not reply `PONG`.
It returns an error when `ttl_secs` is zero.
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

The Redis key of a session is `{prefix}{token}`.
The value is the subject.
The key expires after `ttl_secs`.

| method | result |
| --- | --- |
| `create(subject)` | makes a new token, stores the session, and returns the token |
| `subject(token)` | returns the subject, or no subject when the session is missing or expired |
| `touch(token)` | sets the expiry to `ttl_secs` again; returns `false` when the session is missing |
| `remove(token)` | removes one session |
| `remove_subject(subject)` | removes each session of that subject |
| `ttl_secs()` | returns the configured `ttl_secs` |

A token is 32 random bytes from the OS random source, in lowercase hex.
A token has 64 characters.
`remove_subject` scans the keys that start with `prefix`.
It removes each key whose value is the subject.
A project with many sessions must use a prefix that only the session store uses.

## Cookie

`SessionStore::token` reads the session token from the `Cookie` request header.
It returns the value of the first cookie with the name `cookie_name`.
It returns no token when the header is missing or is not text.

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
3. It calls `touch` for the token.
4. When `touch` returns `true`, it sets `Set-Cookie` to `cookie(token)`.

A Redis failure in the middleware does not change the response.

## Password hash

`hash_password` makes an Argon2id hash with the default parameters and a random salt.
The result is one PHC string.
`verify_password` checks a password against a PHC string.
It returns `false` when the PHC string is not valid.

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
| `Password` | password too long or hash failure | 500 | `auth_password` | `password hash failed` |
| `Random` | OS random source failure | 500 | `auth_random` | `session token failed` |

`AuthError` implements `IntoApiError`.
The message for the client does not contain the internal detail.

## Out of scope

This feature does not include these items:

- A user table or a user lookup.
- Login, logout, or sign-up routes.
- Username, display name, or password length rules.
- One session for each user. A project removes the old sessions with `remove_subject` before `create`.
