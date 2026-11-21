use std::path::Path;

use crate::file::css::parse::Item;

pub struct CssFile<'a> {
    pub(crate) path: &'a Path,
    pub(crate) source: &'a str,
    pub(crate) items: &'a [Item],
}

impl<'a> CssFile<'a> {
    pub fn new(path: &'a Path, source: &'a str, items: &'a [Item]) -> Self {
        Self {
            path,
            source,
            items,
        }
    }
}
