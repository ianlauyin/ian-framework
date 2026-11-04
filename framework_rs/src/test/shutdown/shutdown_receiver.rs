use std::future::Future;

use crate::shutdown::shutdown_receiver::*;
use std::time::Duration;

async fn resolves(wait: impl Future<Output = ()>) -> bool {
    tokio::time::timeout(Duration::from_millis(100), wait)
        .await
        .is_ok()
}

#[tokio::test]
async fn wait_resolves_for_every_clone_after_trigger() {
    let shutdown = ShutdownReceiver::new();
    let before = shutdown.wait();
    let clone = shutdown.clone();
    assert!(!shutdown.is_shutdown());
    assert!(!resolves(shutdown.wait()).await);
    clone.trigger();
    assert!(shutdown.is_shutdown());
    assert!(clone.is_shutdown());
    assert!(resolves(before).await);
    assert!(resolves(shutdown.wait()).await);
    let late = shutdown.clone();
    assert!(resolves(late.wait()).await);
    clone.trigger();
    assert!(shutdown.is_shutdown());
}

#[tokio::test]
async fn wait_works_as_a_graceful_shutdown_future() {
    let shutdown = ShutdownReceiver::new();
    let task = tokio::spawn(shutdown.wait());
    shutdown.trigger();
    assert!(resolves(async { task.await.unwrap() }).await);
}
