//! Framework library.
//!
//! The default build has no optional features.
//! Enable the Google Cloud Storage client with the `gcs` feature:
//!
//! ```toml
//! ian_framework = { version = "0.1", features = ["gcs"] }
//! ```
//!
//! ```sh
//! cargo build --features gcs
//! ```

#![forbid(unsafe_code)]

#[cfg(feature = "gcs")]
pub mod gcs;
