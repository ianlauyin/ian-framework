# Exception

`ian_framework::exception` exports `Exception`. It is the one error type of the library. It replaces the error enums and the error structs of the `auth`, `env`, `gcp` and `gcs` modules.

## Replaced types

`Exception` replaces the old error types `AuthError`, `EnvError`, `GcpError` and `GcsError`. The old types do not exist. A caller that matched a variant of an old type matches `Exception::code()` against a constant of `ian_framework::exception::code`.

## Exception

`Exception` implements `Debug`, `Clone`, `PartialEq`, `Eq`, `Display`, `std::error::Error` and `IntoApiError`.

It holds these fields (the table "Construction" gives how to make one):

| Field | Contents |
| --- | --- |
| `code` | A `&'static str` that names the cause. The caller writes the string at the call site. The table "Codes" gives the values that the library uses. |
| `status` | An optional `u16`. The status of the upstream response. |
| `message` | The internal message. It never holds a secret, a token or a query string. |
| `source` | An optional `Box<Exception>`. The exception that caused this one. |

An `external` node has a message that is made only of the Rust type name of the error and, for a `std::io::Error`, its `ErrorKind`, in the form `external error: {type name}` or `external error: {type name} ({kind})`. A node of a `source` chain has the type name only when it is a `std::io::Error`. Else its message is `external error`. The framework does not store the text of an external error in any field, because that text can hold a secret, a URL or a query string. A call site that needs the detail writes a safe description in the message of its own exception.

| Field | Contents |
| --- | --- |
| `location` | The file and the line of the `exception!` call. An exception in a source chain with the code `external` has none. |

## Construction

The only way to make an exception is the macro `exception!`, exported at the crate root. It records `file!()` and `line!()` of the call. `Exception` has no public constructor.

| Call | Effect |
| --- | --- |
| `exception!(code, message)` | Makes an exception without a source. |
| `exception!(code, message, status = status)` | Makes an exception with a status. |
| `exception!(code, message, caused_by = source)` | Makes an exception that has an `Exception` source. |
| `exception!(code, message, source = error)` | Makes an exception from a `std::error::Error`. The error and its `source` chain become a chain of exceptions with the code `external` and the message that the text above describes. When `error` is an `Exception`, the macro keeps that exception as the cause, as `caused_by` does. |

| Method | Effect |
| --- | --- |
| `with_code(code)` | Gives the same exception with a different code. The message, the status, the location and the source stay. |
| `with_status(status)` | Gives the same exception with a different optional status. The other fields stay. |
| `code`, `status`, `message`, `source_exception` | Read the fields. |
| `location` | Gives `(file, line)` of the call, or `None` for an `external` exception. |
| `trace` | Iterates from this exception to the root source. |

- `Display` writes `{code}: {message}`. It does not write the sources.
- `Debug` is written by hand. It shows `code`, `message`, `status` and `location`. It does not show the sources, because the text of an external source can hold a secret, a token or a query string.
- `PartialEq` and `Eq` compare `code`, `message` and `status`. They do not compare `location` and the sources.
- The sources stay available through `source_exception`, `trace` and `std::error::Error::source`. A caller that writes the text of a source to a log must make sure that the text has no secret.
- `std::error::Error::source` gives the boxed source.

## Codes

| Code | Condition |
| --- | --- |
| `external` | An error from a different crate, or the standard library, in a source chain. |
| `env_missing`, `env_not_unicode`, `env_invalid`, `env_secret` | The `env` module. See `spec/framework_rs/env/read.md`. |
| `auth_config`, `auth_store`, `auth_password`, `auth_random` | The `auth` feature. See `spec/framework_rs/auth/context.md`. |
| `gcp_auth`, `gcp_config`, `gcp_api`, `gcp_transport` | The `gcp` feature. See `spec/framework_rs/gcp/context.md`. |
| `gcs_auth`, `gcs_config`, `gcs_not_found`, `gcs_api`, `gcs_transport`, `gcs_local` | The `gcs` feature. See `spec/framework_rs/gcs/context.md`. |

- `ian_framework::exception::code` has one `pub const` for each code of the table. One macro in `framework_rs/src/exception.rs` defines each constant together with its HTTP status and its client message, so a code cannot exist without both. A test walks the table. The code that makes an exception and the HTTP error mapping use these constants. The names are the code in upper case, such as `code::GCS_API`.

## HTTP error

| Code | Status | Message for the client |
| --- | --- | --- |
| `auth_store` | 503 | `session store is not available` |
| `auth_config` | 500 | `session store is not available` |
| `auth_password` | 500 | `password hash failed` |
| `auth_random` | 500 | `session token failed` |
| `gcs_not_found` | 404 | `storage object not found` |
| `gcs_auth`, `gcs_config` | 500 | `storage is not available` |
| `gcs_api`, `gcs_transport` | 502 | `storage request failed` |
| `gcs_local` | 500 | `storage request failed` |
| `env_missing`, `env_not_unicode`, `env_invalid`, `env_secret`, `gcp_auth`, `gcp_config`, `gcp_api`, `gcp_transport` | 500 | `internal error` |
| Any other code | The stored `status`, when it is from 400 to 599. Else 500. | The message of the exception when the stored `status` is from 400 to 499. Else `internal error`. |

- A code of the library has a fixed status. The stored `status` does not change it, because the stored status of a `gcs` exception is the status of the upstream call.
- The stored `status` applies only to a code that is not in the table, such as a code of a project.
- A project must not use a string of the table as its own code. A project code that is a string literal in `exception!` is checked with `debug_assert!`, and a debug build panics when the literal is a library code. A project code that is not a literal, such as a constant, is not checked.
- A message of a 4xx project exception goes to the client. The message must not hold a secret.

- The error code of the `ApiError` is the `code` of the exception.
- The message for the client never holds the trace. It holds the internal message only for a project code with a stored status from 400 to 499.
