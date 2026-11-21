use std::path::{Path, PathBuf};

use crate::files::FileSet;
use crate::parser::ts::source::TsFile;
use crate::rule::diagnostic::Diagnostic;

pub(crate) struct TsFileRule {
    pub(crate) check: fn(&TsFile<'_>) -> Vec<Diagnostic>,
}

pub(crate) struct TsFolderRule {
    pub(crate) check: fn(&Path, &FileSet, &[PathBuf]) -> Vec<Diagnostic>,
}

pub(crate) enum TsRule {
    File(TsFileRule),
    Folder(TsFolderRule),
}

impl TsRule {
    pub(crate) const fn file(check: fn(&TsFile<'_>) -> Vec<Diagnostic>) -> Self {
        Self::File(TsFileRule { check })
    }

    pub(crate) const fn folder(check: fn(&Path, &FileSet, &[PathBuf]) -> Vec<Diagnostic>) -> Self {
        Self::Folder(TsFolderRule { check })
    }
}
