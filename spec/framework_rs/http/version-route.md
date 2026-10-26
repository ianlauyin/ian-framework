# Process version route

The parts that apply to all of this feature are in `spec/framework_rs/http/context.md`.

## Process version route

- The source file is `framework_rs/src/http/version.rs`.
- `ian_framework::http::version_router()` gives an `axum::Router` with one route.
- The route is `GET /version`.
- The project merges this router into its own router.
- The project adds its own layers to the merged router.
- The route has no authentication.
- The client code that reads the route is in `spec/framework_fe/version.md`.

The response:

- The response status is 200.
- The content type is `application/json`.
- The body is one flat JSON object with the keys in the table below.

| Key | Value |
| --- | --- |
| `version` | The process version, a string. |

The process version:

- The process version is 16 random bytes from `getrandom`, in lowercase hex. It has 32 characters.
- The first request makes the process version.
- Each later request in the same process gets the same value.
- A new process makes a new value.
- A client compares the value with the last value. When the value changes, a new server process is running.
- The process version is not the build version or the release version of the project.

When `getrandom` fails, the response status is 500 and the body is empty. The next request tries again.
