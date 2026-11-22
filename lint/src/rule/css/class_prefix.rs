// The root selector `{selector}` is not `.g-{name}`.
// The root selector `{selector}` does not start with `.g-{name}-`.
// Give the outermost tag of the component the class `g-{name}`.
// The root selector `{selector}` does not name the page.
// Use `#{page}-page` for the outermost tag of the page, or a class that starts with `{page}-page-`.
// The root selector `{selector}` is a class.
// Use the id `#{page}-page`.
// The class `{class}` does not start with `{prefix}`.
// Start the class with `{prefix}`.
// Why a lint rule: The name of a class gives the component or the page that owns the class.

use crate::file::css::parse::{Item, Kind};
use crate::file::css::selector::{classes, is_css_ws_char, name_end, selectors};
use crate::file::css::source::CssFile;
use crate::file::place::Place;
use crate::rule::css::CssRule;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::report::RuleReport;

const RULE: &str = "css-class-prefix";

const SHARED: &str = "g-";
const GROUPS: [&str; 4] = ["@media", "@supports", "@container", "@layer"];

pub(crate) const CSS_RULE: CssRule = CssRule::new(check);

enum Owner {
    Component { class: String, direct: bool },
    Page { names: Vec<String>, folder: String },
}

pub fn check(file: &CssFile) -> Vec<Diagnostic> {
    let place = Place::new(file.path, file.srcs);
    let parts = place.parts();
    let Some((_, folders)) = parts.split_last() else {
        return Vec::new();
    };
    let owner = match folders {
        ["component", name, rest @ ..] => Owner::Component {
            class: format!("{SHARED}{}", kebab_case(name)),
            direct: rest.is_empty(),
        },
        ["page", rest @ ..] => {
            let pages: Vec<_> = rest.iter().filter(|name| is_page_folder(name)).collect();
            let Some(folder) = pages.last() else {
                return Vec::new();
            };
            Owner::Page {
                names: pages.iter().flat_map(|name| page_names(name)).collect(),
                folder: folder.to_string(),
            }
        }
        _ => return Vec::new(),
    };
    let mut pass = Pass {
        report: RuleReport::new(file.path, file.source, RULE),
        source: file.source,
        owner,
    };
    pass.walk(file.items, None);
    pass.report.finish()
}

struct Pass<'a> {
    report: RuleReport<'a>,
    source: &'a str,
    owner: Owner,
}

impl Pass<'_> {
    fn walk(&mut self, items: &[Item], prefix: Option<&str>) {
        for item in items {
            match &item.kind {
                Kind::Rule {
                    selector_end,
                    children,
                } => {
                    let raw = &self.source[item.start as usize..*selector_end as usize];
                    let text = raw.trim_end_matches(is_css_ws_char);
                    match prefix {
                        Some(prefix) => {
                            self.nested(item.start, text, prefix);
                            self.walk(children, Some(prefix));
                        }
                        None => {
                            if let Some(prefix) = self.root(item.start, text) {
                                self.walk(children, Some(&prefix));
                            }
                        }
                    }
                }
                Kind::AtRule { children, .. } => {
                    let text = &self.source[item.start as usize..];
                    if GROUPS.iter().any(|name| text.starts_with(name)) {
                        self.walk(children, prefix);
                    }
                }
                Kind::Comment | Kind::Declaration => {}
            }
        }
    }

    fn root(&mut self, start: u32, text: &str) -> Option<String> {
        let mut prefix = None;
        for (offset, selector) in selectors(text) {
            let found = match &self.owner {
                Owner::Component { class, direct } => component_root(selector, class, *direct),
                Owner::Page { names, folder } => page_root(selector, names, folder),
            };
            match found {
                Ok(found) => prefix = prefix.or(Some(found)),
                Err((problem, fix)) => self.report.at_offset(start + offset as u32, problem, fix),
            }
        }
        prefix
    }

    fn nested(&mut self, start: u32, text: &str, prefix: &str) {
        for (offset, selector) in selectors(text) {
            for class in classes(selector) {
                if class.starts_with(prefix) || class.starts_with(SHARED) {
                    continue;
                }
                self.report.at_offset(
                    start + offset as u32,
                    format!("The class `{class}` does not start with `{prefix}`."),
                    format!("Start the class with `{prefix}`."),
                );
            }
        }
    }
}

fn component_root(selector: &str, class: &str, direct: bool) -> Result<String, (String, String)> {
    let found = single_name(selector, '.');
    let valid = if direct {
        found == Some(class)
    } else {
        found.is_some_and(|found| {
            found
                .strip_prefix(class)
                .is_some_and(|rest| rest.starts_with('-'))
        })
    };
    if valid {
        return Ok(SHARED.to_string());
    }
    let problem = if direct {
        format!("The root selector `{selector}` is not `.{class}`.")
    } else {
        format!("The root selector `{selector}` does not start with `.{class}-`.")
    };
    Err((
        problem,
        format!("Give the outermost tag of the component the class `{class}`."),
    ))
}

fn page_root(selector: &str, names: &[String], folder: &str) -> Result<String, (String, String)> {
    let id = single_name(selector, '#').and_then(|id| id.strip_suffix("-page"));
    if let Some(page) = id
        && names.iter().any(|name| name == page)
    {
        return Ok(format!("{page}-page-"));
    }
    if let Some(class) = single_name(selector, '.') {
        for page in names {
            let Some(rest) = class.strip_prefix(&format!("{page}-page")) else {
                continue;
            };
            if rest.is_empty() {
                return Err((
                    format!("The root selector `{selector}` is a class."),
                    format!("Use the id `#{page}-page`."),
                ));
            }
            if rest.starts_with('-') {
                return Ok(format!("{page}-page-"));
            }
        }
    }
    Err((
        format!("The root selector `{selector}` does not name the page."),
        format!(
            "Use `#{folder}-page` for the outermost tag of the page, or a class that starts with `{folder}-page-`."
        ),
    ))
}

fn single_name(selector: &str, sigil: char) -> Option<&str> {
    let name = selector.strip_prefix(sigil)?;
    (!name.is_empty() && name_end(name, 0) == name.len()).then_some(name)
}

fn is_page_folder(name: &str) -> bool {
    name.chars().next().is_some_and(|char| char.is_lowercase())
}

fn page_names(folder: &str) -> Vec<String> {
    let mut names = vec![folder.to_string()];
    names.extend(
        folder
            .match_indices('-')
            .map(|(index, _)| folder[index + 1..].to_string()),
    );
    names
}

fn kebab_case(name: &str) -> String {
    let mut out = String::new();
    for char in name.chars() {
        if char.is_uppercase() && !out.is_empty() {
            out.push('-');
        }
        out.extend(char.to_lowercase());
    }
    out
}
