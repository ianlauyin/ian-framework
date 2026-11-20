use std::fs;
use std::path::{Path, PathBuf};

use crate::Failure;
use crate::config::Ruleset;
use crate::files::FileSet;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::rs::rule::RsRule;
use crate::rule::rs::source::{RsFile, parse};
use crate::ruleset::rules;

pub(crate) fn check(path: &Path, srcs: &[PathBuf], ruleset: Ruleset) -> Vec<Diagnostic> {
    let Ok(source) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let ast = parse(&source);
    let file = RsFile::new(path, &source, ast.as_ref(), srcs);
    let mut diagnostics = Vec::new();
    for rule in rules(ruleset).rs {
        let RsRule::File(rule) = rule else {
            continue;
        };
        diagnostics.extend((rule.check)(&file));
    }
    diagnostics
}

pub(crate) fn rewrite(path: &Path, ruleset: Ruleset) -> Vec<Failure> {
    let Ok(original) = fs::read_to_string(path) else {
        return Vec::new();
    };
    match preview(path, ruleset) {
        Ok(source) => crate::rule::edit::commit(path, &original, &source),
        Err(output) => vec![Failure { output }],
    }
}

pub(crate) fn preview(path: &Path, ruleset: Ruleset) -> Result<String, String> {
    let original =
        fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut source = original;
    for rule in rules(ruleset).rs {
        let RsRule::File(rule) = rule else {
            continue;
        };
        let Some(edit) = rule.edit else {
            continue;
        };
        let ast = parse(&source);
        let found = {
            let file = RsFile::new(path, &source, ast.as_ref(), &[]);
            edit(&file)
        };
        source = crate::rule::edit::replace(&source, &found)
            .map_err(|error| crate::rule::edit::failure(path, error).output)?;
    }
    Ok(source)
}

pub(crate) fn check_folder(
    path: &Path,
    files: &FileSet,
    srcs: &[PathBuf],
    ruleset: Ruleset,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for rule in rules(ruleset).rs {
        let RsRule::Folder(rule) = rule else {
            continue;
        };
        diagnostics.extend((rule.check)(path, files, srcs));
    }
    diagnostics
}

pub(crate) fn rewrite_folder(
    path: &Path,
    files: &FileSet,
    srcs: &[PathBuf],
    ruleset: Ruleset,
) -> Vec<Failure> {
    let mut failures = Vec::new();
    for rule in rules(ruleset).rs {
        let RsRule::Folder(rule) = rule else {
            continue;
        };
        let Some(edit) = rule.edit else {
            continue;
        };
        failures.extend(crate::rule::edit::write(&edit(path, files, srcs)));
    }
    failures
}
