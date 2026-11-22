use std::path::{Path, PathBuf};

use oxc_allocator::Allocator;

use ian_lint::file::ts::source::TsFile;

pub(crate) fn with_source<T>(path: &Path, source: &str, f: impl FnOnce(&TsFile) -> T) -> T {
    with_srcs(path, source, &[], f)
}

pub(crate) fn with_srcs<T>(
    path: &Path,
    source: &str,
    srcs: &[PathBuf],
    f: impl FnOnce(&TsFile) -> T,
) -> T {
    let allocator = Allocator::default();
    let program = ian_lint::file::ts::source::parse(&allocator, path, source);
    f(&TsFile::new(path, source, program.as_ref(), srcs))
}
