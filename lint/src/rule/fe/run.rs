use std::fs;
use std::path::{Path, PathBuf};

use oxc_allocator::Allocator;

use crate::Failure;
use crate::config::Ruleset;
use crate::files::FileSet;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::fe::css::file::CssFile;
use crate::rule::fe::ts::file::TsFile;
use crate::rule::fe::ts::rule::TsRule;
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
        let program = crate::rule::fe::ts::file::parse(&allocator, path, &source);
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

pub(crate) fn rewrite(path: &Path, ruleset: Ruleset) -> Vec<Failure> {
    let Ok(original) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let rules = rules(ruleset);
    let mut source = original.clone();
    if crate::files::file_name(path).ends_with(".css") {
        for rule in rules.css {
            let Some(edit) = rule.edit else {
                continue;
            };
            let items = crate::rule::fe::css::parse::parse(&source);
            let file = CssFile::new(path, &source, &items);
            match crate::rule::edit::replace(&source, &edit(&file)) {
                Ok(next) => source = next,
                Err(error) => return vec![crate::rule::edit::failure(path, error)],
            }
        }
    } else {
        for rule in rules.ts {
            let TsRule::File(rule) = rule else {
                continue;
            };
            let Some(edit) = rule.edit else {
                continue;
            };
            let allocator = Allocator::default();
            let program = crate::rule::fe::ts::file::parse(&allocator, path, &source);
            let file = TsFile::new(path, &source, program.as_ref());
            match crate::rule::edit::replace(&source, &edit(&file)) {
                Ok(next) => source = next,
                Err(error) => return vec![crate::rule::edit::failure(path, error)],
            }
        }
    }
    crate::rule::edit::commit(path, &original, &source)
}

pub(crate) fn rewrite_folder(
    path: &Path,
    files: &FileSet,
    srcs: &[PathBuf],
    ruleset: Ruleset,
) -> Vec<Failure> {
    let mut failures = Vec::new();
    for rule in rules(ruleset).ts {
        let TsRule::Folder(rule) = rule else {
            continue;
        };
        let Some(edit) = rule.edit else {
            continue;
        };
        failures.extend(crate::rule::edit::write(&edit(path, files, srcs)));
    }
    failures
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
