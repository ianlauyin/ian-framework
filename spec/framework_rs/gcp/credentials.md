# Application Default Credentials and access tokens

The parts that apply to all of this feature are in `spec/framework_rs/gcp/context.md`.

## Credentials

`Credentials::from_adc` reads Application Default Credentials. It gives one of these credentials:

| Credentials | Source |
| --- | --- |
| `User` | The `authorized_user` file at `GOOGLE_APPLICATION_CREDENTIALS`, or else at the well-known gcloud path. |
| `Metadata` | The service account on the VM metadata server. The read uses this when there is no credentials file. |

- When `GOOGLE_APPLICATION_CREDENTIALS` is set and not empty, the read uses that file.
- Else the read uses `$HOME/.config/gcloud/application_default_credentials.json` or `%APPDATA%/gcloud/application_default_credentials.json` when the file exists.
- The read rejects a file that is larger than 1 MiB.
- The read rejects a service account JSON key. The error does not show the key.
- The read rejects impersonated application default credentials.
- The read rejects a different credentials type.
- `User` credentials need `client_id`, `client_secret` and `refresh_token`. A missing field is a `Config` error.
- `User` credentials keep the optional `quota_project_id` of the file. A blank value is not kept.
- `User` credentials with an empty `client_id`, `client_secret` or `refresh_token` are not valid.

The `Debug` output of the credentials does not show the client ID, the client secret or the refresh token.

## Access token

| Credentials | Token request |
| --- | --- |
| `Metadata` | `GET` to the metadata server token URL with the `cloud-platform` scope and the header `Metadata-Flavor: Google`. The time limit is 2 seconds. |
| `User` | `POST` to `https://oauth2.googleapis.com/token` with the refresh token grant. The time limit is 30 seconds. |

- A token response with a status that is not 2xx is an `Auth` error. The message has the status and the API error message.
- A token response that is not JSON, or that has an empty access token, is an `Auth` error.
- A user refresh token keeps the scopes from the original login.

Token expiry:

- The lifetime is `expires_in`. When the response has no `expires_in`, the lifetime is 3600 seconds.
- The maximum lifetime is 3600 seconds. A longer `expires_in` gives 3600 seconds. Thus the expiry calculation does not panic.
- The token expiry is the lifetime minus 60 seconds. When half of the lifetime is longer, the expiry is half of the lifetime. Thus a short lifetime does not cause a token request for each call.

## Token source

`TokenSource` keeps the credentials and one cached access token. It is private to the crate.

- `TokenSource::token` gives the cached token while the token has not expired. Else it gets a new token, keeps it and gives it.
- `TokenSource::invalidate` removes the cached token. The next `token` call gets a new token.
- The cache lock stays held during a token request. Thus concurrent calls send one token request.
- The caller gives the HTTP client to each `token` call.
- `Metadata` credentials use the metadata server token URL. `User` credentials use `https://oauth2.googleapis.com/token`. The tests can change the two URLs.
- The `Debug` output of a `TokenSource` shows the credentials. It does not show the token.

## Users

| User | Use |
| --- | --- |
| GCS client | `spec/framework_rs/gcp/storage/authentication.md`. The client adds the signer email and keeps one `TokenSource`. |
| Secret Manager access | `spec/framework_rs/gcp/secret-manager.md`. The access reads the credentials and the project one time for each process, and keeps one `TokenSource`. |
