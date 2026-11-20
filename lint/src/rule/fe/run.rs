use std::fs;
use std::path::{Path, PathBuf};

use oxc_allocator::Allocator;

use crate::config::Ruleset;
use crate::files::FileSet;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::fe::css::source::CssFile;
use crate::rule::fe::ts::rule::TsRule;
use crate::rule::fe::ts::source::TsFile;
use crate::ruleset::rules;

pub(crate) fn check(path: &Path, ruleset: Ruleset) -> Vec<Diagnostic> {
    let rules = rules(ruleset);
    if crate::files::file_name(path).ends_with(".css") {
        let Some(source) = crate::rule::fe::css::parse::read(path) else {
            return Vec::new();
        };
        let items = crate::rule::fe::css::parse::parse(&source);
        let file = CssFile::new(path, &source, &items);
        let mut diagnostics = Vec::new();
        for rule in rules.css {
            diagnostics.extend((rule.check)(&file));
        }
        diagnostics
    } else {
        let Ok(source) = fs::read_to_string(path) else {
            return Vec::new();
        };
        let allocator = Allocator::default();
        let program = crate::rule::fe::ts::source::parse(&allocator, path, &source);
        let file = TsFile::new(path, &source, program.as_ref());
        let mut diagnostics = Vec::new();
        for rule in rules.ts {
            let TsRule::File(rule) = rule else {
                continue;
            };
            diagnostics.extend((rule.check)(&file));
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
        let TsRule::Folder(rule) = rule else {
            continue;
        };
        diagnostics.extend((rule.check)(path, files, srcs));
    }
    diagnostics
}
