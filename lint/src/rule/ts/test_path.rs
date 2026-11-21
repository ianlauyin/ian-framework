// A test file is in the source folder.
// Move the test to the `tests` folder.
// Why a lint rule: A test stays out of the source folder.

use crate::file::ts::source::TsFile;
use crate::files::file_name;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::ts::TsRule;

const RULE: &str = "test-path";

pub(crate) const TS_RULE: TsRule = TsRule::source(check);

pub fn check(file: &TsFile) -> Vec<Diagnostic> {
    let name = file_name(file.path);
    if !name.ends_with(".test.ts") && !name.ends_with(".test.tsx") {
        return Vec::new();
    }
    vec![Diagnostic::new(
        file.path,
        1,
        1,
        RULE,
        "A test file is in the source folder.",
        "Move the test to the `tests` folder.",
    )]
}
