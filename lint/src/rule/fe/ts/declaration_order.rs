// This item is out of declaration order.
// Put imports first, then private types and constants, then exported items, then private interfaces, then private functions.
// Why a lint rule: That order keeps the exported items in one place.

use oxc_ast::ast::Statement;
use oxc_span::GetSpan;

use crate::rule::diagnostic::Diagnostic;
use crate::rule::edit::{Edit, piece_ranges};
use crate::rule::fe::ts::file::TsFile;
use crate::rule::fe::ts::rule::TsRule;
use crate::rule::report::RuleReport;

const RULE: &str = "declaration-order";

pub(crate) const TS_RULE: TsRule = TsRule::file(check, Some(edit));

const IMPORT: u8 = 0;
const PRIVATE_TYPE_OR_CONSTANT: u8 = 1;
const EXPORT: u8 = 2;
const PRIVATE_INTERFACE: u8 = 3;
const PRIVATE_FUNCTION: u8 = 4;

pub(crate) fn edit(file: &TsFile) -> Vec<Edit> {
    let Some(program) = file.program else {
        return Vec::new();
    };
    if program.body.is_empty()
        || program
            .body
            .iter()
            .any(|statement| group(statement).is_none())
    {
        return Vec::new();
    }
    let spans: Vec<(usize, usize)> = program
        .body
        .iter()
        .map(|statement| {
            let span = statement.span();
            (span.start as usize, span.end as usize)
        })
        .collect();
    let Some((start, end, ranges)) = piece_ranges(file.source, &spans) else {
        return Vec::new();
    };
    let mut pieces = Vec::with_capacity(program.body.len());
    for (index, statement) in program.body.iter().enumerate() {
        let Some(group) = group(statement) else {
            return Vec::new();
        };
        let (from, to) = ranges[index];
        pieces.push(Piece {
            group,
            index,
            text: file.source[from..to].to_owned(),
        });
    }
    pieces.sort_by(|left, right| {
        left.group
            .cmp(&right.group)
            .then(left.index.cmp(&right.index))
    });
    let text: String = pieces.iter().map(|piece| piece.text.as_str()).collect();
    if text == file.source[start..end] {
        return Vec::new();
    }
    vec![Edit {
        path: file.path.to_path_buf(),
        start: start as u32,
        end: end as u32,
        text,
    }]
}

struct Piece {
    group: u8,
    index: usize,
    text: String,
}

pub(crate) fn check(file: &TsFile) -> Vec<Diagnostic> {
    let Some(program) = file.program else {
        return Vec::new();
    };
    let mut report = RuleReport::new(file.path, file.source, RULE);
    let mut last: Option<u8> = None;
    for statement in &program.body {
        let Some(group) = group(statement) else {
            report_order(&mut report, statement.span().start);
            continue;
        };
        if last.is_some_and(|previous| group < previous) {
            report_order(&mut report, statement.span().start);
        }
        last = Some(last.map_or(group, |previous| previous.max(group)));
    }
    report.finish()
}

fn group(statement: &Statement<'_>) -> Option<u8> {
    match statement {
        Statement::ImportDeclaration(_) | Statement::TSImportEqualsDeclaration(_) => Some(IMPORT),
        Statement::VariableDeclaration(_) | Statement::TSTypeAliasDeclaration(_) => {
            Some(PRIVATE_TYPE_OR_CONSTANT)
        }
        Statement::ExportAllDeclaration(_)
        | Statement::ExportDefaultDeclaration(_)
        | Statement::ExportNamedDeclaration(_)
        | Statement::ExportDeclaration(_)
        | Statement::ExportFromDeclaration(_)
        | Statement::TSExportAssignment(_)
        | Statement::TSNamespaceExportDeclaration(_) => Some(EXPORT),
        Statement::TSInterfaceDeclaration(_) => Some(PRIVATE_INTERFACE),
        Statement::FunctionDeclaration(_) => Some(PRIVATE_FUNCTION),
        _ => None,
    }
}

fn report_order(report: &mut RuleReport<'_>, offset: u32) {
    report.at_offset(
        offset,
        "This item is out of declaration order.",
        "Put imports first, then private types and constants, then exported items, then private interfaces, then private functions.",
    );
}
