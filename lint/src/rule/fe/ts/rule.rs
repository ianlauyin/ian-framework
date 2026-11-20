use std::path::{Path, PathBuf};

use crate::files::FileSet;
use crate::rule::diagnostic::Diagnostic;
use crate::rule::edit::Edit;
use crate::rule::fe::ts::declaration_order;
use crate::rule::fe::ts::file::TsFile;
use crate::rule::fe::ts::folder::index_file;
use crate::rule::fe::ts::folder::root_reexport;
use crate::rule::fe::ts::folder::test_path;
use crate::rule::fe::ts::no_comments;
use crate::rule::fe::ts::no_default_export;
use crate::rule::fe::ts::no_restricted_imports;

pub(crate) struct TsFileRule {
    pub(crate) check: fn(&TsFile<'_>) -> Vec<Diagnostic>,
    pub(crate) edit: Option<fn(&TsFile<'_>) -> Vec<Edit>>,
}

pub(crate) struct TsFolderRule {
    pub(crate) check: fn(&Path, &FileSet, &[PathBuf]) -> Vec<Diagnostic>,
    pub(crate) edit: Option<fn(&Path, &FileSet, &[PathBuf]) -> Vec<Edit>>,
}

pub(crate) enum TsRule {
    File(TsFileRule),
    Folder(TsFolderRule),
}

impl TsRule {
    pub(crate) const fn file(
        check: fn(&TsFile<'_>) -> Vec<Diagnostic>,
        edit: Option<fn(&TsFile<'_>) -> Vec<Edit>>,
    ) -> Self {
        Self::File(TsFileRule { check, edit })
    }

    pub(crate) const fn folder(
        check: fn(&Path, &FileSet, &[PathBuf]) -> Vec<Diagnostic>,
        edit: Option<fn(&Path, &FileSet, &[PathBuf]) -> Vec<Edit>>,
    ) -> Self {
        Self::Folder(TsFolderRule { check, edit })
    }
}

pub(crate) const FRAMEWORK: &[TsRule] = &[
    TsRule::file(no_restricted_imports::check, None),
    TsRule::file(no_comments::check, None),
    TsRule::file(no_default_export::check, None),
    TsRule::file(declaration_order::check, Some(declaration_order::edit)),
    TsRule::folder(index_file::check, None),
    TsRule::folder(root_reexport::check, None),
    TsRule::folder(test_path::check, None),
];

pub(crate) const PROJECT: &[TsRule] = &[
    TsRule::file(no_restricted_imports::check, None),
    TsRule::file(no_comments::check, None),
    TsRule::file(declaration_order::check, Some(declaration_order::edit)),
    TsRule::folder(test_path::check, None),
];
