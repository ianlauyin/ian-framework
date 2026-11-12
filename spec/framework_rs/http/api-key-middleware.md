# API key middleware

The parts that apply to all of this feature are in `spec/framework_rs/http/context.md`. The key check functions are in `spec/framework_rs/http/bearer-key.md`.

## API key middleware

- The source file is `framework_rs/src/http/middleware/require_api_key.rs`.
- `ian_framework::http::middleware::require_api_key` is an Axum middleware function.
- The project adds it to a router with `axum::middleware::from_fn_with_state` and one `ApiKey`.
- `ApiKey::new(key)` makes the key. A clone shares the key.
- Each request that goes through the layer needs the key. The middleware has no path rule, no method rule and no public path. To leave a route without a key, the project does not put the layer on that route.

## Check

- The request needs the header `Authorization: Bearer <key>`. The check uses `bearer_token` and `key_matches`. `key_matches` is not an export of `ian_framework::http::headers`.
- When the check passes, the middleware passes the request to the next handler.
- When the check fails, the middleware does not call the next handler. The response has status 401 and the message `unauthorized`.
- Before the 401 response, the middleware calls `Logger::warn` with the action `HTTP/apiKeyRejected` and the context key `method`. The logger rules are in `spec/framework_rs/logging/logger.md`.

The 401 response body is:

```json
{"message":"unauthorized"}
```

Rules:

- The middleware writes one `warn` event with the request method for each 401. The event has no key and no path.
- The `Debug` output of `ApiKey` shows `[redacted]` in place of the key.
- An empty key never passes a request, because `key_matches` is `false` for an empty key.
