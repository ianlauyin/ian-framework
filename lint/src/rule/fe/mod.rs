pub(crate) mod css;
pub(crate) mod folder;
pub(crate) mod ts;

use std::fs;
use std::path::{Path, PathBuf};

use oxc_allocator::Allocator;

use crate::config::Ruleset;
use crate::diagnostic::Diagnostic;
use crate::files::FileSet;
use crate::rule::fe::css::file::CssFile;
use crate::rule::fe::ts::file::TsFile;
use crate::rule::ruleset::{TsRule, rules};

pub(crate) fn check(path: &Path, ruleset: Ruleset) -> Vec<Diagnostic> {
    let rules = rules(ruleset);
    if crate::files::file_name(path).ends_with(".css") {
        let Some(source) = crate::rule::fe::css::parse::read(path) else {
            return Vec::new();
        };
        let items = crate::rule::fe::css::parse::parse(&source);
        let file = CssFile::new(path, &source, &items);
        let mut diagnostics = Vec::new();
        for check in rules.css {
            diagnostics.extend(check(&file));
        }
        diagnostics
    } else {
        let Ok(source) = fs::read_to_string(path) else {
            return Vec::new();
        };
        let allocator = Allocator::default();
        let program = crate::rule::fe::ts::file::parse(&allocator, path, &source);
        let file = TsFile::new(path, &source, program.as_ref());
        let mut diagnostics = Vec::new();
        for rule in rules.ts {
            let TsRule::File(check) = rule else {
                continue;
            };
            diagnostics.extend(check(&file));
        }
        diagnostics
    }
}

pub(crate) fn check_folder(
    path: &Path,
    files: &FileSet,
    srcs: &[PathBuf],
    ruleset: Ruleset,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for rule in rules(ruleset).ts {
        let TsRule::Folder(check) = rule else {
            continue;
        };
        diagnostics.extend(check(path, files, srcs));
    }
    diagnostics
}
