use std::path::{Path, PathBuf};

use crate::files::FileSet;
use crate::parser::rs::source::RsFile;
use crate::rule::diagnostic::Diagnostic;

pub(crate) struct RsFileRule {
    pub(crate) check: fn(&RsFile<'_>) -> Vec<Diagnostic>,
}

pub(crate) struct RsFolderRule {
    pub(crate) check: fn(&Path, &FileSet, &[PathBuf]) -> Vec<Diagnostic>,
}

pub(crate) enum RsRule {
    File(RsFileRule),
    Folder(RsFolderRule),
}

impl RsRule {
    pub(crate) const fn file(check: fn(&RsFile<'_>) -> Vec<Diagnostic>) -> Self {
        Self::File(RsFileRule { check })
    }

    pub(crate) const fn folder(check: fn(&Path, &FileSet, &[PathBuf]) -> Vec<Diagnostic>) -> Self {
        Self::Folder(RsFolderRule { check })
    }
}
