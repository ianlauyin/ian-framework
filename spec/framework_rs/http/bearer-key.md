# Bearer API key

The parts that apply to all of this feature are in `spec/framework_rs/http/context.md`.

## Bearer API key

A server can use a bearer API key to authenticate a request.

The module `ian_framework::http::headers` exports `bearer_token` for this feature.

### bearer_token

`bearer_token(headers: &axum::http::HeaderMap)` gives the token, `Option<&str>`.

`bearer_token` reads the `Authorization` header.

- The header value must be the scheme `Bearer`, then one space, then the token.
- The scheme match is not case sensitive.
- The token is an RFC 9110 `token68`. The check is `ian_framework::token::is_token68`. The rules are in `spec/framework_rs/token/token68.md`.

`bearer_token` gives `None` in these conditions:

- The request has no `Authorization` header.
- The header value is not UTF-8 text.
- The scheme is not `Bearer`.
- The header value has no space.
- The token is not a `token68`.

### key_matches

`key_matches` is a function of the crate. `ian_framework::http::headers` does not export it. A project does not call it. `require_api_key` calls it. The middleware rules are in `spec/framework_rs/http/api-key-middleware.md`.

- `key_matches` makes the SHA-256 hash of the token and the SHA-256 hash of the key.
- It compares the two hashes in constant time with `subtle::ConstantTimeEq`.
- The comparison time does not depend on the position of the first different byte.
- The comparison time does not depend on the length of the key.
- `key_matches` gives `true` only if the token and the key are equal.
- `key_matches` gives `false` when the key is empty. Thus a key that is not set does not match.

### Project tasks

- The project keeps the API key.
- The project can check the API key with `ian_framework::token::is_token68` at startup. The rules are in `spec/framework_rs/token/token68.md`.
- The request check is `ian_framework::http::middleware::require_api_key`. The response rules are in `spec/framework_rs/http/api-key-middleware.md`.
