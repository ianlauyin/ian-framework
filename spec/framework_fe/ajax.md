# Ajax client

This document describes the JSON request client in the frontend framework.
This feature is not product business logic.

## Package

The package is `ian-framework-fe`. The package setup is in `spec/framework_fe/version.md`.
The import path is `ian-framework-fe/ajax`.
The source file is `framework_fe/src/ajax.ts`.
The test file is `framework_fe/test/ajax.test.ts`.
The client uses only browser APIs. The client does not use a UI library.

## API

| item | kind | meaning |
| --- | --- | --- |
| `ajax(url, method, request?, pathParams?)` | function | sends one request and returns the parsed JSON body |
| `AjaxError` | class | the error for a response status that is not 2xx |
| `ErrorBody` | type | the error body: `{ message: string }` |
| `setUnauthorizedHandler(handler?)` | function | sets the handler for a 401 response. A call with no handler removes the handler |

`method` is one of `GET`, `DELETE`, `POST` and `PUT`.

`AjaxError` has these fields:

| field | type | value |
| --- | --- | --- |
| `code` | `number` | the HTTP status |
| `message` | `string` | `message` from the error body, or `Unknown error` if the body has no message |

## Behaviour

The client adds the prefix `/ajax` to `url`. The project serves its JSON routes under `/ajax` on the same origin.

The client replaces each `{name}` in `url` with `pathParams[name]`. If a path param has no value, the client throws an `Error` and does not send the request.

For `GET` and `DELETE`:

- The client adds `request` to the URL as a query string.
- The request has no body.

For `POST` and `PUT`:

- The client sends `request` as a JSON body.
- The request has the header `Content-Type: application/json`.

Each request uses `credentials: "same-origin"`.

The client reads the response:

- When the status is 401, the client calls the unauthorized handler before it throws.
- When the status is not 2xx, the client throws `AjaxError`.
- For a status that is not 2xx, a body that is empty or not valid JSON gives the error body `null`, so the message is `Unknown error`. An example is an HTML 502 page from a proxy.
- For a 2xx status, an empty body gives `null`.
- For a 2xx status, a body that is not empty is parsed as JSON.

## Out of scope

This feature does not include these items:

- The text for the user for each error message.
- A retry or a timeout.
- An `ApiError` body with a `code` field. The client reads only `message`. The Rust `ApiError` is in `spec/framework_rs/http.md`.
