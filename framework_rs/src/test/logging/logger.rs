use std::collections::BTreeMap;

use crate::logging::LogShipper;

use crate::logging::logger::*;

#[test]
fn logs_without_a_shipper_and_keeps_the_first_shipper() {
    Logger::error("TEST/error", BTreeMap::new(), "failed".into());
    if Logger::get().is_none() {
        let first = LogShipper::new(crate::logging::ShipperConfig::new("my-app")).unwrap();
        assert!(Logger::init(first).is_ok());
    }
    let second = LogShipper::new(crate::logging::ShipperConfig::new("my-app")).unwrap();
    assert!(Logger::init(second).is_err());
    Logger::warn("TEST/warn", BTreeMap::new());
    Logger::warn_with("TEST/warn", BTreeMap::new(), "slow".into());
}

#[test]
fn error_adds_the_exception_trace_to_the_message() {
    let messages = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let capture = Capture(std::sync::Arc::clone(&messages));
    let root = crate::exception!("port is not a number", code = "ENV_INVALID");
    let top = crate::exception!("app cannot start", code = "ENV_MISSING", source = root);
    tracing::subscriber::with_default(capture, || {
        Logger::error("TEST/exceptionTrace", BTreeMap::new(), Box::new(top));
        Logger::error("TEST/plain", BTreeMap::new(), "slow".into());
    });
    let messages = messages.lock().unwrap();
    let text = messages[0].as_str();
    assert!(text.starts_with("app cannot start\n0: ERROR [ENV_MISSING] app cannot start at "));
    assert!(text.contains("\n1: ERROR [ENV_INVALID] port is not a number at "));
    assert_eq!(messages[1], "slow");
}

struct Capture(std::sync::Arc<std::sync::Mutex<Vec<String>>>);

struct MessageVisitor<'a>(&'a mut Option<String>);

impl tracing::field::Visit for MessageVisitor<'_> {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "error_message" {
            *self.0 = Some(value.to_string());
        }
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "error_message" && self.0.is_none() {
            let text = format!("{value:?}");
            let text = text
                .strip_prefix("Some(\"")
                .and_then(|text| text.strip_suffix("\")"))
                .unwrap_or(&text);
            *self.0 = Some(text.replace("\\n", "\n").replace("\\\"", "\""));
        }
    }
}

impl tracing::Subscriber for Capture {
    fn enabled(&self, metadata: &tracing::Metadata<'_>) -> bool {
        metadata.is_event()
    }

    fn new_span(&self, _span: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(1)
    }

    fn record(&self, _span: &tracing::span::Id, _values: &tracing::span::Record<'_>) {}

    fn record_follows_from(&self, _span: &tracing::span::Id, _follows: &tracing::span::Id) {}

    fn event(&self, event: &tracing::Event<'_>) {
        let mut message = None;
        event.record(&mut MessageVisitor(&mut message));
        if let Some(message) = message {
            self.0.lock().unwrap().push(message);
        }
    }

    fn enter(&self, _span: &tracing::span::Id) {}

    fn exit(&self, _span: &tracing::span::Id) {}
}
