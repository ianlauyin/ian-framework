# Read functions and the error type

The parts that apply to all of this feature are in `spec/framework_rs/env/context.md`.

## Public items

`ian_framework::env` exports these items:

| Item | Kind |
| --- | --- |
| `missing` | Function |
| `invalid` | Function |
| `required` | Async function |
| `optional` | Async function |
| `u64_or` | Function |

## Errors

The error of this feature is `ian_framework::exception::Exception`. Its definition is in `spec/framework_rs/exception.md`.

| Code | Condition | Message |
| --- | --- | --- |
| `env_missing` | The variable is absent or blank. | `{name} is required` |
| `env_not_unicode` | The value of the variable is not valid unicode. | `{name} is not valid unicode` |
| `env_invalid` | The value of the variable is not valid for the read. | `{name} is not valid` |
| `env_secret` | The Secret Manager read for the variable fails. | `{name} cannot be read from Secret Manager` |

Only the `gcp` feature gives `env_secret`. Its source is the Secret Manager exception, so the trace shows the cause.

| Function | Signature | Effect |
| --- | --- | --- |
| `missing` | `fn missing(name: &str) -> Exception` | Makes the exception with the code `env_missing`. |
| `invalid` | `fn invalid(name: &str) -> Exception` | Makes the exception with the code `env_invalid`. |

- A project uses `missing` when it reads a value with `std::env::var` and the value is not set.
- A project uses `invalid` when its own check of a value fails.
- The exception holds the name of the variable. It does not hold the value. Thus the `Display` output and the `Debug` output do not show the value. The value can be a secret.

## required and optional

| Function | Signature |
| --- | --- |
| `required` | `pub async fn required(name: &str) -> Result<String, Exception>` |
| `optional` | `pub async fn optional(name: &str) -> Result<Option<String>, Exception>` |

- The signatures are the same with and without the `gcp` feature.
- The returned futures are `Send`.
- `required` uses the same lookup as `optional`. When `optional` gives `None`, `required` gives the code `env_missing`.

### Lookup

With the `gcp` feature, the lookup reads Secret Manager first. Then it reads the environment variable.

1. Get the shared Secret Manager state. The rules are in `spec/framework_rs/gcp/secret-manager.md`.
2. When the state is "no credentials", go to step 4.
3. Read the secret `projects/{project}/secrets/{name}/versions/latest`. The secret id is `name` exactly. When the secret or its latest version does not exist (status 404), go to step 4. Else give the secret value.
4. Read the environment variable `name`.

Without the `gcp` feature, the lookup does only step 4.

| Condition | Result |
| --- | --- |
| No `gcp` feature | The environment variable |
| No credentials: no credentials file and no metadata server | The environment variable |
| `User` credentials and no project | The environment variable |
| Secret Manager status 404 | The environment variable |
| Secret Manager gives the secret | The secret value. The lookup does not read the environment variable. |
| A different failure: credentials file not valid, token failure, metadata server status that is not 2xx, status 401, 403 or a different status, network error, bad payload, empty or non-UTF-8 value, secret id with a character that is not valid | Exception with the code `env_secret` for the name `name`. The lookup does not read the environment variable. |

- The error detail is the Secret Manager error message and its source.
- The lookup does not write a log line.
- The errors do not hold the secret value or the value of the environment variable.

### Environment variable read

- The read removes the whitespace at the start and at the end of the value.
- An absent variable, an empty value or a value with only whitespace gives `None`.
- When the value is not valid unicode, the read gives the code `env_not_unicode`.

## u64_or

`pub fn u64_or(name: &str, default: u64) -> Result<u64, Exception>` reads one optional `u64` variable.

- `u64_or` is not async. It reads only the environment variable. It does not read Secret Manager.

- When the variable is absent, `u64_or` gives `default`.
- `u64_or` parses the value with `str::parse::<u64>`. It does not remove whitespace.
- When the parse fails, `u64_or` gives the code `env_invalid`. Thus an empty value, a value with whitespace, a negative value and a value larger than `u64::MAX` give `env_invalid`.
- When the value is not valid unicode, `u64_or` gives the code `env_not_unicode`.
