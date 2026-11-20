use std::path::Path;

use crate::rule::fe::css::parse::Item;

pub(crate) struct CssFile<'a> {
    pub(crate) path: &'a Path,
    pub(crate) source: &'a str,
    pub(crate) items: &'a [Item],
}

impl<'a> CssFile<'a> {
    pub(crate) fn new(path: &'a Path, source: &'a str, items: &'a [Item]) -> Self {
        Self {
            path,
            source,
            items,
        }
    }
}

#[cfg(test)]
pub(crate) fn with_source<T>(path: &Path, source: &str, f: impl FnOnce(&CssFile) -> T) -> T {
    let items = crate::rule::fe::css::parse::parse(source);
    f(&CssFile::new(path, source, &items))
}
