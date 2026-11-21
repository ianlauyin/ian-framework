use std::path::{Path, PathBuf};

use ian_lint::file::rs::source::RsFile;

pub(crate) fn with_source<T>(
    path: &Path,
    source: &str,
    srcs: &[PathBuf],
    f: impl FnOnce(&RsFile) -> T,
) -> T {
    let ast = ian_lint::file::rs::source::parse(source);
    f(&RsFile::new(path, source, ast.as_ref(), srcs))
}
