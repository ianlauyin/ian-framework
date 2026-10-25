use crate::logging::{LogBatch, LogEvent, SHIP_CHUNK};
use crate::shutdown::ShutdownReceiver;

use super::{meets_capacity_warn, LogShipper, SHIP_TIMEOUT};

impl LogShipper {
    pub fn spawn_ship_loop(&self, shutdown: ShutdownReceiver) -> tokio::task::JoinHandle<()> {
        let shipper = self.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(shipper.inner.ship_every);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            let stop = shutdown.wait();
            tokio::pin!(stop);
            loop {
                tokio::select! {
                    biased;
                    _ = &mut stop => break,
                    _ = ticker.tick() => shipper.ship_batch().await,
                }
            }
            shipper.flush().await;
        })
    }

    // Stop at the first batch that does not leave the buffer, so a down log
    // server does not hold the process open.
    async fn flush(&self) {
        loop {
            let before = self.buffered_len();
            if before == 0 {
                return;
            }
            self.ship_batch().await;
            if self.buffered_len() >= before {
                return;
            }
        }
    }

    async fn ship_batch(&self) {
        let _shipping = self.inner.shipping.lock().await;
        let Some(endpoint) = self.inner.endpoint.clone() else {
            return;
        };
        let batch = {
            let mut buffer = self.lock_buffer();
            let count = SHIP_CHUNK.min(buffer.events.len());
            buffer.events.drain(..count).collect::<Vec<_>>()
        };
        if batch.is_empty() {
            return;
        }
        let Some(client) = self.inner.http.clone() else {
            self.restore_batch(batch);
            return;
        };
        let url = self.batch_url(&endpoint);
        let posted = match serde_json::to_string(&LogBatch {
            events: batch.clone(),
        }) {
            Ok(body) => match client
                .post(&url)
                .timeout(SHIP_TIMEOUT)
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(body)
                .send()
                .await
            {
                Ok(response) => response.status().is_success(),
                Err(_) => false,
            },
            Err(_) => false,
        };
        if !posted {
            self.restore_batch(batch);
        } else {
            let mut buffer = self.lock_buffer();
            if !meets_capacity_warn(buffer.events.len(), self.inner.capacity) {
                buffer.capacity_warned = false;
            }
        }
    }

    fn restore_batch(&self, batch: Vec<LogEvent>) {
        let capacity = self.inner.capacity;
        let mut buffer = self.lock_buffer();
        for event in batch.into_iter().rev() {
            buffer.events.push_front(event);
        }
        while buffer.events.len() > capacity {
            buffer.events.pop_back();
        }
    }

    fn batch_url(&self, endpoint: &str) -> String {
        format!("{endpoint}/log/{}", self.inner.app)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logging::log_shipper::CAPACITY_WARN_NOTE;
    use crate::logging::{event, LogLevel, ShipperConfig};
    use std::time::Duration;

    fn config(app: &str, endpoint: impl Into<String>) -> ShipperConfig {
        let mut config = ShipperConfig::new(app);
        config.endpoint = Some(endpoint.into());
        config
    }

    fn trace_only() -> LogShipper {
        LogShipper::new(config("api", "")).unwrap()
    }

    fn messages_of(events: &[LogEvent]) -> Vec<&str> {
        events.iter().map(|event| event.message.as_str()).collect()
    }

    fn capacity_warn_count(events: &[LogEvent]) -> usize {
        events
            .iter()
            .filter(|event| event.message == CAPACITY_WARN_NOTE)
            .count()
    }

    fn assert_capacity_warn(event: &LogEvent) {
        assert_eq!(event.level, LogLevel::Warn);
        assert_eq!(event.message, CAPACITY_WARN_NOTE);
        assert_eq!(event.id, "");
        assert_eq!(event.ip, "");
        assert_eq!(event.user_agent, "");
        assert_eq!(event.action, "");
        assert_eq!(event.status, None);
        assert_eq!(event.latency_ms, None);
        assert!(event.extra.is_empty());
    }

    fn parse_body(body: &str) -> Vec<LogEvent> {
        serde_json::from_str::<LogBatch>(body).unwrap().events
    }

    #[test]
    fn batch_url_uses_the_app_path() {
        let shipper = LogShipper::new(config("cs", "http://logs.example/ingest/")).unwrap();
        assert_eq!(
            shipper.batch_url(shipper.inner.endpoint.as_deref().unwrap()),
            "http://logs.example/ingest/log/cs"
        );
    }

    #[tokio::test]
    async fn unset_endpoint_ships_without_http() {
        let shipper = trace_only();
        shipper.record(event("", "local", LogLevel::Info));
        shipper.ship_batch().await;
        assert_eq!(shipper.buffered_len(), 0);
        assert!(shipper.inner.http.is_none());
    }

    #[tokio::test]
    async fn empty_batch_does_not_post() {
        let config = config("api", "http://127.0.0.1:9");
        let shipper = LogShipper::new(config).unwrap();
        shipper.ship_batch().await;
        assert_eq!(shipper.buffered_len(), 0);
    }

    #[tokio::test]
    async fn unreachable_log_server_keeps_the_buffer() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        let config = config("api", format!("http://{addr}"));
        let shipper = LogShipper::new(config).unwrap();
        shipper.record(event("", "down", LogLevel::Error));
        shipper.ship_batch().await;
        assert_eq!(shipper.buffered_len(), 1);
        assert_eq!(shipper.buffered_events()[0].message, "down");
    }

    #[tokio::test]
    async fn ship_batch_posts_json_to_the_app_path() {
        let (endpoint, mut rx) = capture_server(StatusAction::NoContent).await;
        let config = config("cs", endpoint);
        let shipper = LogShipper::new(config).unwrap();
        shipper.record(event("", "one", LogLevel::Info));
        shipper.record(event("", "two", LogLevel::Warn));
        shipper.record(event("", "three", LogLevel::Error));
        shipper.ship_batch().await;
        assert_eq!(shipper.buffered_len(), 0);
        let captured = rx.recv().await.unwrap();
        assert_eq!(captured.app, "cs");
        assert_eq!(captured.path, "/log/cs");
        assert_eq!(captured.content_type, "application/json");
        let events = parse_body(&captured.body);
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].message, "one");
        assert_eq!(events[1].message, "two");
        assert_eq!(events[2].message, "three");
        let body: serde_json::Value = serde_json::from_str(&captured.body).unwrap();
        assert!(body.as_object().is_some());
        assert!(body.as_array().is_none());
        assert_eq!(body.as_object().unwrap().len(), 1);
        for item in body["events"].as_array().unwrap() {
            assert!(item.get("app").is_none());
            assert!(item.get("project").is_none());
            assert!(item.get("action").is_some());
            assert!(item.get("request").is_none());
        }
        assert!(serde_json::from_str::<Vec<LogEvent>>(&captured.body).is_err());
        shipper.ship_batch().await;
        assert_eq!(shipper.buffered_len(), 0);
    }

    #[tokio::test]
    async fn ship_batch_posts_one_chunk_and_leaves_the_rest() {
        let (endpoint, mut rx) = capture_server(StatusAction::NoContent).await;
        let shipper = LogShipper::new(config("api", endpoint)).unwrap();
        let total = SHIP_CHUNK + 1;
        for index in 0..total {
            shipper.record(event("", index.to_string(), LogLevel::Info));
        }
        shipper.ship_batch().await;
        assert_eq!(shipper.buffered_len(), 1);
        let captured = rx.recv().await.unwrap();
        let events = parse_body(&captured.body);
        assert_eq!(events.len(), SHIP_CHUNK);
        assert_eq!(events[0].message, "0");
        assert_eq!(events[SHIP_CHUNK - 1].message, (SHIP_CHUNK - 1).to_string());
        assert_eq!(shipper.buffered_events()[0].message, SHIP_CHUNK.to_string());
        shipper.ship_batch().await;
        assert_eq!(shipper.buffered_len(), 0);
        let rest = rx.recv().await.unwrap();
        let rest_events = parse_body(&rest.body);
        assert_eq!(rest_events.len(), 1);
        assert_eq!(rest_events[0].message, SHIP_CHUNK.to_string());
    }

    #[tokio::test]
    async fn http_error_restores_the_batch() {
        let (endpoint, _rx) = capture_server(StatusAction::Unavailable).await;
        let shipper = LogShipper::new(config("api", endpoint)).unwrap();
        shipper.record(event("", "keep", LogLevel::Info));
        shipper.ship_batch().await;
        assert_eq!(shipper.buffered_events()[0].message, "keep");
    }

    #[tokio::test]
    async fn in_flight_records_stay_out_of_a_successful_batch() {
        let (endpoint, _rx) = capture_server(StatusAction::Slow).await;
        let mut config = config("api", endpoint);
        config.capacity = 2;
        let shipper = LogShipper::new(config).unwrap();
        shipper.record(event("", "a", LogLevel::Info));
        shipper.record(event("", "b", LogLevel::Info));
        let shipper_task = shipper.clone();
        let ship = tokio::spawn(async move { shipper_task.ship_batch().await });
        tokio::time::timeout(Duration::from_secs(2), async {
            while shipper.buffered_len() != 0 {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        shipper.record(event("", "c", LogLevel::Info));
        ship.await.unwrap();
        let left = shipper.buffered_events();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].message, "c");
    }

    #[tokio::test]
    async fn failed_restore_keeps_the_oldest_batch() {
        let (endpoint, _rx) = capture_server(StatusAction::SlowUnavailable).await;
        let mut config = config("api", endpoint);
        config.capacity = 2;
        let shipper = LogShipper::new(config).unwrap();
        shipper.record(event("", "a", LogLevel::Info));
        shipper.record(event("", "b", LogLevel::Info));
        let shipper_task = shipper.clone();
        let ship = tokio::spawn(async move { shipper_task.ship_batch().await });
        tokio::time::timeout(Duration::from_secs(2), async {
            while shipper.buffered_len() != 0 {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        shipper.record(event("", "c", LogLevel::Info));
        ship.await.unwrap();
        let left: Vec<_> = shipper
            .buffered_events()
            .into_iter()
            .map(|event| event.message)
            .collect();
        assert_eq!(left, ["b".to_string(), CAPACITY_WARN_NOTE.to_string()]);
    }

    #[tokio::test]
    async fn trace_only_loop_keeps_running() {
        let mut config = config("api", "");
        config.ship_every = Duration::from_millis(10);
        let shipper = LogShipper::new(config).unwrap();
        shipper.record(event("", "traced", LogLevel::Info));
        let handle = shipper.spawn_ship_loop(ShutdownReceiver::new());
        tokio::time::sleep(Duration::from_millis(40)).await;
        assert!(!handle.is_finished());
        handle.abort();
        assert_eq!(shipper.buffered_len(), 0);
    }

    #[tokio::test]
    async fn shutdown_flushes_every_chunk_and_ends_the_loop() {
        let (endpoint, mut rx) = capture_server(StatusAction::NoContent).await;
        let shipper = LogShipper::new(config("api", endpoint)).unwrap();
        let shutdown = ShutdownReceiver::new();
        let handle = shipper.spawn_ship_loop(shutdown.clone());
        tokio::time::sleep(Duration::from_millis(20)).await;
        for index in 0..SHIP_CHUNK + 1 {
            shipper.record(event("", index.to_string(), LogLevel::Info));
        }
        shutdown.trigger();
        tokio::time::timeout(Duration::from_secs(2), handle)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(shipper.buffered_len(), 0);
        assert_eq!(parse_body(&rx.recv().await.unwrap().body).len(), SHIP_CHUNK);
        assert_eq!(parse_body(&rx.recv().await.unwrap().body).len(), 1);
    }

    #[tokio::test]
    async fn shutdown_flush_stops_when_the_log_server_fails() {
        let (endpoint, _rx) = capture_server(StatusAction::Unavailable).await;
        let shipper = LogShipper::new(config("api", endpoint)).unwrap();
        shipper.record(event("", "keep", LogLevel::Info));
        let shutdown = ShutdownReceiver::new();
        shutdown.trigger();
        let handle = shipper.spawn_ship_loop(shutdown);
        tokio::time::timeout(Duration::from_secs(2), handle)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(shipper.buffered_events()[0].message, "keep");
    }

    #[tokio::test]
    async fn failed_ship_does_not_repeat_the_capacity_warning() {
        let (endpoint, _rx) = capture_server(StatusAction::Unavailable).await;
        let mut config = config("api", endpoint);
        config.capacity = 5;
        let shipper = LogShipper::new(config).unwrap();
        for message in ["a", "b", "c", "d"] {
            shipper.record(event("", message, LogLevel::Info));
        }
        shipper.ship_batch().await;
        let events = shipper.buffered_events();
        assert_eq!(
            messages_of(&events),
            ["a", "b", "c", "d", CAPACITY_WARN_NOTE]
        );
        assert_eq!(events[0].level, LogLevel::Info);
        assert_capacity_warn(&events[4]);
        shipper.record(event("", "e", LogLevel::Info));
        let events = shipper.buffered_events();
        assert_eq!(
            messages_of(&events),
            ["b", "c", "d", CAPACITY_WARN_NOTE, "e"]
        );
        assert_eq!(capacity_warn_count(&events), 1);
        assert_eq!(events[0].message, "b");
        assert_eq!(events[0].level, LogLevel::Info);
    }

    #[tokio::test]
    async fn successful_ship_below_80_percent_warns_on_the_next_crossing() {
        let (endpoint, mut rx) = capture_server(StatusAction::NoContent).await;
        let mut config = config("api", endpoint);
        config.capacity = 5;
        let shipper = LogShipper::new(config).unwrap();
        for message in ["a", "b", "c", "d"] {
            shipper.record(event("", message, LogLevel::Info));
        }
        shipper.ship_batch().await;
        assert_eq!(shipper.buffered_len(), 0);
        let posted = parse_body(&rx.recv().await.unwrap().body);
        assert_eq!(
            messages_of(&posted),
            ["a", "b", "c", "d", CAPACITY_WARN_NOTE]
        );
        assert_eq!(posted[0].level, LogLevel::Info);
        assert_eq!(posted[3].message, "d");
        assert_capacity_warn(&posted[4]);
        for message in ["e", "f", "g"] {
            shipper.record(event("", message, LogLevel::Info));
        }
        let events = shipper.buffered_events();
        assert_eq!(messages_of(&events), ["e", "f", "g"]);
        assert_eq!(capacity_warn_count(&events), 0);
        shipper.record(event("", "h", LogLevel::Info));
        let events = shipper.buffered_events();
        assert_eq!(
            messages_of(&events),
            ["e", "f", "g", "h", CAPACITY_WARN_NOTE]
        );
        assert_eq!(events[0].level, LogLevel::Info);
        assert_capacity_warn(&events[4]);
        shipper.record(event("", "i", LogLevel::Info));
        let events = shipper.buffered_events();
        assert_eq!(
            messages_of(&events),
            ["f", "g", "h", CAPACITY_WARN_NOTE, "i"]
        );
        assert_eq!(capacity_warn_count(&events), 1);
    }

    #[tokio::test]
    async fn successful_ship_at_80_percent_does_not_warn_again() {
        let (endpoint, mut rx) = capture_server(StatusAction::NoContent).await;
        let mut config = config("api", endpoint);
        config.capacity = 5_000;
        let shipper = LogShipper::new(config).unwrap();
        for index in 0..4_000 {
            shipper.record(event("", index.to_string(), LogLevel::Info));
        }
        let events = shipper.buffered_events();
        assert_eq!(events.len(), 4_001);
        assert_eq!(events[0].message, "0");
        assert_eq!(events[0].level, LogLevel::Info);
        assert_eq!(capacity_warn_count(&events), 1);
        assert_capacity_warn(events.last().unwrap());
        for index in 4_000..4_999 {
            shipper.record(event("", index.to_string(), LogLevel::Info));
        }
        assert_eq!(shipper.buffered_len(), 5_000);
        assert_eq!(capacity_warn_count(&shipper.buffered_events()), 1);
        shipper.ship_batch().await;
        assert_eq!(shipper.buffered_len(), 4_000);
        let posted = parse_body(&rx.recv().await.unwrap().body);
        assert_eq!(posted.len(), SHIP_CHUNK);
        assert_eq!(posted[0].message, "0");
        assert_eq!(posted[0].level, LogLevel::Info);
        assert_eq!(capacity_warn_count(&posted), 0);
        let left = shipper.buffered_events();
        assert_eq!(left[0].message, "1000");
        assert_eq!(left[0].level, LogLevel::Info);
        assert_eq!(capacity_warn_count(&left), 1);
        shipper.record(event("", "more", LogLevel::Info));
        let left = shipper.buffered_events();
        assert_eq!(left.len(), 4_001);
        assert_eq!(left[0].message, "1000");
        assert_eq!(left.last().unwrap().message, "more");
        assert_eq!(left.last().unwrap().level, LogLevel::Info);
        assert_eq!(capacity_warn_count(&left), 1);
        shipper.ship_batch().await;
        assert_eq!(shipper.buffered_len(), 3_001);
        let second = parse_body(&rx.recv().await.unwrap().body);
        assert_eq!(second[0].message, "1000");
        assert_eq!(capacity_warn_count(&second), 0);
        assert_eq!(capacity_warn_count(&shipper.buffered_events()), 1);
        for index in 0..998 {
            shipper.record(event("", format!("extra-{index}"), LogLevel::Info));
        }
        assert_eq!(shipper.buffered_len(), 3_999);
        assert_eq!(capacity_warn_count(&shipper.buffered_events()), 1);
        shipper.record(event("", "cross", LogLevel::Info));
        let events = shipper.buffered_events();
        assert_eq!(events.len(), 4_001);
        assert_eq!(capacity_warn_count(&events), 2);
        assert_eq!(events[events.len() - 2].message, "cross");
        assert_eq!(events[events.len() - 2].level, LogLevel::Info);
        assert_capacity_warn(events.last().unwrap());
    }

    enum StatusAction {
        NoContent,
        Unavailable,
        Slow,
        SlowUnavailable,
    }

    struct Captured {
        app: String,
        path: String,
        content_type: String,
        body: String,
    }

    async fn capture_server(
        action: StatusAction,
    ) -> (String, tokio::sync::mpsc::UnboundedReceiver<Captured>) {
        use axum::extract::{Path, State};
        use axum::http::{HeaderMap, StatusCode, Uri};
        use axum::routing::post;
        use axum::Router;

        #[derive(Clone)]
        struct Capture {
            tx: tokio::sync::mpsc::UnboundedSender<Captured>,
            action: u8,
        }

        let action_code = match action {
            StatusAction::NoContent => 0,
            StatusAction::Unavailable => 1,
            StatusAction::Slow => 2,
            StatusAction::SlowUnavailable => 3,
        };
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let app = Router::new()
            .route(
                "/log/{app}",
                post(
                    |State(capture): State<Capture>,
                     Path(app): Path<String>,
                     uri: Uri,
                     headers: HeaderMap,
                     body: String| async move {
                        if capture.action == 2 || capture.action == 3 {
                            tokio::time::sleep(Duration::from_millis(200)).await;
                        }
                        let content_type = headers
                            .get(axum::http::header::CONTENT_TYPE)
                            .and_then(|value| value.to_str().ok())
                            .unwrap_or("")
                            .to_string();
                        let _ = capture.tx.send(Captured {
                            app,
                            path: uri.path().to_string(),
                            content_type,
                            body,
                        });
                        if capture.action == 1 || capture.action == 3 {
                            StatusCode::SERVICE_UNAVAILABLE
                        } else {
                            StatusCode::NO_CONTENT
                        }
                    },
                ),
            )
            .with_state(Capture {
                tx,
                action: action_code,
            });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        (format!("http://{addr}"), rx)
    }
}
