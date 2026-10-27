# Secret Manager secret access

The parts that apply to all of this feature are in `spec/framework_rs/gcp/context.md`.

## Shared state

`SecretManager` is private to the crate. One process has one shared `SecretManager` state in a Tokio `OnceCell`. The first `SecretManager::shared` call makes the state. Each later call uses the same state.

The state is one of these:

| State | Condition |
| --- | --- |
| No credentials | There is no credentials file and the metadata server cannot be reached. Or the `User` credentials have no project. |
| Ready | The credentials, the project and one access token. |

The steps to make the state are:

1. Read the credentials with `Credentials::from_adc`. The rules are in `spec/framework_rs/gcp/credentials.md`. A credentials file that is not valid is an error.
2. Find the project. The rules are in the table below.
3. When there is no project, the state is "no credentials".
4. Get one access token. A token failure is an error.

| Credentials | Project |
| --- | --- |
| `Metadata` | `GET http://169.254.169.254/computeMetadata/v1/project/project-id` with the header `Metadata-Flavor: Google`. The time limit is 2 seconds. When the request cannot be sent, there are no credentials. |
| `User` | The environment variable `GOOGLE_CLOUD_PROJECT` when it is not blank. Else the `quota_project_id` of the credentials. Else there is no project. |

- A metadata project status that is not 2xx is an `Auth` error.
- A project id that is empty or has a character that is not an ASCII letter, a digit, `-`, `_` or `.` is a `Config` error.
- The whitespace at the start and at the end of the project id is removed.
- An error does not go into the `OnceCell`. Thus the next call tries again.
- When the token expires, the next access gets a new token and keeps it.
- The `OnceCell` is not in the public API.

## Access

`SecretManager::access_latest(secret)` reads the latest version of one secret. It gives `None` when the secret or its latest version does not exist.

- `secret` is the secret id. It is not empty. It has only ASCII letters, digits, `-`, `_` and `.`. A different id is a `Config` error. The check occurs before the request.
- The request is `GET https://secretmanager.googleapis.com/v1/projects/{project}/secrets/{secret}/versions/latest:access` with `Authorization: Bearer {token}`.

Request rules:

- When the `User` credentials have a quota project, the request has the header `x-goog-user-project: {quota project}`.
- The request time limit is 30 seconds.
- The HTTP client uses the shared settings in `spec/framework_rs/gcp/context.md`. Thus the connect time limit is 10 seconds, and the client does not follow redirects.
- A transport failure is a `Transport` error. The error source does not have the URL.

## Response

| Response | Result |
| --- | --- |
| Status 404 | `None` |
| Status 401 or 403 | `Auth` error with the message `secret manager status {status}: {API error message}` |
| A different status that is not 2xx and not 404 | `Api` error with the same message |
| Body that is not JSON or has no string `payload.data` | `Api` error |
| `payload.data` that is not standard base64 | `Api` error |
| Decoded value that is not UTF-8 | `Config` error |
| Decoded value that is empty after the removal of the whitespace | `Config` error |
| A different 2xx response | The decoded value, with the whitespace at the start and at the end removed |

- The API error message is the `error.message` field of the body. When the body has no such field, the message is empty. The message is cut to 300 bytes.
- No error holds the secret value.
- No error holds a JSON, base64 or UTF-8 parser error, because these errors can quote the value.
- The access does not write a log line.
