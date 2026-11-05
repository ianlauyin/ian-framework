# API error

The parts that apply to all of this feature are in `spec/framework_rs/http/context.md`.

## Exception response

`Exception` implements `IntoResponse` in this module. `Exception` is in `spec/framework_rs/exception.md`. A handler can return `Result<T, Exception>`.

- A domain error does not implement `IntoResponse`.
- An I/O error does not implement `IntoResponse`.

The impl uses the code of the top exception.

| Code of the exception | Status | Message |
| --- | --- | --- |
| `VALIDATION_ERROR` or `BAD_REQUEST` | 400 | The message of the exception |
| `UNAUTHORIZED` | 401 | The message of the exception |
| `FORBIDDEN` | 403 | The message of the exception |
| `NOT_FOUND` | 404 | The message of the exception |
| `REQUEST_TIMEOUT` | 408 | The message of the exception |
| `PAYLOAD_TOO_LARGE` | 413 | The message of the exception |
| `TOO_MANY_REQUESTS` | 429 | The message of the exception |
| `INTERNAL_ERROR` | 500 | The message of the exception |
| A different code | 500 | `internal server error` |

- Only the code of the top exception counts. The code of a source does not change the result. Thus a client error that a caller wraps in a different exception becomes status 500.
- A response for a different code never holds the internal message or the trace.

## Well-known codes

The module `ian_framework::http::error_code` exports one `&'static str` constant for each well-known code. The value of each constant is its name.

| Constant | Condition |
| --- | --- |
| `VALIDATION_ERROR` | A value from the client does not obey a rule. |
| `BAD_REQUEST` | The client sent a request that the code cannot read. |
| `UNAUTHORIZED` | The client is not authenticated. |
| `FORBIDDEN` | The client cannot do this action. |
| `NOT_FOUND` | The item that the client asked for does not exist. |
| `REQUEST_TIMEOUT` | The request took too long. |
| `PAYLOAD_TOO_LARGE` | The request body is too large. |
| `TOO_MANY_REQUESTS` | The client sent too many requests. |
| `INTERNAL_ERROR` | The server failed. |

- The message of an exception with a well-known code goes to the client. Write it for the client. It must not hold an internal detail.
- Each other code means that the cause is internal.

## JSON response

`IntoResponse` for `Exception` builds the HTTP response.

- The response status comes from the code of the exception.
- The content type is `application/json`.
- The body is one flat JSON object.
- The body has no `error` key.
- The body has one key, `message`. The value is the message.
