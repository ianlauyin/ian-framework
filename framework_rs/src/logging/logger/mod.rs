pub(crate) mod inner;
pub(crate) mod installed;
pub(crate) mod log_buffer;
pub(crate) mod ship;
pub(crate) mod trace_event;

use std::collections::BTreeMap;
use std::error::Error;
use std::sync::{Arc, Mutex};

use crate::exception::Exception;
use crate::logging::log_shipper::{HttpLogShipper, LogShipper, SHIP_CHUNK};
use crate::logging::mask::Masks;
use crate::logging::{LogEvent, LogValue};
use crate::shutdown::ShutdownReceiver;

pub use installed::LOGGER_ALREADY_SET;

#[derive(Clone)]
pub struct Logger {
    inner: Arc<inner::Inner>,
}

impl Logger {
    pub fn new(shipper: HttpLogShipper, mask_patterns: Vec<String>) -> Result<Self, Exception> {
        Ok(Self {
            inner: Arc::new(inner::Inner {
                shipper,
                masks: Masks::new(&mask_patterns)?,
                buffer: Mutex::new(log_buffer::LogBuffer::new()),
                shipping: tokio::sync::Mutex::new(()),
            }),
        })
    }

    pub fn init(logger: Logger) -> Result<(), Exception> {
        installed::set(logger)
    }

    pub fn get() -> Option<&'static Logger> {
        installed::get()
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

    pub async fn direct_send(events: &[LogEvent]) -> Result<(), Exception> {
        let Some(logger) = Logger::get() else {
            return Ok(());
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
            let mut ticker = tokio::time::interval(ship::SHIP_EVERY);
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

    pub(crate) fn lock_buffer(&self) -> std::sync::MutexGuard<'_, log_buffer::LogBuffer> {
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
        if self.inner.shipper.ship(&batch).await.is_err() {
            self.lock_buffer().restore(batch);
        } else {
            self.lock_buffer().shipped();
        }
    }

    async fn flush(&self) {
        let mut failed = 0u8;
        while self.buffered_len() > 0 {
            let before = self.buffered_len();
            self.send_buffer().await;
            if self.buffered_len() < before {
                continue;
            }
            failed += 1;
            if failed >= ship::FLUSH_RETRY_LIMIT {
                return;
            }
            tokio::time::sleep(ship::FLUSH_RETRY_EVERY).await;
        }
    }

    fn log(mut log_event: LogEvent, context: BTreeMap<String, LogValue>) {
        log_event.context = context;
        match Logger::get() {
            Some(logger) => logger.record(log_event),
            None => trace_event::trace_event(&log_event),
        }
    }
}
