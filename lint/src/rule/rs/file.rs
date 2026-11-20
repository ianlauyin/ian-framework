use std::path::{Path, PathBuf};

pub(crate) struct RsFile<'a> {
    pub(crate) path: &'a Path,
    pub(crate) source: &'a str,
    pub(crate) ast: Option<&'a syn::File>,
    pub(crate) srcs: &'a [PathBuf],
}

impl<'a> RsFile<'a> {
    pub(crate) fn new(
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

pub(crate) fn parse(source: &str) -> Option<syn::File> {
    syn::parse_file(source).ok()
}

#[cfg(test)]
pub(crate) fn with_source<T>(
    path: &Path,
    source: &str,
    srcs: &[PathBuf],
    f: impl FnOnce(&RsFile) -> T,
) -> T {
    let ast = parse(source);
    f(&RsFile::new(path, source, ast.as_ref(), srcs))
}
