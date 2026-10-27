# Cookie headers

The parts that apply to all of this feature are in `spec/framework_rs/http/context.md`.

## Cookie headers

The module `ian_framework::http::headers` has these functions:

| Function | Result |
| --- | --- |
| `read_cookie(headers: &axum::http::HeaderMap, name: &str)` | The cookie value, `Option<&str>`. |
| `sets_cookie(headers: &axum::http::HeaderMap, name: &str)` | The match result, `bool`. |

`read_cookie` reads each `Cookie` request header. An HTTP/2 client can send each cookie in a different `Cookie` header.

- `read_cookie` returns the value of the first cookie with the name `name`.
- It skips a `Cookie` header that is not UTF-8 text.
- It returns `None` when no cookie has the name.
- It does not remove quotes from the value.

`sets_cookie` reads each `Set-Cookie` response header. It returns `true` when one of them sets the cookie with the name `name`.

Cookie sessions use these functions. Cookie sessions are in `spec/framework_rs/auth/session-store.md`.
