pub(crate) mod log_buffer;

use std::collections::BTreeMap;
use std::error::Error;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use log_buffer::LogBuffer;

use crate::exception::Exception;
use crate::logging::log_shipper::{HttpLogShipper, LogShipper, SHIP_CHUNK};
use crate::logging::mask::Masks;
use crate::logging::{LogEvent, LogLevel, LogValue};
use crate::shutdown::ShutdownReceiver;

pub(crate) const SHIP_EVERY: Duration = Duration::from_secs(60);

const FLUSH_RETRY_EVERY: Duration = Duration::from_secs(1);

static LOGGER: OnceLock<Logger> = OnceLock::new();

#[derive(Clone)]
pub struct Logger {
    inner: Arc<Inner>,
}

impl Logger {
    pub fn new(shipper: HttpLogShipper, mask_patterns: Vec<String>) -> Result<Self, Exception> {
        Ok(Self {
            inner: Arc::new(Inner {
                shipper,
                masks: Masks::new(&mask_patterns)?,
                buffer: Mutex::new(LogBuffer::new()),
                shipping: tokio::sync::Mutex::new(()),
            }),
        })
    }

    pub fn init(logger: Logger) -> Result<(), Logger> {
        LOGGER.set(logger)
    }

    pub fn get() -> Option<&'static Logger> {
        LOGGER.get()
    }

    pub fn info(action: impl Into<String>, context: BTreeMap<String, LogValue>) {
        Self::log(LogEvent::info(action), context);
    }

    pub fn warn(action: impl Into<String>, context: BTreeMap<String, LogValue>) {
        Self::log(LogEvent::warn(action), context);
    }

    pub fn warn_with(
        action: impl Into<String>,
        context: BTreeMap<String, LogValue>,
        error: Box<dyn Error>,
    ) {
        Self::log(LogEvent::warn_with(action, error.to_string()), context);
    }

    pub fn error(
        action: impl Into<String>,
        context: BTreeMap<String, LogValue>,
        error: Box<dyn Error>,
    ) {
        let error_message = match error.downcast::<Exception>() {
            Ok(exception) => {
                let message = exception.to_string();
                let trace = String::from(exception.as_ref());
                format!("{message}\n{trace}")
            }
            Err(error) => error.to_string(),
        };
        Self::log(LogEvent::error(action, error_message), context);
    }

    pub fn record(&self, mut event: LogEvent) {
        self.inner.masks.mask_event(&mut event);
        self.lock_buffer().push(event);
    }

    pub async fn direct_send(events: &[LogEvent]) -> bool {
        let Some(logger) = Logger::get() else {
            return true;
        };
        let mut masked = events.to_vec();
        for event in &mut masked {
            logger.inner.masks.mask_event(event);
        }
        logger.inner.shipper.ship(&masked).await
    }

    pub fn spawn_ship_loop(&self, shutdown: ShutdownReceiver) -> tokio::task::JoinHandle<()> {
        let logger = self.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(SHIP_EVERY);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            let stop = shutdown.wait();
            tokio::pin!(stop);
            loop {
                tokio::select! {
                    biased;
                    _ = &mut stop => break,
                    _ = ticker.tick() => logger.send_buffer().await,
                }
            }
            logger.flush().await;
        })
    }

    pub(crate) fn lock_buffer(&self) -> std::sync::MutexGuard<'_, LogBuffer> {
        self.inner
            .buffer
            .lock()
            .unwrap_or_else(|err| err.into_inner())
    }

    pub(crate) fn buffered_len(&self) -> usize {
        self.lock_buffer().len()
    }

    pub(crate) async fn send_buffer(&self) {
        let _shipping = self.inner.shipping.lock().await;
        let batch = self.lock_buffer().take_chunk(SHIP_CHUNK);
        if batch.is_empty() {
            return;
        }
        if !self.inner.shipper.ship(&batch).await {
            self.lock_buffer().restore(batch);
        } else {
            self.lock_buffer().shipped();
        }
    }

    // Shutdown waits until the buffer is empty. A failed batch is retried after a pause.
    async fn flush(&self) {
        while self.buffered_len() > 0 {
            let before = self.buffered_len();
            self.send_buffer().await;
            if self.buffered_len() >= before {
                tokio::time::sleep(FLUSH_RETRY_EVERY).await;
            }
        }
    }

    fn log(mut log_event: LogEvent, context: BTreeMap<String, LogValue>) {
        log_event.context = context;
        match Logger::get() {
            Some(logger) => logger.record(log_event),
            None => trace_event(&log_event),
        }
    }
}

struct Inner {
    shipper: HttpLogShipper,
    masks: Masks,
    buffer: Mutex<LogBuffer>,
    shipping: tokio::sync::Mutex<()>,
}

fn trace_event(log_event: &LogEvent) {
    let context = serde_json::to_string(&log_event.context).unwrap_or_default();
    match log_event.level {
        LogLevel::Info => tracing::info!(action = %log_event.action, context = %context),
        LogLevel::Warn => tracing::warn!(
            action = %log_event.action,
            error_message = ?log_event.error_message,
            context = %context
        ),
        LogLevel::Error => tracing::error!(
            action = %log_event.action,
            error_message = ?log_event.error_message,
            context = %context
        ),
    }
}
