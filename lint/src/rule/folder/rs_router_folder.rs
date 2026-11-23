// The file `{name}` is directly in `router/api`.
// Move the file to the folder of its feature.
// The folder `{name}` is an access tier.
// Give the folder the name of its feature, and put the access tier in the `mod.rs` of the feature.
// Why a lint rule: Each feature of the API is one folder, so a new access tier or a new permission does not move a file.

use std::path::{Path, PathBuf};

use crate::config::Side;
use crate::files::{FileSet, file_name, without_dot};
use crate::rule::diagnostic::Diagnostic;
use crate::rule::folder::FolderRule;

const RULE: &str = "router-folder";

const TIERS: [&str; 2] = ["public", "protected"];

pub(crate) const FOLDER_RULE: FolderRule = FolderRule::new(Side::Be, check);

pub fn check(path: &Path, files: &FileSet, srcs: &[PathBuf]) -> Vec<Diagnostic> {
    let folder = without_dot(path);
    let Some(api) = srcs
        .iter()
        .map(|src| without_dot(src).join("router").join("api"))
        .find(|api| folder.starts_with(api))
    else {
        return Vec::new();
    };
    if folder != api {
        return tier_folder(path);
    }
    files
        .files()
        .filter(|file| file.parent().map(without_dot).as_ref() == Some(&api))
        .filter(|file| file_name(file) != "mod.rs")
        .map(|file| {
            Diagnostic::new(
                file,
                1,
                1,
                RULE,
                format!(
                    "The file `{}` is directly in `router/api`.",
                    file_name(file)
                ),
                "Move the file to the folder of its feature.",
            )
        })
        .collect()
}

fn tier_folder(path: &Path) -> Vec<Diagnostic> {
    let name = file_name(path);
    if !TIERS.contains(&name) {
        return Vec::new();
    }
    vec![Diagnostic::new(
        path,
        1,
        1,
        RULE,
        format!("The folder `{name}` is an access tier."),
        "Give the folder the name of its feature, and put the access tier in the `mod.rs` of the feature.",
    )]
}
