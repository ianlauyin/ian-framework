# Health route

The parts that apply to all of this feature are in `spec/framework_rs/http/context.md`.

## Health route

- The source file is `framework_rs/src/http/router/health.rs`.
- `ian_framework::http::router::health_router()` gives an `axum::Router` with one route.
- The route is `GET /health`.
- The project merges this router into its own router.
- The project adds its own layers to the merged router.
- The route has no authentication.

The response:

- The response status is 200.
- The content type is `application/json`.
- The body is one flat JSON object with the keys in the table below.

| Key | Value |
| --- | --- |
| `ok` | `true`. |

- Every request gets the same body.
- The route does not read the process state, the disk, or a dependency.
