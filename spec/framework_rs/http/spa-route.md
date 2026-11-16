# SPA route

The parts that apply to all of this feature are in `spec/framework_rs/http/context.md`.

## SPA router

- The source file is `framework_rs/src/http/router/spa.rs`.
- `ian_framework::http::router::spa_router()` gives an `axum::Router`. The function has no argument.
- The router has only a fallback handler. It handles every method and every path.
- The project merges this router into its own router. The project routes take priority over the fallback.
- The route has no authentication.
- The router always reads the folder `frontend/dist`. The path is relative to the working directory of the process.
- The project cannot change this path.

## Response

| Case | Status | Body |
| --- | --- | --- |
| `frontend/dist` cannot be resolved | 500 | The `INTERNAL_ERROR` response of `ian_framework::http::error_code`. |
| The path is a file inside `frontend/dist` | 200 | The file. |
| The path is empty, is not a file, or is outside `frontend/dist` | 200 | The file `index.html` of `frontend/dist`. |
| The file to serve cannot be read | 500 | The `INTERNAL_ERROR` response of `ian_framework::http::error_code`. |

- Before each 500 response, the router calls `Logger::error` with an `Exception` that has the code `NOT_FOUND` and the I/O error as source. The log context has the key `path`.
- The response to the client has the code `INTERNAL_ERROR`. The logged code and the response code are different on purpose.
- The message of the log and of the response is `spa is missing`.
- The action of the log is `SPA/distMissing` or `SPA/fileUnreadable`.
- The 500 response body does not show the path.
- The router resolves the path with `canonicalize`. A path that leaves `frontend/dist`, also through a link, serves `index.html`.
- The `content-type` header comes from the extension of the served file.
- An unknown extension gives `application/octet-stream`.
- The router reads the file for each request. It does not cache.

## Content types

| Extension | Content type |
| --- | --- |
| `html` | `text/html; charset=utf-8` |
| `css` | `text/css; charset=utf-8` |
| `js` | `text/javascript; charset=utf-8` |
| `svg` | `image/svg+xml` |
| `png` | `image/png` |
| `jpg`, `jpeg` | `image/jpeg` |
| `webp` | `image/webp` |
| `woff2` | `font/woff2` |
| `json` | `application/json` |
| `webmanifest` | `application/manifest+json` |
| `ico` | `image/x-icon` |
| `txt` | `text/plain; charset=utf-8` |
