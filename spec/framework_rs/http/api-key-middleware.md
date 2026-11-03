# API key middleware

The parts that apply to all of this feature are in `spec/framework_rs/http/context.md`. The key check functions are in `spec/framework_rs/http/bearer-key.md`.

## API key middleware

- The source file is `framework_rs/src/http/middleware/require_api_key.rs`.
- `ian_framework::http::middleware::require_api_key` is an Axum middleware function.
- The project adds it to a router with `axum::middleware::from_fn_with_state` and one `ApiKeyRules`.

| Type | Meaning |
| --- | --- |
| `PathMatch` | How a rule matches the request path. |
| `ApiKeyRule` | One path match, an optional list of methods, and one key. A public rule has no key. |
| `ApiKeyRules` | The ordered list of rules. `ApiKeyRules::new(rules)` makes it. A clone shares the rules. |

`ApiKeyRule::new(path, key)` makes a rule that matches each method. `ApiKeyRule::methods(methods)` limits the rule to the given methods. `ApiKeyRule::public(path)` makes a rule that matches each method and needs no key.

`ApiKeyRules::deny_unmatched()` makes the rules refuse a request that no rule matches. Without it, such a request passes.

`PathMatch` has these variants:

| Variant | Matches |
| --- | --- |
| `Exact(path)` | The path that is equal to `path`. |
| `Tree(root)` | The path `root` and each path that starts with `root` and a `/`. |
| `Child(root)` | A path that is `root`, a `/`, and one name that is not empty and has no `/`. |

## Check

For each request, the middleware finds the first rule that matches the method and the path.

- The match uses the path without one trailing `/`. Thus the path `/log/` matches the rules of the path `/log`. The path `/` is not changed.
- When no rule matches, the middleware passes the request to the next handler. When the rules have `deny_unmatched`, the middleware sends the 401 response instead.
- When a public rule matches, the middleware passes the request to the next handler with no key.
- When a rule matches, the request needs the header `Authorization: Bearer <key>`. The check uses `bearer_token` and `key_matches`.
- When the check passes, the middleware passes the request to the next handler.
- When the check fails, the middleware does not call the next handler. The response is an `ApiError` with status 401, the error code `unauthorized`, and the message `unauthorized`.

The 401 response body is:

```json
{"code":"unauthorized","message":"unauthorized"}
```

Rules:

- The order of the rules matters. The first match decides. A key of a later rule does not work on a path that an earlier rule matches.
- The middleware writes one `warn` event with the request method for each 401. The event has no key and no path.
- The `Debug` output of `ApiKeyRule` shows `[redacted]` in place of the key.
- A rule with an empty key never passes a request, because `key_matches` is `false` for an empty key.
