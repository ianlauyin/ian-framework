use std::path::{Path, PathBuf};

use ian_lint::file::css::source::CssFile;

pub(crate) fn with_source<T>(path: &Path, source: &str, f: impl FnOnce(&CssFile) -> T) -> T {
    with_srcs(path, source, &[], f)
}

pub(crate) fn with_srcs<T>(
    path: &Path,
    source: &str,
    srcs: &[PathBuf],
    f: impl FnOnce(&CssFile) -> T,
) -> T {
    let items = ian_lint::file::css::parse::parse(source);
    f(&CssFile::new(path, source, &items, srcs))
}
