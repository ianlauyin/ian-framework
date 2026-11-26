use crate::file::ts::source::TsFile;
use crate::rule::diagnostic::Diagnostic;

pub mod ajax_name;
pub mod ajax_no_extends;
pub mod capture_action_name;
pub mod component_folder;
pub mod declaration_order;
pub mod import_alias;
pub mod import_order;
pub mod interface_object_shape;
pub mod module_shape;
pub mod no_arrow_method;
pub mod no_comments;
pub mod no_console;
pub mod no_default_export;
pub mod no_restricted_imports;
pub mod no_snake_case_field;
pub mod page_context;
pub mod private_method_order;
pub mod test_path;

pub(crate) struct TsRule {
    pub(crate) check: fn(&TsFile<'_>) -> Vec<Diagnostic>,
    pub(crate) tests: bool,
}

impl TsRule {
    pub(crate) const fn file(check: fn(&TsFile<'_>) -> Vec<Diagnostic>) -> Self {
        Self { check, tests: true }
    }

    pub(crate) const fn source(check: fn(&TsFile<'_>) -> Vec<Diagnostic>) -> Self {
        Self {
            check,
            tests: false,
        }
    }
}
