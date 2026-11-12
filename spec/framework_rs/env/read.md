# Read functions and the error type

The parts that apply to all of this feature are in `spec/framework_rs/env/context.md`.

## Public items

`ian_framework::env` exports these items:

| Item | Kind |
| --- | --- |
| `required` | Function |
| `optional` | Function |

## Errors

The error of this feature is `ian_framework::exception::Exception`. Its definition is in `spec/framework_rs/exception.md`.

| Code | Condition | Message |
| --- | --- | --- |
| `ENV_MISSING` | The variable is absent or blank. | `{name} is required` |
| `ENV_NOT_UNICODE` | The value of the variable is not valid unicode. | `{name} is not valid unicode` |

- Each read function makes its exceptions in its own body. The module has no public function that makes an exception.
- The exception holds the name of the variable. It does not hold the value. Thus the `Display` output and the `Debug` output do not show the value. The value can be a secret.

## required and optional

| Function | Signature |
| --- | --- |
| `required` | `pub fn required(name: &str) -> Result<String, Exception>` |
| `optional` | `pub fn optional(name: &str) -> Result<Option<String>, Exception>` |

- `required` uses the same lookup as `optional`. When `optional` gives `None`, `required` gives the code `ENV_MISSING`.

The functions read only the environment variable. They do not read Secret Manager. A project that wants a secret first reads it with `ian_framework::gcp::SecretManager` and then falls back to `optional` or `required`.

### Environment variable read

- The read removes the whitespace at the start and at the end of the value.
- An absent variable, an empty value or a value with only whitespace gives `None`.
- When the value is not valid unicode, the read gives the code `ENV_NOT_UNICODE`.
