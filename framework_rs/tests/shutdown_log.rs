use std::time::Duration;

use axum::Router;
use axum::extract::{Json, State};
use axum::http::StatusCode;
use axum::routing::post;
use tokio::net::TcpListener;

use ian_framework_rs::logging::{HttpLogShipper, LogLevel, LogRequest, Logger};
use ian_framework_rs::shutdown::ShutdownReceiver;

#[tokio::test]
async fn first_trigger_ships_one_shutdown_event() {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let app = Router::new()
        .route(
            "/log/api",
            post(
                |State(tx): State<tokio::sync::mpsc::UnboundedSender<LogRequest>>,
                 Json(batch): Json<LogRequest>| async move {
                    tx.send(batch).unwrap();
                    StatusCode::NO_CONTENT
                },
            ),
        )
        .with_state(tx);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    let logger = Logger::new(
        HttpLogShipper::new("api", endpoint, "test-key").unwrap(),
        vec![],
    )
    .unwrap();
    Logger::init(logger.clone()).unwrap();
    let shutdown = ShutdownReceiver::new();
    let ship_task = logger.spawn_ship_loop(shutdown.clone());
    shutdown.trigger();
    shutdown.clone().trigger();

    tokio::time::timeout(Duration::from_secs(2), ship_task)
        .await
        .unwrap()
        .unwrap();
    let events = rx.try_recv().unwrap().events;
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].action, "SHUTDOWN/started");
    assert_eq!(events[0].level, LogLevel::Info);
    assert!(rx.try_recv().is_err());
    assert_eq!(logger.buffered_len(), 0);
}
