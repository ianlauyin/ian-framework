use std::path::Path;

use crate::rule::rs::file::result_type::check;
use crate::test::rule::rs::source::with_source;

#[test]
fn a_result_of_exception_passes() {
    let source = "\
fn read() -> Result<String, Exception> {
    Ok(String::new())
}

struct Tube;

impl Tube {
    fn load(&self) -> Result<(), crate::exception::Exception> {
        Ok(())
    }
}

impl std::io::Read for Tube {
    fn read(&mut self, _buf: &mut [u8]) -> Result<usize, std::io::Error> {
        Ok(0)
    }
}

fn fine() -> i32 {
    1
}
";
    assert!(with_source(Path::new("api/src/tube.rs"), source, &[], check).is_empty());
}

#[test]
fn a_result_with_another_error_fails() {
    let source = "\
fn read() -> Result<String, std::io::Error> {
    Ok(String::new())
}

struct Tube;

impl Tube {
    fn load(&self) -> io::Result<()> {
        Ok(())
    }
}
";
    let found = with_source(Path::new("api/src/tube.rs"), source, &[], check);
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].line, 1);
    assert_eq!(found[0].rule, "result-type");
    assert_eq!(
        found[0].problem,
        "`read` does not return `Result<T, Exception>`."
    );
    assert_eq!(found[0].fix, "Return `Result<T, Exception>`.");
    assert_eq!(found[1].line, 8);
    assert_eq!(
        found[1].problem,
        "`load` does not return `Result<T, Exception>`."
    );
    assert!(found.iter().all(|item| item.fix == found[0].fix));
}

#[test]
fn a_file_that_is_not_valid_rust_has_no_diagnostic() {
    assert!(with_source(Path::new("api/src/tube.rs"), "fn (", &[], check).is_empty());
}
