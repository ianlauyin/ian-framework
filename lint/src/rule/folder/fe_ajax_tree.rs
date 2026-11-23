// The ajax file has no handler file `{path}`.
// Give the ajax file the same path as the handler file.
// Why a lint rule: The ajax tree matches the router tree, so the path of a call gives its handler.

use std::path::{Path, PathBuf};

use crate::config::Side;
use crate::files::{FileSet, file_name, without_dot};
use crate::rule::diagnostic::Diagnostic;
use crate::rule::folder::FolderRule;

const RULE: &str = "ajax-tree";

pub(crate) const FOLDER_RULE: FolderRule = FolderRule::new(Side::All, check);

pub fn check(path: &Path, files: &FileSet, srcs: &[PathBuf]) -> Vec<Diagnostic> {
    let folder = without_dot(path);
    if !srcs
        .iter()
        .any(|src| without_dot(src).join("util").join("ajax").join("api") == folder)
    {
        return Vec::new();
    }
    let routers: Vec<_> = files
        .dirs()
        .filter(|dir| {
            let dir = without_dot(dir);
            srcs.iter()
                .any(|src| without_dot(src).join("router").join("api") == dir)
        })
        .collect();
    let Some(first) = routers.first() else {
        return Vec::new();
    };
    let mut diagnostics = Vec::new();
    for file in files.files() {
        let Ok(relative) = file.strip_prefix(path) else {
            continue;
        };
        if !file_name(file).ends_with(".ts") {
            continue;
        }
        let handler = relative.with_extension("rs");
        if file_name(&handler) != "mod.rs"
            && routers
                .iter()
                .any(|router| files.has_file(&router.join(&handler)))
        {
            continue;
        }
        diagnostics.push(Diagnostic::new(
            file,
            1,
            1,
            RULE,
            format!(
                "The ajax file has no handler file `{}`.",
                first.join(&handler).display()
            ),
            "Give the ajax file the same path as the handler file.",
        ));
    }
    diagnostics
}
