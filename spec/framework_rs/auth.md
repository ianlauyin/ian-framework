# Sessions and passwords

This document describes cookie sessions and password hashes in the framework.
This feature is not product business logic.

## Crate

- The package name is `ian_framework`.
- The crate path is `framework_rs/`.
- The module path is `ian_framework::auth`.
- `rust-toolchain.toml` gives the Rust version for this repository.

## Feature `auth`

- The Cargo feature name is `auth`.
- `auth` is not a default feature.
- The `auth` feature uses Tokio for the password hash.
- The `auth` feature does not enable `gcs`.
- The `auth` feature enables the `redis` feature `tokio-rustls-comp`. Thus a `rediss://` URL connects with TLS.
- The TLS connection uses `rustls` with the system root certificates.
- A project does not add its own `redis` dependency for TLS.
- The HTTP boundary error and the cookie header helpers are in `spec/framework_rs/http.md`.

Put this block in the project `Cargo.toml`:

```toml
[dependencies]
ian_framework = { git = "https://github.com/ianlauyin/ian-framework", rev = "<commit>", features = ["auth"] }
```

The crate is not on crates.io. A project adds the crate as a git dependency. The project pins `rev` to one commit.

Use these commands in `framework_rs/`:

```sh
cargo check --features auth
cargo test --features auth
```

The session tests need a Redis server:

- The tests read the Redis URL from `REDIS_URL`.
- When `REDIS_URL` is not set, the tests use `redis://127.0.0.1:6379`.
- When the tests cannot connect, the session tests write a skip note and pass.

## Session store

A session joins one random token to one subject.

- A session ends when it is idle for `ttl_secs`.
- A session also ends `max_age_secs` after `create`, even when it is in use.
- The subject is one string.
- The project owns the subject.
- The framework does not read the user table.

### SessionConfig

`SessionConfig` gives the session settings. It is a plain struct with `pub` fields and `Default`.

| Field | Default | Value |
| --- | --- | --- |
| `prefix` | `session:` | The start of each Redis key. |
| `cookie_name` | `session` | The name of the session cookie. |
| `ttl_secs` | 604800 | The idle session life in seconds. |
| `max_age_secs` | 2592000 | The longest session life in seconds, from `create`. |

### Connect

`SessionStore::connect` takes one Redis URL and one `SessionConfig`. It opens one Redis connection manager. Then it sends `PING`.

`SessionStore::connect` returns an error in these conditions:

- The URL is not a Redis URL.
- Redis does not reply `PONG`.
- `ttl_secs` is zero.
- `max_age_secs` is zero.
- `max_age_secs` is more than 400 days (34560000 seconds). Browsers limit a cookie lifetime to 400 days.
- `ttl_secs` is more than `max_age_secs`.
- `cookie_name` is not an RFC 6265 cookie name. A cookie name has one or more visible ASCII characters. It has no separator character `()<>@,;:\"/[]?={}`.

The connection manager stops early when Redis is down. It uses these settings:

| Setting | Value |
| --- | --- |
| Connection timeout | 2 seconds |
| Response timeout | 5 seconds |
| Retries | 2 |
| Maximum delay between retries | 500 milliseconds |

- `SessionStore` is a handle. A clone of the handle costs little.
- All clones use the same connection manager.
- The error messages do not contain the Redis URL.

### Redis data

- The Redis key of a session is `{prefix}{hash}`.
- `hash` is the SHA-256 of the token, in lowercase hex.
- Redis does not hold the token. Thus a copy of Redis does not give tokens that work.
- The value is `{deadline}:{subject}`.
- `deadline` is the creation time plus `max_age_secs`, in Unix seconds.
- The key expires after `ttl_secs`.

### Methods

| Method | Result |
| --- | --- |
| `create(subject)` | Makes a new token, stores the session, and returns the token. |
| `subject(token)` | Returns the subject. Returns no subject when the session is missing, expired, or past its deadline. |
| `touch(token)` | Sets the expiry to the smaller of `ttl_secs` and the time to the deadline. Returns `false` when the session is missing or past its deadline. |
| `remove(token)` | Removes one session. |
| `remove_subject(subject)` | Removes each session of that subject. |
| `remove_subject_except(subject, keep_token)` | Removes each session of that subject, but not the session of `keep_token`. |
| `ttl_secs()` | Returns the configured `ttl_secs`. |

A token is 32 random bytes from the OS random source, in lowercase hex. A token has 64 characters.

`remove_subject` uses these rules:

- It scans the keys that start with `prefix`.
- It removes each key whose value has the subject.
- A project with many sessions must use a prefix that only the session store uses.

`remove_subject_except` uses these rules:

- It does the same scan as `remove_subject`. It skips the key of `keep_token`.
- `keep_token` continues to work. It does not make a new token.
- A project calls it after a password change. This ends the other sessions of that user and keeps the current session.

## Cookie

`SessionStore::token` reads the session token from the `Cookie` request headers with `ian_framework::http::read_cookie`.

- It takes the value of the first cookie with the name `cookie_name`.
- It returns no token when no cookie has the name.
- It returns no token when the value is not 64 lowercase hex characters. Thus a bad value does not go to Redis.

`SessionStore::cookie` returns one `Set-Cookie` value for a token. The value has these attributes:

- `HttpOnly`
- `Secure`
- `SameSite=Lax`
- `Path=/`
- `Max-Age={ttl_secs}`
- `Expires` at the current UTC time plus `ttl_secs`, in the HTTP date format

`SessionStore::clear_cookie` returns one `Set-Cookie` value that removes the cookie.

- It has an empty value.
- It has `Max-Age=0`.
- It has `Expires=Thu, 01 Jan 1970 00:00:00 GMT`.
- Its other attributes are the same as the attributes of `SessionStore::cookie`.

## Sliding session

`slide_session` is the Axum middleware function. Use it with `axum::middleware::from_fn_with_state` and one `SessionStore`.

The middleware reads the token before the inner handler runs. After the handler returns, the middleware does these steps:

1. When the response status is 401, it returns the response with no change.
2. When the request has no token, it returns the response with no change.
3. When the response has a `Set-Cookie` for `cookie_name`, it returns the response with no change. It uses `ian_framework::http::sets_cookie` for this check. This step keeps the cookie of a login handler or a logout handler.
4. It calls `touch` for the token.
5. When `touch` returns `true`, it adds one `Set-Cookie` with `cookie(token)`. It keeps the other `Set-Cookie` values of the response.

A Redis failure in the middleware does not change the response.

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

## Error

`AuthError` is the error of this module. `AuthError::kind` returns one `AuthErrorKind`.

| Kind | Cause | Status | Code | Message for the client |
| --- | --- | --- | --- | --- |
| `Config` | Bad Redis URL or bad `SessionConfig` | 500 | `auth_config` | `session store is not available` |
| `Store` | Redis connection failure or Redis command failure | 503 | `auth_store` | `session store is not available` |
| `Password` | Password too long, hash failure, or blocking task failure | 500 | `auth_password` | `password hash failed` |
| `Random` | OS random source failure | 500 | `auth_random` | `session token failed` |

- `AuthError` implements `IntoApiError`.
- The message for the client does not contain the internal detail.

## Out of scope

This feature does not include these items:

- A user table or a user lookup.
- Login routes, logout routes, or sign-up routes.
- Username rules, display name rules, or password length rules.
- One session for each user. To get this, a project removes the old sessions with `remove_subject` before `create`.
