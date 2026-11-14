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
- `ApiKey::on_reject(callback)` sets a callback and returns the `ApiKey`. The callback is `Fn(&Request) + Send + Sync + 'static`. A clone shares the callback.
- Before the 401 response, the middleware calls the callback with the request, when the project set one. The middleware does not write a log event.

The 401 response body is:

```json
{"message":"unauthorized"}
```

Rules:

- The callback runs once for each 401. The callback does not run for a request that passes.
- The `Debug` output of `ApiKey` shows `[redacted]` in place of the key.
- An empty key never passes a request, because `key_matches` is `false` for an empty key.
