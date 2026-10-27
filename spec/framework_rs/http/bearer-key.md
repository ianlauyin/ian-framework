# Bearer API key

The parts that apply to all of this feature are in `spec/framework_rs/http/context.md`.

## Bearer API key

A server can use a bearer API key to authenticate a request.

The module has these functions:

| Function | Result |
| --- | --- |
| `bearer_token(headers: &axum::http::HeaderMap)` | The token, `Option<&str>`. |
| `key_matches(token: &str, key: &str)` | The match result, `bool`. |
| `is_token68(token: &str)` | The token68 check result, `bool`. |

### bearer_token

`bearer_token` reads the `Authorization` header.

- The header value must be the scheme `Bearer`, then one space, then the token.
- The scheme match is not case sensitive.
- The token is an RFC 9110 `token68`. The rule is in the `is_token68` section.

`bearer_token` gives `None` in these conditions:

- The request has no `Authorization` header.
- The header value is not UTF-8 text.
- The scheme is not `Bearer`.
- The header value has no space.
- The token is not a `token68`.

### is_token68

`is_token68` gives `true` only if the text is an RFC 9110 `token68`.

- The text has one or more of the characters `A`-`Z`, `a`-`z`, `0`-`9`, `-`, `.`, `_`, `~`, `+`, `/`.
- Then the text has zero or more `=`.
- `bearer_token` uses `is_token68` for the token.
- A project can use `is_token68` to reject an API key at startup. `bearer_token` never gives a token that is not a `token68`. Thus such a key never matches.

### key_matches

- `key_matches` makes the SHA-256 hash of the token and the SHA-256 hash of the key.
- It compares the two hashes in constant time with `subtle::ConstantTimeEq`.
- The comparison time does not depend on the position of the first different byte.
- The comparison time does not depend on the length of the key.
- `key_matches` gives `true` only if the token and the key are equal.
- `key_matches` gives `false` when the key is empty. Thus a key that is not set does not match.

### Project tasks

- The project keeps the API key.
- The project can check the API key with `is_token68` at startup.
- The project selects the HTTP status and the error for a rejected request.
