use std::fs;
use std::path::{Path, PathBuf};

use oxc_allocator::Allocator;

use crate::config::Ruleset;
use crate::files::FileSet;
use crate::parser::css::source::CssFile;
use crate::parser::ts::source::TsFile;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::ts::rule::TsRule;
use crate::ruleset::rules;

pub(crate) fn check(path: &Path, ruleset: Ruleset) -> Result<Vec<Diagnostic>, String> {
    let rules = rules(ruleset);
    let source =
        fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut diagnostics = Vec::new();
    if crate::files::file_name(path).ends_with(".css") {
        let items = crate::parser::css::parse::parse(&source);
        let file = CssFile::new(path, &source, &items);
        for rule in rules.css {
            diagnostics.extend((rule.check)(&file));
        }
    } else {
        let allocator = Allocator::default();
        let program = crate::parser::ts::source::parse_valid(&allocator, path, &source)?;
        let file = TsFile::new(path, &source, Some(&program));
        for rule in rules.ts {
            let TsRule::File(rule) = rule else {
                continue;
            };
            diagnostics.extend((rule.check)(&file));
        }
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
    for rule in rules(ruleset).ts {
        let TsRule::Folder(rule) = rule else {
            continue;
        };
        diagnostics.extend((rule.check)(path, files, srcs));
    }
    diagnostics
}
