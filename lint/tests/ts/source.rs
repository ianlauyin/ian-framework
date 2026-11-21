use std::path::Path;

use oxc_allocator::Allocator;

use ian_lint::file::ts::source::TsFile;

pub(crate) fn with_source<T>(path: &Path, source: &str, f: impl FnOnce(&TsFile) -> T) -> T {
    let allocator = Allocator::default();
    let program = ian_lint::file::ts::source::parse(&allocator, path, source);
    f(&TsFile::new(path, source, program.as_ref()))
}
