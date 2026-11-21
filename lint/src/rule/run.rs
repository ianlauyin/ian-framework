use std::fs;
use std::path::Path;

use oxc_allocator::Allocator;

use crate::config::Ruleset;
use crate::file::css::source::CssFile;
use crate::file::ts::source::TsFile;
use crate::rule::diagnostic::Diagnostic;
use crate::ruleset::rules;

pub fn check(path: &Path, ruleset: Ruleset) -> Result<Vec<Diagnostic>, String> {
    let rules = rules(ruleset);
    let source =
        fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut diagnostics = Vec::new();
    if crate::files::file_name(path).ends_with(".css") {
        let items = crate::file::css::parse::parse(&source);
        let file = CssFile::new(path, &source, &items);
        for rule in rules.css {
            diagnostics.extend((rule.check)(&file));
        }
    } else {
        let allocator = Allocator::default();
        let program = crate::file::ts::source::parse_valid(&allocator, path, &source)?;
        let file = TsFile::new(path, &source, Some(&program));
        for rule in rules.ts {
            diagnostics.extend((rule.check)(&file));
        }
    }
    Ok(diagnostics)
}
