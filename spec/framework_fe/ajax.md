# Ajax client

This document describes the JSON request client in the frontend framework.
This feature is not product business logic.

## Package

- The package is `ian-framework-fe`. The package setup is in `spec/framework_fe/version.md`.
- The import path is `ian-framework-fe/ajax`.
- The source file is `framework_fe/src/ajax.ts`.
- The test file is `framework_fe/test/ajax.test.ts`.
- The client uses only browser APIs. The client does not use a UI library.

## API

| Item | Kind | Meaning |
| --- | --- | --- |
| `ajax(url, method, request?, pathParams?)` | function | Sends one request and returns the parsed JSON body. |
| `AjaxError` | class | The error for a response status that is not 2xx. |
| `ErrorBody` | type | The error body: `{ message: string }`. |
| `setUnauthorizedHandler(handler?)` | function | Sets the handler for a 401 response. A call with no handler removes the handler. |

`method` is one of `GET`, `DELETE`, `POST` and `PUT`.

`AjaxError` has these fields:

| Field | Type | Value |
| --- | --- | --- |
| `code` | `number` | The HTTP status. |
| `message` | `string` | `message` from the error body. If the body has no message, the value is `Unknown error`. |

## Behaviour

### URL

- The client adds the prefix `/ajax` to `url`. The project serves its JSON routes under `/ajax` on the same origin.
- The client replaces each `{name}` in `url` with `pathParams[name]`.
- The client changes the path param value to text. Then the client encodes the text with `encodeURIComponent`. The client puts the encoded text in the URL.
- If a path param value is `undefined`, `null` or the empty string, the client throws an `Error`. The client does not send the request.
- The values `0` and `false` are valid path param values.

### Request

| Method | Request data | Body |
| --- | --- | --- |
| `GET`, `DELETE` | The client adds `request` to the URL as a query string. | No body. |
| `POST`, `PUT` | The client sends `request` as a JSON body. | JSON. The request has the header `Content-Type: application/json`. |

Each request uses `credentials: "same-origin"`.

### Response

- When the status is 401, the client calls the unauthorized handler. Then the client throws.
- When the status is not 2xx, the client throws `AjaxError`.
- When the status is not 2xx and the body is empty or not valid JSON, the error body is `null`. Then the message is `Unknown error`.
- When the status is 2xx and the body is empty, the result is `null`.
- When the status is 2xx and the body is not empty, the client parses the body as JSON.

## Out of scope

This feature does not include these items:

- The text for the user for each error message.
- A retry or a timeout.
- An `ApiError` body with a `code` field. The client reads only `message`. The Rust `ApiError` is in `spec/framework_rs/http.md`.
