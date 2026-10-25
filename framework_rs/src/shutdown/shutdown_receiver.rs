use std::future::Future;

use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::watch;

#[derive(Clone, Debug)]
pub struct ShutdownReceiver {
    signal: watch::Sender<bool>,
}

impl ShutdownReceiver {
    // `new` spawns the signal listener, which a `Default` value should not do.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let (signal_tx, _) = watch::channel(false);
        let receiver = Self { signal: signal_tx };
        let listener = receiver.clone();
        tokio::spawn(async move {
            let mut sigterm = signal(SignalKind::terminate()).ok();
            // A listener that fails to install disables its branch. With no branch left, the
            // task ends without a shutdown.
            tokio::select! {
                Ok(()) = tokio::signal::ctrl_c() => {}
                Some(()) = async { sigterm.as_mut()?.recv().await } => {}
                else => return,
            }
            tracing::info!("shutdown signal received");
            listener.trigger();
        });
        receiver
    }

    // The future owns its own watch receiver, so it is 'static and can go to
    // `with_graceful_shutdown` or a `select!` in a loop without a clone.
    pub fn wait(&self) -> impl Future<Output = ()> + Send + 'static {
        let mut signal = self.signal.subscribe();
        async move {
            let _ = signal.wait_for(|down| *down).await;
        }
    }

    pub fn is_shutdown(&self) -> bool {
        *self.signal.borrow()
    }

    pub fn trigger(&self) {
        self.signal.send_replace(true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
