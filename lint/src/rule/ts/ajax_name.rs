// The file has no exported function `{name}`.
// The function `{found}` does not have the name `{name}`.
// Name the function from the path of the file, and end the name with `Ajax`.
// The type `{found}` does not have the name `{name}`.
// Name the type from the name of the function.
// The type `{name}` is not immediately before its function.
// Put the type immediately before the function.
// Why a lint rule: The name of a call gives its handler, and the types of a call are next to the call.

use oxc_ast::ast::{Declaration, Statement};
use oxc_span::GetSpan;

use crate::file::place::Place;
use crate::file::ts::source::TsFile;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::report::RuleReport;
use crate::rule::ts::TsRule;

const RULE: &str = "ajax-name";

const SHARED: &str = "shared-types.ts";
const SUFFIXES: [&str; 2] = ["Request", "Response"];
const NAME_FIX: &str = "Name the function from the path of the file, and end the name with `Ajax`.";

pub(crate) const TS_RULE: TsRule = TsRule::source(check);

enum Kind<'a> {
    Function(&'a str),
    Type(&'a str),
    Other,
}

pub fn check(file: &TsFile) -> Vec<Diagnostic> {
    let Some(program) = file.program else {
        return Vec::new();
    };
    let place = Place::new(file.path, file.srcs);
    let parts = place.parts();
    let ["util", "ajax", rest @ ..] = &parts[..] else {
        return Vec::new();
    };
    if rest.is_empty() || rest == [SHARED] {
        return Vec::new();
    }
    let rest = match rest {
        ["api", tail @ ..] if !tail.is_empty() => tail,
        _ => rest,
    };
    let base = pascal_case(rest);
    let function = format!("{}Ajax", lower_first(&base));
    let kinds: Vec<_> = program.body.iter().map(kind).collect();
    let mut report = RuleReport::new(file.path, file.source, RULE);
    if !kinds.iter().any(|kind| matches!(kind, Kind::Function(_))) {
        report.at_offset(
            0,
            format!("The file has no exported function `{function}`."),
            NAME_FIX,
        );
    }
    for (index, statement) in program.body.iter().enumerate() {
        let offset = statement.span().start;
        match kinds[index] {
            Kind::Function(name) if name != function => report.at_offset(
                offset,
                format!("The function `{name}` does not have the name `{function}`."),
                NAME_FIX,
            ),
            Kind::Type(name) => {
                let expected = SUFFIXES
                    .iter()
                    .find(|suffix| name.ends_with(**suffix))
                    .map(|suffix| format!("{base}{suffix}"));
                if expected.as_deref() != Some(name) {
                    report.at_offset(
                        offset,
                        format!(
                            "The type `{name}` does not have the name `{}`.",
                            expected.unwrap_or_default()
                        ),
                        "Name the type from the name of the function.",
                    );
                }
                if !before_function(&kinds[index + 1..]) {
                    report.at_offset(
                        offset,
                        format!("The type `{name}` is not immediately before its function."),
                        "Put the type immediately before the function.",
                    );
                }
            }
            _ => {}
        }
    }
    report.finish()
}

fn before_function(rest: &[Kind<'_>]) -> bool {
    matches!(
        rest.iter().find(|kind| !matches!(kind, Kind::Type(_))),
        Some(Kind::Function(_))
    )
}

fn kind<'a>(statement: &'a Statement<'_>) -> Kind<'a> {
    let name = match statement {
        Statement::TSTypeAliasDeclaration(item) => item.id.name.as_str(),
        Statement::TSInterfaceDeclaration(item) => item.id.name.as_str(),
        Statement::ExportDeclaration(export) => match &export.declaration {
            Declaration::FunctionDeclaration(function) => {
                return function
                    .id
                    .as_ref()
                    .map_or(Kind::Other, |id| Kind::Function(id.name.as_str()));
            }
            Declaration::TSTypeAliasDeclaration(item) => item.id.name.as_str(),
            Declaration::TSInterfaceDeclaration(item) => item.id.name.as_str(),
            _ => return Kind::Other,
        },
        _ => return Kind::Other,
    };
    if SUFFIXES.iter().any(|suffix| name.ends_with(suffix)) {
        Kind::Type(name)
    } else {
        Kind::Other
    }
}

fn pascal_case(parts: &[&str]) -> String {
    let mut out = String::new();
    for part in parts {
        let part = part.split('.').next().unwrap_or(part);
        for word in part.split(['-', '_']) {
            let mut chars = word.chars();
            let Some(first) = chars.next() else {
                continue;
            };
            out.extend(first.to_uppercase());
            out.push_str(chars.as_str());
        }
    }
    out
}

fn lower_first(name: &str) -> String {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    first.to_lowercase().chain(chars).collect()
}
