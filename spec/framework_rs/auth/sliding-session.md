# Sliding session

The parts that apply to all of this feature are in `spec/framework_rs/auth/context.md`.

## Sliding session

`ian_framework::http::middleware::slide_session` is the Axum middleware function. The source file is `framework_rs/src/http/middleware/slide_session.rs`. It is available with the Cargo feature `auth`. Use it with `axum::middleware::from_fn_with_state` and one `SessionStore`. The session store is in `spec/framework_rs/auth/session-store.md`.

The middleware does these steps before the inner handler runs:

1. It reads the token with `SessionStore::token`.
2. When the request has a token, it calls `load` for the token. This is the only Redis call of the request for the session.
3. When `load` gives a `Session`, it inserts the `Session` in the request extensions.
4. When `load` fails, it logs the error with `Logger::error` and the action `session_load`. It inserts a crate-only marker in the request extensions. It then runs the inner handler as for a request with no session.

After the handler returns, the middleware does these steps:

1. When it has no `Session`, it returns the response with no change.
2. When the response status is 401, it returns the response with no change.
3. When the response has a `Set-Cookie` for `cookie_name`, it returns the response with no change. It uses `ian_framework::http::headers::sets_cookie` for this check. This step keeps the cookie of a login handler or a logout handler.
4. It adds one `Set-Cookie` with `cookie(session)`. It keeps the other `Set-Cookie` values of the response.

The expiry in Redis slides in step 2 before the handler runs. Thus a response with status 401 does not stop the slide in Redis. It stops only the new cookie.

## Session extractor

A handler behind `slide_session` takes the session as an argument of type `Session` or `Option<Session>`. The extractor does not call Redis.

| Request extensions | `Session` | `Option<Session>` |
| --- | --- | --- |
| A `Session` | The session | The session |
| No `Session` and no marker | Rejects with code `UNAUTHORIZED` (401) | `None` |
| The marker of a failed `load` | Rejects with code `AUTH_STORE` (500) | Rejects with code `AUTH_STORE` (500) |

- A handler that is not behind `slide_session` always gets no session.
- A Redis failure does not change the response of a handler that does not take a session.
