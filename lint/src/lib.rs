pub mod args;
pub(crate) mod check;
pub mod config;
pub mod file;
pub mod files;
pub mod rule;
pub(crate) mod ruleset;

pub use check::check;
pub use config::Side;
pub use rule::diagnostic::Diagnostic;

pub struct Report {
    pub diagnostics: Vec<Diagnostic>,
    pub failures: Vec<Failure>,
}

#[derive(Debug)]
pub struct Failure {
    pub output: String,
}
