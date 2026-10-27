# Read functions and the error type

The parts that apply to all of this feature are in `spec/framework_rs/env/context.md`.

## Public items

`ian_framework::env` exports these items:

| Item | Kind |
| --- | --- |
| `EnvError` | Struct |
| `EnvErrorKind` | Enum |
| `required` | Function |
| `u64_or` | Function |

## EnvErrorKind

`EnvErrorKind` implements `Debug`, `Clone`, `Copy`, `PartialEq` and `Eq`.

| Variant | Condition |
| --- | --- |
| `Missing` | The variable is absent or blank. |
| `NotUnicode` | The value of the variable is not valid unicode. |
| `Invalid` | The value of the variable is not valid for the read. |

## EnvError

`EnvError` implements `Debug`, `Clone`, `PartialEq`, `Eq`, `Display` and `std::error::Error`.

| Method | Signature | Effect |
| --- | --- | --- |
| `EnvError::invalid` | `fn invalid(name: &str) -> Self` | Makes one error of the kind `Invalid` for the variable `name`. |
| `kind` | `fn kind(&self) -> EnvErrorKind` | Gives the kind of the error. |
| `name` | `fn name(&self) -> &str` | Gives the name of the variable. |

- A project uses `EnvError::invalid` when its own check of a value fails.
- `EnvError` holds only the kind and the name of the variable.
- `EnvError` does not hold the value of the variable. Thus the `Display` output and the `Debug` output do not show the value. The value can be a secret.
- `source` gives `None`.

`Display` gives one message for each kind:

| Kind | Message |
| --- | --- |
| `Missing` | `{name} is required` |
| `NotUnicode` | `{name} is not valid unicode` |
| `Invalid` | `{name} is not valid` |

## required

`required(name: &str) -> Result<String, EnvError>` reads one variable that must have a value.

- `required` removes the whitespace at the start and at the end of the value. Then it gives the result.
- When the variable is absent, `required` gives the kind `Missing`.
- When the value has only whitespace or is empty, `required` gives the kind `Missing`.
- When the value is not valid unicode, `required` gives the kind `NotUnicode`.

## u64_or

`u64_or(name: &str, default: u64) -> Result<u64, EnvError>` reads one optional `u64` variable.

- When the variable is absent, `u64_or` gives `default`.
- `u64_or` parses the value with `str::parse::<u64>`. It does not remove whitespace.
- When the parse fails, `u64_or` gives the kind `Invalid`. Thus an empty value, a value with whitespace, a negative value and a value larger than `u64::MAX` give `Invalid`.
- When the value is not valid unicode, `u64_or` gives the kind `NotUnicode`.
