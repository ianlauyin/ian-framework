pub(crate) mod check;
pub(crate) mod config;
pub(crate) mod file;
pub(crate) mod files;
pub(crate) mod rule;
pub(crate) mod ruleset;

pub use check::check;
pub use config::Side;
pub use rule::diagnostic::Diagnostic;

#[cfg(test)]
#[path = "../test/mod.rs"]
mod test;

pub struct Report {
    pub diagnostics: Vec<Diagnostic>,
    pub failures: Vec<Failure>,
}

#[derive(Debug)]
pub struct Failure {
    pub output: String,
}
