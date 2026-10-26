# Sessions and passwords

This document describes cookie sessions and password hashes in the framework.
This feature is not product business logic.

## Sub-features

| Sub-feature | Document |
| --- | --- |
| Session store and cookie | `spec/framework_rs/auth/session-store.md` |
| Sliding session | `spec/framework_rs/auth/sliding-session.md` |
| Password hash | `spec/framework_rs/auth/password.md` |

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
- The HTTP boundary error is in `spec/framework_rs/http/api-error.md`. The cookie header helpers are in `spec/framework_rs/http/cookie.md`.

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
