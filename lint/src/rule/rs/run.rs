use std::fs;
use std::path::{Path, PathBuf};

use crate::config::Ruleset;
use crate::files::FileSet;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::rs::rule::RsRule;
use crate::rule::rs::source::RsFile;
use crate::ruleset::rules;

pub(crate) fn check(
    path: &Path,
    srcs: &[PathBuf],
    ruleset: Ruleset,
) -> Result<Vec<Diagnostic>, String> {
    let source =
        fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let ast = syn::parse_file(&source).map_err(|error| {
        let start = error.span().start();
        format!(
            "{}:{}:{}: The file does not parse: {error}",
            path.display(),
            start.line,
            start.column + 1
        )
    })?;
    let file = RsFile::new(path, &source, Some(&ast), srcs);
    let mut diagnostics = Vec::new();
    for rule in rules(ruleset).rs {
        let RsRule::File(rule) = rule else {
            continue;
        };
        diagnostics.extend((rule.check)(&file));
    }
    Ok(diagnostics)
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
