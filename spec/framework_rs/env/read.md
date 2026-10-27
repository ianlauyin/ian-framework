# Read functions and the error type

The parts that apply to all of this feature are in `spec/framework_rs/env/context.md`.

## Public items

`ian_framework::env` exports these items:

| Item | Kind |
| --- | --- |
| `EnvError` | Struct |
| `EnvErrorKind` | Enum |
| `required` | Async function |
| `optional` | Async function |
| `u64_or` | Function |

## EnvErrorKind

`EnvErrorKind` implements `Debug`, `Clone`, `Copy`, `PartialEq` and `Eq`.

| Variant | Condition |
| --- | --- |
| `Missing` | The variable is absent or blank. |
| `NotUnicode` | The value of the variable is not valid unicode. |
| `Invalid` | The value of the variable is not valid for the read. |
| `Secret` | The Secret Manager read for the variable fails. |

The build always has all the variants. Only the `gcp` feature gives `Secret`.

## EnvError

`EnvError` implements `Debug`, `Clone`, `PartialEq`, `Eq`, `Display` and `std::error::Error`.

| Method | Signature | Effect |
| --- | --- | --- |
| `EnvError::invalid` | `fn invalid(name: &str) -> Self` | Makes one error of the kind `Invalid` for the variable `name`. |
| `kind` | `fn kind(&self) -> EnvErrorKind` | Gives the kind of the error. |
| `name` | `fn name(&self) -> &str` | Gives the name of the variable. |

- A project uses `EnvError::invalid` when its own check of a value fails.
- `EnvError` holds the kind, the name of the variable and an optional detail.
- Only the kind `Secret` has a detail. The detail is the Secret Manager read error. It does not hold the secret value.
- `EnvError` does not hold the value of the variable. Thus the `Display` output and the `Debug` output do not show the value. The value can be a secret.
- `source` gives `None`.

`Display` gives one message for each kind:

| Kind | Message |
| --- | --- |
| `Missing` | `{name} is required` |
| `NotUnicode` | `{name} is not valid unicode` |
| `Invalid` | `{name} is not valid` |
| `Secret` | `{name} cannot be read from Secret Manager` |

When the error has a detail, `Display` adds `: {detail}` to the message.

## required and optional

| Function | Signature |
| --- | --- |
| `required` | `pub async fn required(name: &str) -> Result<String, EnvError>` |
| `optional` | `pub async fn optional(name: &str) -> Result<Option<String>, EnvError>` |

- The signatures are the same with and without the `gcp` feature.
- The returned futures are `Send`.
- `required` uses the same lookup as `optional`. When `optional` gives `None`, `required` gives the kind `Missing`.

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
| A different failure: credentials file not valid, token failure, metadata server status that is not 2xx, status 401, 403 or a different status, network error, bad payload, empty or non-UTF-8 value, secret id with a character that is not valid | Error of the kind `Secret` with the name `name`. The lookup does not read the environment variable. |

- The error detail is the Secret Manager error message and its source.
- The lookup does not write a log line.
- The errors do not hold the secret value or the value of the environment variable.

### Environment variable read

- The read removes the whitespace at the start and at the end of the value.
- An absent variable, an empty value or a value with only whitespace gives `None`.
- When the value is not valid unicode, the read gives the kind `NotUnicode`.

## u64_or

`pub fn u64_or(name: &str, default: u64) -> Result<u64, EnvError>` reads one optional `u64` variable.

- `u64_or` is not async. It reads only the environment variable. It does not read Secret Manager.

- When the variable is absent, `u64_or` gives `default`.
- `u64_or` parses the value with `str::parse::<u64>`. It does not remove whitespace.
- When the parse fails, `u64_or` gives the kind `Invalid`. Thus an empty value, a value with whitespace, a negative value and a value larger than `u64::MAX` give `Invalid`.
- When the value is not valid unicode, `u64_or` gives the kind `NotUnicode`.
