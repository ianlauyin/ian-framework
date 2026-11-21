use std::path::{Path, PathBuf};

pub struct RsFile<'a> {
    pub(crate) path: &'a Path,
    pub(crate) source: &'a str,
    pub(crate) ast: Option<&'a syn::File>,
    pub(crate) srcs: &'a [PathBuf],
}

impl<'a> RsFile<'a> {
    pub fn new(
        path: &'a Path,
        source: &'a str,
        ast: Option<&'a syn::File>,
        srcs: &'a [PathBuf],
    ) -> Self {
        Self {
            path,
            source,
            ast,
            srcs,
        }
    }
}

pub fn parse(source: &str) -> Option<syn::File> {
    syn::parse_file(source).ok()
}
