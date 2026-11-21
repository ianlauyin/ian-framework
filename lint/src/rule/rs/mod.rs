use crate::file::rs::source::RsFile;
use crate::rule::diagnostic::Diagnostic;

pub(crate) mod env_name;
pub(crate) mod exception_code;
pub(crate) mod exception_new;
pub(crate) mod impl_order;
pub(crate) mod inherent_impl;
pub(crate) mod mod_file;
pub(crate) mod mutex_lock;
pub(crate) mod no_comments;
pub(crate) mod private_static;
pub(crate) mod result_type;
pub(crate) mod run;
pub(crate) mod source_order;
pub(crate) mod super_path;
pub(crate) mod test_attr;

pub(crate) struct RsRule {
    pub(crate) check: fn(&RsFile<'_>) -> Vec<Diagnostic>,
}

impl RsRule {
    pub(crate) const fn file(check: fn(&RsFile<'_>) -> Vec<Diagnostic>) -> Self {
        Self { check }
    }
}
