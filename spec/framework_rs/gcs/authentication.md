# GCS authentication

The parts that apply to all of this feature are in `spec/framework_rs/gcs/context.md`.

## Authentication

The client accepts only Application Default Credentials. The shared credentials code reads them and gets the access tokens. Its rules are in `spec/framework_rs/gcp/credentials.md`.

- If there is no credentials file, metadata auth uses the service account on the VM metadata server.
- The client rejects a service account JSON key.
- The client rejects impersonated application default credentials.
- User application default credentials need `GCS_SIGNER_EMAIL`. The client adds this check to the shared credentials read.
- The client does not send the quota project of user credentials.
- `GCS_SIGNER_EMAIL` is the service account email that signs URLs.

Scopes:

- The metadata token requests the `cloud-platform` scope.
- That scope gives access to object calls and to `signBlob`.
- A user refresh token keeps the scopes from the original login.
- The command `gcloud auth application-default login` includes the `cloud-platform` scope.
