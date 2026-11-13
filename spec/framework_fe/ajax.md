# Ajax client

This document describes the JSON request client in the frontend framework.
This feature is not product business logic.

## Package

- The package is `ian-framework-fe`. The package setup is in `spec/framework_fe/version.md`.
- The import path is `ian-framework-fe`.
- The source files are `framework_fe/src/http/ajax.ts`, `framework_fe/src/http/network.ts` and `framework_fe/src/http/raw-send.ts`. `ajax.ts` builds the URL and the body. `network.ts` holds the error classes and the query string. `ajax`, `upload` and `download-file` throw the same error classes. `raw-send.ts` holds the send step, the interceptor, the unauthorized handler and the JSON parse. It is shared by `ajax` and `upload`.
- The test file is `framework_fe/test/http/ajax.test.ts`.
- `framework_fe/src/http/index.ts` re-exports the public items of `http`, and the root entry re-exports it.
- The root entry exports the error classes, the types and the two setter functions that are in the API table.
- The client uses only browser APIs. The client does not use a UI library.

## API

| Item | Kind | Meaning |
| --- | --- | --- |
| `ajax(url, method, request?, pathParams?)` | function | Sends one request and returns the parsed JSON body. |
| `ApiError` | class | The error for a response that the app gave with a status that is not 2xx. |
| `NetworkConnectionException` | class | The error for a request that did not give a usable response. |
| `ErrorBody` | type | The error body: `{ message: string }`. |
| `RequestInterceptor` | type | `(request: { headers: Headers }) => void \| Promise<void>`. |
| `setUnauthorizedHandler(handler?)` | function | Sets the handler for a 401 response. A call with no handler removes the handler. |
| `setRequestInterceptor(interceptor?)` | function | Sets the interceptor that changes the headers of each request. A call with no interceptor removes it. |

`method` is one of `GET`, `DELETE`, `POST` and `PUT`.

`ApiError` has these fields:

| Field | Type | Value |
| --- | --- | --- |
| `code` | `number` | The HTTP status. |
| `message` | `string` | `message` from the error body when it is a string. Otherwise `Unknown error`. |
| `requestURL` | `string` | The URL of the request, with the prefix `/ajax`. |

`NetworkConnectionException` has these fields:

| Field | Type | Value |
| --- | --- | --- |
| `message` | `string` | The reason, in text. |
| `requestURL` | `string` | The URL of the request, with the prefix `/ajax`. |

Both classes extend `Error`.

The type of `pathParams` comes from the `{name}` parts of `url`, when `url` is a text literal. The type requires each name, and its value is a string, a number or a `bool`. When `url` has no `{name}` part, `pathParams` is optional.

## Behaviour

### URL

- The client adds the prefix `/ajax` to `url`. The project serves its JSON routes under `/ajax` on the same origin.
- The client replaces each `{name}` in `url` with `pathParams[name]`.
- The client changes the path param value to text. Then the client encodes the text with `encodeURIComponent`. The client puts the encoded text in the URL.
- If a path param value is `.` or `..`, the client throws an `Error`, because the encoding keeps it and the browser would change the path. The client does not send the request.
- If a path param value is `undefined`, `null` or the empty string, the client throws an `Error`. The client does not send the request.
- The values `0` and `false` are valid path param values.

### Request

| Method | Request data | Body |
| --- | --- | --- |
| `GET`, `DELETE` | The client adds `request` to the URL as a query string. It leaves out a key whose value is `undefined`. When no key is left, the URL has no `?`. | No body. |
| `POST`, `PUT` | The client sends `request` as a JSON body. | JSON. The request has the header `Content-Type: application/json`. |

Each request uses `credentials: "same-origin"`.

- The client always sends a `Headers` object. For `POST` and `PUT`, it has `Content-Type: application/json`.
- The client calls the request interceptor with the headers before it sends the request, and waits for it. The interceptor can add or change a header. `upload` in `spec/framework_fe/file-transfer.md` uses the same interceptor and the same unauthorized handler.
- In the query string, a `Date` value is written with `toISOString()`.

### Response

- When the fetch fails or the body cannot be read, the client throws `NetworkConnectionException`. `message` is the message of the cause.
- When the status is 401, the client calls the unauthorized handler. Then the client throws `ApiError`.
- When the status is 0, 502 or 504, the client throws `NetworkConnectionException` with the message `HTTP {status}`. A gateway or a proxy gives these statuses, so the app was not reached. The status 503 stays an `ApiError`, because the app uses it for a planned stop.
- When the status is not 2xx and not one of those, the client throws `ApiError`.
- When the status is not 2xx and the body is empty, not valid JSON, or has no string `message`, the message is `Unknown error`.
- When the status is 2xx and the body is empty, the result is `null`.
- When the status is 2xx and the body is not empty, the client parses the body as JSON. Each string that is a full ISO 8601 date and time with a time zone (`Z` or an offset) becomes a `Date`. A date with no time stays a string.
- When the status is 2xx and the body is not valid JSON, the client throws `NetworkConnectionException`. A gateway page is the usual cause.

## Out of scope

This feature does not include these items:

- The text for the user for each error message.
- A retry or a timeout.
