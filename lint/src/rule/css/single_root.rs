// The file has more than one root selector.
// Keep one root selector. Nest each other selector in its block.
// Why a lint rule: The styles of one `index.css` stay under the root of its component.

use crate::file::css::parse::Kind;
use crate::file::css::selector::selectors;
use crate::file::css::source::CssFile;
use crate::file::place::Place;
use crate::files::file_name;
use crate::rule::css::CssRule;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::report::RuleReport;

const RULE: &str = "css-single-root";

const FILE: &str = "index.css";

pub(crate) const CSS_RULE: CssRule = CssRule::new(check);

pub fn check(file: &CssFile) -> Vec<Diagnostic> {
    if file_name(file.path) != FILE || Place::new(file.path, file.srcs).is_file(FILE) {
        return Vec::new();
    }
    let mut report = RuleReport::new(file.path, file.source, RULE);
    let mut roots = 0;
    for item in file.items {
        let Kind::Rule { selector_end, .. } = &item.kind else {
            continue;
        };
        let text = &file.source[item.start as usize..*selector_end as usize];
        for (offset, _) in selectors(text) {
            roots += 1;
            if roots > 1 {
                report.at_offset(
                    item.start + offset as u32,
                    "The file has more than one root selector.",
                    "Keep one root selector. Nest each other selector in its block.",
                );
            }
        }
    }
    report.finish()
}
