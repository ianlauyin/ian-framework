# Exception

`ian_framework::exception` exports `Exception`. It is the one error type of the library. It replaces the error enums and the error structs of the `auth`, `env`, `gcp` and `gcs` modules.

## Replaced types

`Exception` replaces the old error types `AuthError`, `EnvError`, `GcpError` and `GcsError`. The old types do not exist. A caller that matched a variant of an old type matches `Exception::code()` against a code string.

## Exception

`Exception` implements `Debug`, `Clone`, `PartialEq`, `Eq`, `Display`, `std::error::Error`, `IntoApiError`, `From<&Exception> for String` and `From<Exception> for String`.

It holds these fields (the table "Construction" gives how to make one):

| Field | Contents |
| --- | --- |
| `severity` | `Severity::Warn` or `Severity::Error`. The default is `Error`. |
| `code` | An optional `&'static str` that names the cause. The caller writes the string at the call site. The table "Codes" gives the values that the library uses. An exception can have no code. |
| `message` | The internal message. It never holds a secret, a token or a query string. |
| `source` | An optional `Box<Exception>`. The exception that caused this one. |

An external node has no code and has a message that is made only of the Rust type name of the error and, for a `std::io::Error`, its `ErrorKind`, in the form `external error: {type name}` or `external error: {type name} ({kind})`. A node of a `source` chain has the type name only when it is a `std::io::Error`. Else its message is `external error`. The framework does not store the text of an external error in any field, because that text can hold a secret, a URL or a query string. A call site that needs the detail writes a safe description in the message of its own exception.

| Field | Contents |
| --- | --- |
| `location` | The file and the line of the `exception!` call. An exception in a source chain that comes from an external error has none. |

## Construction

The only way to make an exception is the macro `exception!`, exported at the crate root. It records `file!()` and `line!()` of the call. `Exception` has no public constructor.

| Call | Effect |
| --- | --- |
| `exception!(message)` | Makes an exception without a code and without a source. The severity is `Error`. |
| `exception!(message, code = code, severity = severity, source = error)` | Each of `code`, `severity` and `source` is optional. The order is as written. `source` takes a `std::error::Error`. The error and its `source` chain become a chain of external exceptions with the message that the text above describes. When `error` is an `Exception`, the macro keeps that exception as the cause. |

| Method | Effect |
| --- | --- |
| `with_code(code)` | Gives the same exception with a different code. The other fields stay. |
| `with_severity(severity)` | Gives the same exception with a different severity. The other fields stay. |
| `severity`, `code`, `message`, `source_exception` | Read the fields. `code` gives `Option<&'static str>`. |
| `location` | Gives `(file, line)` of the call, or `None` for an external exception. |
| `trace` | Iterates from this exception to the root source. |

- `Display` writes the `message` of this exception only. It does not write the code and it does not write the sources.
- `Debug` is written by hand. It shows `severity`, `code`, `message` and `location`. It does not show the sources, because the text of an external source can hold a secret, a token or a query string.
- `PartialEq` and `Eq` compare `code` and `message`. They do not compare `location` and the sources.
- The sources stay available through `source_exception`, `trace` and `std::error::Error::source`. A caller that writes the text of a source to a log must make sure that the text has no secret.
- `std::error::Error::source` gives the boxed source.
- The `From` impls for `String` give the text for a log `error_message`. `Logger::exception(action, &exception, context)` uses it. The text has one line for each exception of `trace`, from this exception to the root source. A line is `{index}: {SEVERITY} [{code}] {message} at {file}:{line}`, with the index from 0. The `[{code}]` part is absent when the exception has no code. The ` at {file}:{line}` part is absent for an external exception. The text does not hold the text of an external error.

## Codes

| Code | Condition |
| --- | --- |
| `ENV_MISSING`, `ENV_NOT_UNICODE`, `ENV_INVALID`, `ENV_SECRET` | The `env` module. See `spec/framework_rs/env/read.md`. |
| `AUTH_CONFIG`, `AUTH_STORE`, `AUTH_PASSWORD`, `AUTH_RANDOM` | The `auth` feature. See `spec/framework_rs/auth/context.md`. |
| `GCP_AUTH`, `GCP_CONFIG`, `GCP_API`, `GCP_TRANSPORT` | The `gcp` feature. See `spec/framework_rs/gcp/context.md`. |
| `GCS_AUTH`, `GCS_CONFIG`, `GCS_NOT_FOUND`, `GCS_API`, `GCS_UNAVAILABLE`, `GCS_TRANSPORT`, `GCS_LOCAL` | The `gcs` feature. See `spec/framework_rs/gcs/context.md`. |

- A code is an upper case string that the caller writes at the call site. The library has no constant for a code. A code does not give an HTTP status or a client message. A project can use its own string as a code.

## HTTP error

- The HTTP status of an `Exception` is always 500.
- The message for the client is always `internal error`.
- The error code of the `ApiError` is the `code` of the exception, or `INTERNAL_ERROR` when it has none.
- The message for the client never holds the internal message or the trace.
- A handler that must give a different status or message returns an `ApiError` of its own.
