# Require session

The parts that apply to all of this feature are in `spec/framework_rs/auth/context.md`.

## Require session middleware

- The source file is `framework_rs/src/http/middleware/require_session.rs`.
- `ian_framework::http::middleware::require_session` is an Axum middleware function. It is available with the Cargo feature `auth`.
- The project adds it to a router with `axum::middleware::from_fn_with_state` and one `SessionStore`. The session store is in `spec/framework_rs/auth/session-store.md`.
- `SessionSubject(pub String)` is the type that the middleware puts in the request extensions.

For each request, the middleware does these steps:

1. It reads the token with `SessionStore::token`.
2. It reads the subject with `SessionStore::subject`.
3. When it has the subject, it puts one `SessionSubject` in the request extensions. Then it calls the next handler.

The middleware does not call the next handler in these conditions:

| Condition | Status | Code | Message |
| --- | --- | --- | --- |
| The request has no token. | 401 | `not_signed_in` | `not signed in` |
| The store has no subject for the token. | 401 | `not_signed_in` | `not signed in` |
| The store fails. | The status of the `AuthError` | The code of the `AuthError` | The message of the `AuthError` |

The 401 response body is:

```json
{"code":"not_signed_in","message":"not signed in"}
```

Rules:

- The middleware does not extend the session. Use `slide_session` for that. Put `slide_session` outside `require_session`. `slide_session` does not change a 401 response.
- The middleware gives only the subject. A project loads its own user data with the subject.
- Use the middleware on the routes that need a signed-in user. Do not use it on the login route.
