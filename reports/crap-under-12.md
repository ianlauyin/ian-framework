# CRAP report for framework_rs

## Result

- Target: the CRAP score of each non-test function is under 12.
- Result: 0 of 495 functions have a score of 12 or more.
- `cargo fmt` shows no diff.
- `cargo clippy --all-targets` shows no warning, with default features and with `--all-features`.
- `cargo test --all-features` passes 342 tests. The first measurement had 266 tests.
- Nothing is committed. 25 files in `framework_rs/src/` changed (1994 lines added, 507 lines removed).

## Method

- CRAP formula: complexity squared, times (1 minus coverage) cubed, plus complexity.
- Coverage: `cargo llvm-cov --all-features`. Coverage of a function is the share of its regions that run. The count of a region is the highest count of all its generic instances.
- Complexity: `rust-code-analysis-cli`, cyclomatic metric per function.
- Test code is not measured.
- Start state: 44 functions had a score of 12 or more. The highest score was 196.7, in `put_chunk_retrying`.

## Cause

- Most of the GCS client had no test. The client used fixed Google URLs, so no test could reach the network code.
- Some functions had a complexity of 12 or more by themselves. Tests alone could not lower their score enough.

## Changes

### Test seam for the GCS client

- Added the `Endpoints` struct in `framework_rs/src/gcs/gcs_client/mod.rs`. It holds the API root, the upload root, the upload location prefix, the IAM root and the metadata token URL.
- `Endpoints::google()` gives the production values. Production behaviour is the same as before.
- The root constants moved to `framework_rs/src/gcs/mod.rs`.
- `metadata_token` in `framework_rs/src/gcp/credentials.rs` takes the token URL as an argument. `Auth::fetch_token` in `framework_rs/src/gcs/gcs_client/credentials/auth.rs` passes it.
- Tests start a local fake server (`serve_script`) and point a client to it with `fake_client`.

### Functions that were split

| File | Function | Result |
| --- | --- | --- |
| `framework_rs/src/logging/log_shipper.rs` | `LogShipper::new` (complexity 32) | Six small private functions |
| `framework_rs/src/gcs/gcs_client/mod.rs` | `send` (17) | A loop around `try_send`, plus three helpers |
| `framework_rs/src/gcs/gcs_client/upload/resumable_upload.rs` | `put_chunk_retrying` (22) | A loop around `try_chunk`, plus five helpers |
| `framework_rs/src/gcs/gcs_client/upload/upload_source.rs` | `read_chunk` (12) | `bytes_chunk` and `file_chunk` |
| `framework_rs/src/gcs/gcs_client/mod.rs` | `rewrite_object`, `move_object` | `rewrite_step`, `failed_move_error` |
| `framework_rs/src/gcs/gcs_client/credentials/signed_parts.rs` | `sign_blob` | `signature_from_response` |
| `framework_rs/src/gcp/secret_manager.rs` | `from_adc`, `shared`, `decode_access_response` | `resolve_project`, `initialized`, `status_error`, `decode_payload` |
| `framework_rs/src/gcp/credentials.rs` | `UserCredentials::fetch_token` | `fetch_token_at` |
| `framework_rs/src/http/middleware/require_api_key.rs` | `applies` (14) | `PathMatch::matches`, `without_trailing_slash` |
| `framework_rs/src/http/middleware/rate_limit.rs` | `check` (12) | `make_room` |
| `framework_rs/src/env/error.rs` | `Display::fmt` (11) | One `problem` string from one `match` |
| `framework_rs/src/monitor/cpu_monitor.rs` | `spawn` | A private `run` that takes a sampler closure |

### Tests that were added

- GCS client: `stat`, `list`, `put`, `delete`, rewrite, move, retry in `send`, status and error mapping.
- Resumable upload: resume, rewind, probe, retry.
- Other GCS code: `UploadSource`, `ObjectReader`, `sign_blob`, `metadata_email`, `content_type`, `rewrite_step`.
- Google credentials: ADC lookup, the well-known path, `read_adc_at`, `validate`, `token_from_response`, `fetch_token_at`.
- Secret manager: project lookup and response decoding.
- Others: `slide_session`, `cpu_monitor`, `auth` error source, `LogValue` from a float.
- The `store_or_skip` test helper moved to the `tests` module in `framework_rs/src/auth/mod.rs`. The `session_store.rs` tests and the `slide_session.rs` tests both use it.

### Earlier fixes in the same session

- `cargo fmt` diff in `framework_rs/src/logging/log_batch.rs`.
- Index access in `framework_rs/src/monitor/cpu_monitor.rs` and `framework_rs/src/gcp/error.rs` now uses `get`.

### Two tests that used the real metadata server

- The first version of two GCS tests called the real metadata server. They were not isolated and could take 2 seconds each when offline.
- They now use the local fake server. The rejected-token test checks the 401, the token fetch and the retry with the new token.

## Open points

- The `slide_session` tests skip themselves when Redis is not running. Coverage of that file is full only when Redis runs.
- One `cargo llvm-cov` run failed once and passed on the next run. The cause is not known. Two agents built in the same target folder at that time.
- Spec: no file in `spec/` names a changed internal. Behaviour and contracts did not change, so no spec update was necessary.
