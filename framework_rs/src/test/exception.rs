use crate::exception;

#[test]
fn display_shows_the_current_message_only() {
    let root = exception!("port is not a number", code = "ENV_INVALID");
    let middle = exception!("cannot read the secret", code = "ENV_SECRET", source = root);
    let top = exception!("app cannot start", code = "ENV_MISSING", source = middle);
    assert_eq!(top.to_string(), "app cannot start");
    let causes: Vec<_> = top.trace().map(|e| e.code()).collect();
    assert_eq!(causes, ["ENV_MISSING", "ENV_SECRET", "ENV_INVALID"]);
}

#[test]
fn display_and_debug_do_not_show_the_source_text() {
    let leak = "https://host/o?token=ya29.fake-token&sig=abc";
    let external = exception!(
        "cannot connect",
        code = "ENV_SECRET",
        source = std::io::Error::other(leak)
    );
    let top = exception!(
        "outer",
        code = "ENV_MISSING",
        source = exception!(
            "inner",
            code = "ENV_INVALID",
            source = exception!(
                "cannot connect",
                code = "ENV_SECRET",
                source = std::io::Error::other(leak)
            )
        )
    );
    for error in [&external, &top] {
        for text in [
            error.to_string(),
            format!("{error:?}"),
            format!("{error:#?}"),
            String::from(error),
        ] {
            assert!(!text.contains("fake-token"), "{text}");
            assert!(!text.contains("token="), "{text}");
            assert!(!text.contains("sig=abc"), "{text}");
        }
    }
    for node in top.trace() {
        assert!(!node.message().contains("fake-token"), "{}", node.message());
    }
    let root = top.trace().last().unwrap();
    assert_eq!(root.code(), "ENV_SECRET");
    assert_eq!(root.message(), "cannot connect");
    let io_error = std::error::Error::source(root)
        .unwrap()
        .downcast_ref::<std::io::Error>()
        .unwrap();
    assert_eq!(io_error.kind(), std::io::ErrorKind::Other);
    let debug = format!("{top:?}");
    assert!(
        debug.contains("ENV_MISSING") && debug.contains("outer"),
        "{debug}"
    );
}

#[test]
fn exception_records_the_file_and_the_line_of_the_call() {
    let error = exception!("APP_KEY is required", code = "ENV_MISSING");
    let line = line!() - 1;
    assert_eq!(error.location(), (file!(), line));
    let external = exception!(
        "x",
        code = "ENV_SECRET",
        source = std::io::Error::other("refused")
    );
    assert!(external.source_exception().is_none());
    assert!(std::error::Error::source(&external)
        .unwrap()
        .downcast_ref::<std::io::Error>()
        .is_some());
}

#[test]
fn display_of_one_exception_has_no_trace() {
    let error = exception!("APP_KEY is required", code = "ENV_MISSING");
    assert_eq!(error.to_string(), "APP_KEY is required");
    assert!(std::error::Error::source(&error).is_none());
}

#[test]
fn with_source_stores_the_external_error() {
    let error = exception!(
        "cannot connect",
        code = "ENV_SECRET",
        source = std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "refused")
    );
    assert!(error.source_exception().is_none());
    let causes: Vec<_> = error.trace().map(|e| (e.code(), e.message())).collect();
    assert_eq!(causes, [("ENV_SECRET", "cannot connect")]);
    let io_error = std::error::Error::source(&error)
        .unwrap()
        .downcast_ref::<std::io::Error>()
        .unwrap();
    assert_eq!(io_error.kind(), std::io::ErrorKind::ConnectionRefused);
    assert!(!error.to_string().contains("refused"));
    assert!(!String::from(&error).contains("refused"));
}

#[test]
fn with_code_keeps_the_message_and_source() {
    let error = exception!(
        "outer",
        code = "ENV_MISSING",
        source = exception!("bad", code = "ENV_INVALID")
    );
    let error = error.with_code("ENV_SECRET");
    assert_eq!(error.code(), "ENV_SECRET");
    assert_eq!(error.message(), "outer");
    let cause = error.source_exception().unwrap();
    assert_eq!(cause.code(), "ENV_INVALID");
    assert_eq!(cause.message(), "bad");
}

#[test]
fn an_exception_given_as_source_keeps_its_code() {
    let inner = exception!("inner", code = "ENV_INVALID");
    let location = inner.location();
    let outer = exception!("outer", code = "ENV_SECRET", source = inner);
    let cause = outer.source_exception().unwrap();
    assert_eq!(cause.code(), "ENV_INVALID");
    assert_eq!(cause.location(), location);
}

#[derive(Debug)]
struct Outer(std::io::Error);

impl std::fmt::Display for Outer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "outer https://h/o?token=fake-token")
    }
}

impl std::error::Error for Outer {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

#[test]
fn an_external_source_keeps_its_cause_and_hides_its_text() {
    let leak = std::io::Error::new(std::io::ErrorKind::NotFound, "https://h/o?token=fake-token");
    let error = exception!("x", code = "ENV_SECRET", source = Outer(leak));
    assert!(error.source_exception().is_none());
    assert_eq!(error.trace().count(), 1);
    let outer = std::error::Error::source(&error)
        .unwrap()
        .downcast_ref::<Outer>()
        .unwrap();
    let io_error = std::error::Error::source(outer)
        .unwrap()
        .downcast_ref::<std::io::Error>()
        .unwrap();
    assert_eq!(io_error.kind(), std::io::ErrorKind::NotFound);
    assert!(!error.to_string().contains("fake-token"));
    assert!(!format!("{error:?}").contains("fake-token"));
    assert!(!String::from(&error).contains("fake-token"));
    for node in error.trace() {
        assert!(!node.message().contains("fake-token"));
    }
}

#[test]
fn log_text_has_every_message_with_its_code_and_location() {
    let root = exception!("port is not a number", code = "ENV_INVALID");
    let line = line!() - 1;
    let top = exception!("app cannot start", code = "ENV_MISSING", source = root);
    let top_line = line!() - 1;
    let external_line = line!() + 1;
    let external = exception!(
        "cannot connect",
        code = "ENV_SECRET",
        source = std::io::Error::other("refused")
    );
    let file = file!();
    assert_eq!(
        String::from(&top),
        format!(
            "0: ERROR [ENV_MISSING] app cannot start at {file}:{top_line}\n1: ERROR [ENV_INVALID] port is not a number at {file}:{line}"
        )
    );
    let external_text = String::from(&external);
    assert_eq!(
        external_text,
        format!("0: ERROR [ENV_SECRET] cannot connect at {file}:{external_line}")
    );
    assert!(!external_text.contains("refused"));
}

#[test]
fn a_warn_exception_shows_its_severity_and_code() {
    let warn = exception!(
        "slow",
        code = "SLOW",
        severity = crate::exception::Severity::Warn
    );
    assert_eq!(warn.code(), "SLOW");
    assert_eq!(warn.severity(), crate::exception::Severity::Warn);
    assert!(String::from(&warn).starts_with("0: WARN [SLOW] slow at "));
}
