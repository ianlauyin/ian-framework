use std::fs;
use std::path::{Path, PathBuf};

use crate::config::Ruleset;
use crate::file::rs::source::RsFile;
use crate::rule::diagnostic::Diagnostic;
use crate::ruleset::rules;

pub fn check(path: &Path, srcs: &[PathBuf], ruleset: Ruleset) -> Result<Vec<Diagnostic>, String> {
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
        diagnostics.extend((rule.check)(&file));
    }
    Ok(diagnostics)
}
