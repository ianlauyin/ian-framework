# Sliding session

The parts that apply to all of this feature are in `spec/framework_rs/auth/context.md`.

## Sliding session

`slide_session` is the Axum middleware function. Use it with `axum::middleware::from_fn_with_state` and one `SessionStore`. The session store is in `spec/framework_rs/auth/session-store.md`.

The middleware reads the token before the inner handler runs. After the handler returns, the middleware does these steps:

1. When the response status is 401, it returns the response with no change.
2. When the request has no token, it returns the response with no change.
3. When the response has a `Set-Cookie` for `cookie_name`, it returns the response with no change. It uses `ian_framework::http::headers::sets_cookie` for this check. This step keeps the cookie of a login handler or a logout handler.
4. It calls `touch` for the token.
5. When `touch` returns `true`, it adds one `Set-Cookie` with `cookie(token)`. It keeps the other `Set-Cookie` values of the response.

A Redis failure in the middleware does not change the response.
