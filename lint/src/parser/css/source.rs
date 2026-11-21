use std::path::Path;

use crate::parser::css::parse::Item;

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
