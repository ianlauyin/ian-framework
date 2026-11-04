# Exception

`ian_framework::exception` exports `Exception`. It is the one error type of the library. It replaces the error enums and the error structs of the `auth`, `env`, `gcp` and `gcs` modules.

## Replaced types

`Exception` replaces the old error types `AuthError`, `EnvError`, `GcpError` and `GcsError`. The old types do not exist. A caller that matched a variant of an old type matches `Exception::code()` against a code string.

## Exception

`Exception` implements `Debug`, `Display`, `std::error::Error`, `From<&Exception> for String` and `From<Exception> for String`. `ApiError` implements `From<Exception>` in `ian_framework::http`.

It holds these fields (the table "Construction" gives how to make one):

| Field | Contents |
| --- | --- |
| `severity` | `Severity::Warn` or `Severity::Error`. The default is `Error`. |
| `code` | A `&'static str` that names the cause. The caller writes the string at the call site. The table "Codes" gives the values that the library uses. Each exception has a code. |
| `message` | The internal message. It never holds a secret, a token or a query string. |
| `source` | An optional `Box<dyn std::error::Error + Send + Sync>`. The error that caused this exception. |

The library stores the given error as `source`. It does not make a new exception for an external error. `Display`, `Debug` and the `String` text do not write the source. The text of an external error can hold a secret, a URL or a query string. A call site that needs the detail writes a safe description in the message of its own exception.

| Field | Contents |
| --- | --- |
| `location` | The file and the line of the `exception!` call. Each exception has a location. |

## Construction

The only way to make an exception is the macro `exception!`. It is exported at the crate root. It records `file!()` and `line!()` of the call. `Exception` has no public constructor.

| Call | Effect |
| --- | --- |
| `exception!(message, code = code)` | Makes an exception without a source. The severity is `Error`. `code` is required and must be a `&'static str`. |
| `exception!(message, code = code, severity = severity, source = error)` | `severity` and `source` are optional. The order is as written. `source` takes a `std::error::Error + Send + Sync`. The macro stores that error as the source. |

| Method | Effect |
| --- | --- |
| `with_code(code)` | Gives the same exception with a different code. The other fields stay. |
| `with_severity(severity)` | Gives the same exception with a different severity. The other fields stay. |
| `severity`, `code`, `message`, `source_exception` | Read the fields. `code` gives `&'static str`. `source_exception` gives the source when the source is an `Exception`. |
| `location` | Gives `(file, line)` of the call. |
| `trace` | Iterates from this exception through each source that is an `Exception`. An external error ends the iteration. |

- `Display` writes the `message` of this exception only. It does not write the code and it does not write the source.
- `Debug` is written by hand. It shows `severity`, `code`, `message` and `location`. It does not show the source, because the text of an external source can hold a secret, a token or a query string.
- `Exception` does not implement `Clone`, `PartialEq` or `Eq`. Compare `code` and `message`.
- `std::error::Error::source` gives the boxed source. When the source is an `Exception`, `source_exception` and `trace` also give it. A caller that writes the text of a source to a log must make sure that the text has no secret.
- The `From` impls for `String` give the text for a log `error_message`. `Logger::exception(action, &exception, context)` uses it. The text has one line for each exception of `trace`, from this exception to the last `Exception` source. A line is `{index}: {SEVERITY} [{code}] {message} at {file}:{line}`, with the index from 0. The text does not hold an external source.

## Codes

| Code | Condition |
| --- | --- |
| `ENV_MISSING`, `ENV_NOT_UNICODE`, `ENV_INVALID`, `ENV_SECRET` | The `env` module. See `spec/framework_rs/env/read.md`. |
| `AUTH_CONFIG`, `AUTH_STORE`, `AUTH_PASSWORD`, `AUTH_RANDOM` | The `auth` feature. See `spec/framework_rs/auth/context.md`. |
| `GCP_AUTH`, `GCP_CONFIG`, `GCP_API`, `GCP_TRANSPORT` | The `gcp` feature. See `spec/framework_rs/gcp/context.md`. |
| `GCS_AUTH`, `GCS_CONFIG`, `GCS_NOT_FOUND`, `GCS_API`, `GCS_UNAVAILABLE`, `GCS_TRANSPORT`, `GCS_LOCAL` | The `gcs` feature. See `spec/framework_rs/gcs/context.md`. |

- A code is an upper case string that the caller writes at the call site. A project can use its own string as a code.
- `Exception` has no HTTP status. The `exception` module does not use `axum` or `ian_framework::http`.

## Well-known codes

The module `ian_framework::exception::error_code` exports one `&'static str` constant for each well-known code. The value of each constant is its name.

| Constant | Condition |
| --- | --- |
| `VALIDATION_ERROR` | A value from the client does not obey a rule. |
| `BAD_REQUEST` | The client sent a request that the code cannot read. |
| `FORBIDDEN` | The client cannot do this action. |
| `NOT_FOUND` | The item that the client asked for does not exist. |

- A well-known code means that the cause is the client. Each other code means that the cause is internal.
- The message of an exception with a well-known code goes to the client. Write it for the client. It must not hold an internal detail.
- Each transport gives the meaning of a well-known code. The HTTP rules are in `spec/framework_rs/http/api-error.md`.
