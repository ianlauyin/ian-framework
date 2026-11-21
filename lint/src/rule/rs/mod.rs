use crate::file::rs::source::RsFile;
use crate::rule::diagnostic::Diagnostic;

pub mod env_name;
pub mod exception_code;
pub mod exception_new;
pub mod impl_order;
pub mod inherent_impl;
pub mod mod_file;
pub mod mutex_lock;
pub mod no_comments;
pub mod private_static;
pub mod result_type;
pub mod run;
pub mod source_order;
pub mod super_path;
pub mod test_attr;

pub(crate) struct RsRule {
    pub(crate) check: fn(&RsFile<'_>) -> Vec<Diagnostic>,
}

impl RsRule {
    pub(crate) const fn file(check: fn(&RsFile<'_>) -> Vec<Diagnostic>) -> Self {
        Self { check }
    }
}
