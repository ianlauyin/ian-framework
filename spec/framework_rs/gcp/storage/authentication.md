# GCS authentication

The parts that apply to all of this feature are in `spec/framework_rs/gcp/storage/context.md`.

## Authentication

The client accepts only Application Default Credentials. The official crates read them. The order is in `spec/framework_rs/gcp/context.md`.

- The `Storage` client and the `StorageControl` client use the default credentials. A failure to build one is a `CLIENT_BUILD_FAILED` error.
- The client does not read a credentials file by its own code.

## URL signer

`GcsClient::connect_with_adc` builds one `Signer` of `google-cloud-auth`. The signer signs the URLs.

| `signer_email` | Signer |
| --- | --- |
| `None` | The signer of the default credentials. These must be a service account key file or the metadata server. A failure is a `URL_SIGNER_BUILD_FAILED` error. |
| `Some` | The default credentials impersonate that service account, and the signer signs through the IAM Credentials API. The caller needs the permission to sign as that account. |

- A signer email must have `@` and no whitespace. Another value is a `SIGNER_EMAIL_INVALID` error.
- The library does not read an env variable for the signer email. The project gives it.
- User application default credentials cannot sign. They need `Some` signer email.
- A failure to build the signer is a `URL_SIGNER_BUILD_FAILED` error.
- `GcsClient` has no `signer_email` method.
