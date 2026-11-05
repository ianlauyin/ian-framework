# Token68 check

The parts that apply to all of this feature are in `spec/framework_rs/token/context.md`.

## Public items

`ian_framework::token` exports only `is_token68`.

## is_token68

`is_token68(token: &str)` gives `true` only if the text is an RFC 9110 `token68`.

- The text has one or more of the characters `A`-`Z`, `a`-`z`, `0`-`9`, `-`, `.`, `_`, `~`, `+`, `/`.
- Then the text has zero or more `=`.
- `bearer_token` uses `is_token68` for the token. The bearer rules are in `spec/framework_rs/http/bearer-key.md`.
- A project can use `is_token68` to reject an API key at startup. `bearer_token` never gives a token that is not a `token68`. Thus such a key never matches.
