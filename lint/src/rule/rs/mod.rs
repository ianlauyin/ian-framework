use crate::file::rs::source::RsFile;
use crate::rule::diagnostic::Diagnostic;

pub mod cfg_on_mod;
pub mod config_import;
pub mod env_name;
pub mod exception_code;
pub mod exception_new;
pub mod handler_file;
pub mod impl_order;
pub mod import_order;
pub mod inherent_impl;
pub mod mod_file;
pub mod mutex_lock;
pub mod no_comments;
pub mod no_framework_alias;
pub mod no_nested_import;
pub mod no_panic;
pub mod no_self_import;
pub mod ops_import;
pub mod private_static;
pub mod result_type;
pub mod run;
pub mod source_order;
pub mod sqlx_import;
pub mod std_env;
pub mod super_path;
pub mod test_attr;
pub mod test_crate_path;

pub(crate) struct RsRule {
    pub(crate) check: fn(&RsFile<'_>) -> Vec<Diagnostic>,
}

impl RsRule {
    pub(crate) const fn file(check: fn(&RsFile<'_>) -> Vec<Diagnostic>) -> Self {
        Self { check }
    }
}
