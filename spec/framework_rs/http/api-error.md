# API error

The parts that apply to all of this feature are in `spec/framework_rs/http/context.md`.

## ApiError

`ApiError` is the HTTP boundary error.

- Only `ApiError` in this module implements `IntoResponse`.
- A domain error does not implement `IntoResponse`.
- An I/O error does not implement `IntoResponse`.
- `ApiError` does not implement `std::error::Error`.
- `ApiError` does not store `Box<dyn std::error::Error>`.

`ApiError` has these fields. A caller can read each field.

| Field | Type | Value |
| --- | --- | --- |
| `status` | `axum::http::StatusCode` | The HTTP status. |
| `code` | `String` | The stable error code. |
| `message` | `String` | The error text. |

The display text is the error code, then `: `, then the message.

### Constructors

| Function | Arguments, in order | Result |
| --- | --- | --- |
| `ApiError::new` | status, error code, message | One `ApiError`. |
| `ApiError::from_status_code` | status number, error code, message | One `ApiError` from a status number. |

`ApiError::from_status_code` uses these rules:

- A status number from 100 through 999 stays that status.
- Each other status number, zero included, becomes status 500.
- The error code does not change.
- The message does not change.

## From Exception

`ApiError` implements `From<Exception>`. The impl is in this module. `Exception` is in `spec/framework_rs/exception.md`.

| Code of the exception | Status | Error code | Message |
| --- | --- | --- | --- |
| `VALIDATION_ERROR` or `BAD_REQUEST` | 400 | The code of the exception | The message of the exception |
| `FORBIDDEN` | 403 | The code of the exception | The message of the exception |
| `NOT_FOUND` | 404 | The code of the exception | The message of the exception |
| A different code | 500 | `INTERNAL_ERROR` | `internal error` |

- Only the code of the top exception counts. The code of a source does not change the result. Thus a client error that a caller wraps in a different exception becomes status 500.
- A response with status 500 never holds the internal message or the trace.
- A handler that must give a different status or message returns an `ApiError` of its own.

## JSON response

`IntoResponse` for `ApiError` builds the HTTP response.

- The response status is the `status` field.
- The content type is `application/json`.
- The body is one flat JSON object.
- The body has no `error` key.
- The body has only the keys in the table below.

| Key | Value |
| --- | --- |
| `code` | The error code. |
| `message` | The message. |

## Trait IntoApiError

A project error type implements `IntoApiError`. The trait has no Axum type.

The trait has these methods:

| Method | Result |
| --- | --- |
| `status_code` | The status number, `u16`. |
| `error_code` | The stable error code, `&str`. |
| `message` | The error text, `String`. |

`ApiError` implements `From` for each type that implements `IntoApiError`. `ApiError::from` uses these rules:

- It reads `status_code`, `error_code`, and `message`.
- It uses the status rule of `ApiError::from_status_code`. Thus an invalid status number becomes status 500.
- The error code does not change.
- The message does not change.

A handler can return `Result<T, ApiError>`. These two forms convert a project error into `ApiError`:

- The `?` operator.
- `map_err(ApiError::from)`.
