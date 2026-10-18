# ian_framework

This repository is the framework. A project contains the business logic.

A project copies the files in `template/`. The framework rules are in `spec/00_structure.md`.

## Rust library

The library crate is `crates/ian_framework`. The package name is `ian_framework`.

Optional behaviour is a Cargo feature. The default feature set is empty. The default build does not compile the Google Cloud Storage client.

Enable the client with the feature `gcs`:

```toml
[dependencies]
ian_framework = { version = "0.1", features = ["gcs"] }
```

`rust-toolchain.toml` selects the Rust version for this repository.

From this repository:

```sh
cargo check
cargo check --features gcs
cargo test --features gcs
```

A project can also pass `--features gcs` to its own build when this crate is a path dependency, or select the dependency feature `ian_framework/gcs`.

### GCS client

`ian_framework::gcs` talks to the Cloud Storage JSON API and signs V4 `GET` URLs with the IAM `signBlob` method. It does not include product rules (trash, browse, upload caps, or an HTTP framework).

The calls below belong in an async function:

```rust
use std::time::Duration;

use ian_framework::gcs::{self, ListQuery, PutOptions};

let backend = gcs::gcs_from_adc("my-bucket").await?;
gcs::put_bytes(&backend, "folder/file.txt", b"hello", &PutOptions::default()).await?;
let bytes = gcs::get_bytes(&backend, "folder/file.txt").await?;
let page = gcs::list(&backend, &ListQuery {
    prefix: Some("folder/".to_string()),
    delimiter: Some("/".to_string()),
    ..ListQuery::default()
}).await?;
let url = gcs::signed_get_url(&backend, "folder/file.txt", Duration::from_secs(300)).await?;
let _ = (bytes, page, url);
```

Constructors:

- `gcs_from_adc(bucket)` reads Application Default Credentials.
- `gcs_backend(bucket, auth)` takes an explicit `gcs::Auth`.

Authentication:

- On a production VM, use the metadata server service account (`Auth::Metadata`).
- Service account JSON keys are rejected.
- Impersonated application-default credentials are rejected.
- User application-default credentials (`authorized_user`) need the environment variable `GCS_SIGNER_EMAIL`. That value is the service account email that `signBlob` uses.

The metadata-server token requests the `cloud-platform` scope so the same token can call Cloud Storage and `signBlob`. A user refresh token keeps the scopes from the original login. `gcloud auth application-default login` includes `cloud-platform`.

Custom metadata is a map on `PutOptions`. This crate does not set `uploaded-by` or `uploaded-by-id`. A project that wants those keys puts them in the map.

Object results are `name`, `size`, `content_type`, `updated`, and the raw metadata map. List results also include `prefixes` and `next_page_token`. This crate does not hide prefixes and it does not apply a list cap.
