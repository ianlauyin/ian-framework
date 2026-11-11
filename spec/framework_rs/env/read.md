# Read functions and the error type

The parts that apply to all of this feature are in `spec/framework_rs/env/context.md`.

## Public items

`ian_framework::env` exports these items:

| Item | Kind |
| --- | --- |
| `required` | Async function |
| `optional` | Async function |

## Errors

The error of this feature is `ian_framework::exception::Exception`. Its definition is in `spec/framework_rs/exception.md`.

| Code | Condition | Message |
| --- | --- | --- |
| `ENV_MISSING` | The variable is absent or blank. | `{name} is required` |
| `ENV_NOT_UNICODE` | The value of the variable is not valid unicode. | `{name} is not valid unicode` |
| `ENV_SECRET` | The Secret Manager read for the variable fails. | `{name} cannot be read from Secret Manager` |

Only the `gcp` feature gives `ENV_SECRET`. Its source is the Secret Manager exception, so the trace shows the cause.

- Each read function makes its exceptions in its own body. The module has no public function that makes an exception.
- The exception holds the name of the variable. It does not hold the value. Thus the `Display` output and the `Debug` output do not show the value. The value can be a secret.

## required and optional

| Function | Signature |
| --- | --- |
| `required` | `pub async fn required(name: &str) -> Result<String, Exception>` |
| `optional` | `pub async fn optional(name: &str) -> Result<Option<String>, Exception>` |

- The signatures are the same with and without the `gcp` feature.
- The returned futures are `Send`.
- `required` uses the same lookup as `optional`. When `optional` gives `None`, `required` gives the code `ENV_MISSING`.

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
| A different failure: credentials file not valid, token failure, metadata server status that is not 2xx, status 401, 403 or a different status, network error, bad payload, empty or non-UTF-8 value, secret id with a character that is not valid | Exception with the code `ENV_SECRET` for the name `name`. The lookup does not read the environment variable. |

- The error detail is the Secret Manager error message and its source.
- The lookup does not write a log line.
- The errors do not hold the secret value or the value of the environment variable.

### Environment variable read

- The read removes the whitespace at the start and at the end of the value.
- An absent variable, an empty value or a value with only whitespace gives `None`.
- When the value is not valid unicode, the read gives the code `ENV_NOT_UNICODE`.
