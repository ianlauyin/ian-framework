# Session middleware

The parts that apply to all of this feature are in `spec/framework_rs/auth/context.md`.

## Middleware

`ian_framework::http::middleware::require_session` is the Axum middleware function. The source file is `framework_rs/src/http/middleware/require_session.rs`. It is available with the Cargo feature `auth`. Use it with `axum::middleware::from_fn_with_state` and one `SessionStore`. The session store is in `spec/framework_rs/auth/session-store.md`.

The middleware protects the route. Every route behind it needs a token that matches a live session. Do not put a login route behind it, because a login route has no session yet.

The middleware does these steps before the inner handler runs:

1. It reads the token with `SessionStore::token`. When the request has no token, it returns the exception with code `UNAUTHORIZED` (401). It does not call Redis.
2. It calls `load` for the token. This is the only Redis call of the request for the session.
3. When `load` gives no session, it returns the exception with code `UNAUTHORIZED` (401).
4. When `load` fails, it returns the `Exception` of `load` as a response with `into_response`. The code of this exception is `AUTH_STORE`, so the client gets status 500 with the message `internal server error`. The middleware does not log the error. `access_log` logs the exception. Thus a request fails when Redis fails.
5. When `load` gives a `Session`, it inserts the `Session` in the request extensions.

In steps 1, 3 and 4 the inner handler does not run and no cookie is added.

After the handler returns, the middleware does these steps:

1. When the response status is 401, it returns the response with no change.
2. When the response has a `Set-Cookie` for `cookie_name`, it returns the response with no change. It uses `ian_framework::http::headers::sets_cookie` for this check. This step keeps the cookie of a logout handler.
3. It adds one `Set-Cookie` with `cookie(token)`. It keeps the other `Set-Cookie` values of the response.

The expiry in Redis slides in step 2 before the handler runs. Thus a response with status 401 does not stop the slide in Redis. It stops only the new cookie.

## Session extractor

A handler behind `require_session` takes the session as an argument of type `Session`. The extractor does not call Redis. The middleware always gives a `Session`, so the extractor does not reject on such a route.

A handler that is not behind `require_session` can take `Option<Session>`. It always gets `None`. A `Session` argument always rejects with code `UNAUTHORIZED` (401).
