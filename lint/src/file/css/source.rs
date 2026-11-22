use std::path::{Path, PathBuf};

use crate::file::css::parse::Item;

pub struct CssFile<'a> {
    pub(crate) path: &'a Path,
    pub(crate) source: &'a str,
    pub(crate) items: &'a [Item],
    pub(crate) srcs: &'a [PathBuf],
}

impl<'a> CssFile<'a> {
    pub fn new(path: &'a Path, source: &'a str, items: &'a [Item], srcs: &'a [PathBuf]) -> Self {
        Self {
            path,
            source,
            items,
            srcs,
        }
    }
}
