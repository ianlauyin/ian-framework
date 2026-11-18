//! The frontend rules.
//! `check_files` in `lint/src/lib.rs` calls `check` for a `.ts`, `.tsx`, or `.css` file under `src`.
//! To add a rule: declare its module in this file, then call it from `check`.

use std::path::Path;

use crate::diagnostic::Diagnostic;
use crate::files::FileSet;

#[allow(
    clippy::missing_const_for_fn,
    reason = "A rule that is added later reads the file."
)]
pub fn check(_path: &Path, _files: &FileSet) -> Vec<Diagnostic> {
    Vec::new()
}
