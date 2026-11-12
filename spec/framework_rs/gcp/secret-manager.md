# Secret Manager secret access

The parts that apply to all of this feature are in `spec/framework_rs/gcp/context.md`.

## Shared state

`SecretManager` is public. Its fields are private. One process has one shared `SecretManager` state in a Tokio `OnceCell`. The first `SecretManager::shared` call makes the state. Each later call uses the same state.

The state is one of these:

| State | Condition |
| --- | --- |
| No project | The process has no project id. `shared` gives `None`. |
| Ready | The project id and one `SecretManagerService` client of `google-cloud-secretmanager-v1`. |

The steps to make the state are:

1. Find the project. The rules are in `spec/framework_rs/gcp/context.md`. When there is no project, the state is "no project", and the client is not built.
2. Build the client with the default builder. The builder reads Application Default Credentials. A failure is a `GCP_AUTH` error.

- An error does not go into the `OnceCell`. Thus the next call tries again.
- The `OnceCell` is not in the public API.

## Access

`SecretManager::access_latest(secret)` reads the latest version of one secret. It gives `None` when the secret or its latest version does not exist.

- `secret` is the secret id. It is not empty. It has only ASCII letters, digits, `-`, `_` and `.`. A different id is a `GCP_CONFIG` error. The check occurs before the request.
- The request is `AccessSecretVersion` for the name `projects/{project}/secrets/{secret}/versions/latest`.
- The official client does the retries and the token refresh.

## Response

| Response | Result |
| --- | --- |
| Not found (`GCS_NOT_FOUND`) | `None` |
| Another failure | An error with the code from `spec/framework_rs/gcp/context.md` and the message `secret manager access failed` |
| Payload that is not UTF-8 | `GCP_CONFIG` error |
| Payload that is empty after the removal of the whitespace | `GCP_CONFIG` error |
| Any other payload | The value, with the whitespace at the start and at the end removed |

- No error holds the secret value.
- No error holds a UTF-8 parser error, because this error can quote the value.
- The access does not write a log line.
