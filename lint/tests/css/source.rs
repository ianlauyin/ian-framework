use std::path::Path;

use ian_lint::file::css::source::CssFile;

pub(crate) fn with_source<T>(path: &Path, source: &str, f: impl FnOnce(&CssFile) -> T) -> T {
    let items = ian_lint::file::css::parse::parse(source);
    f(&CssFile::new(path, source, &items))
}
