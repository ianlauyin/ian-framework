use std::sync;
use std::time::Duration;

use crate::exception::Exception;
use crate::logging::LogEvent;
use crate::logging::log_shipper::LogShipper;
use crate::logging::mask::Masks;
use crate::mutex;
use crate::shutdown::ShutdownReceiver;

pub mod installed;
pub mod log_buffer;
pub mod trace_event;

pub use installed::LOGGER_ALREADY_SET;

const SHIP_EVERY: Duration = Duration::from_secs(60);
const FLUSH_RETRY_EVERY: Duration = Duration::from_secs(1);
const FLUSH_RETRY_LIMIT: u8 = 3;

#[derive(Clone)]
pub struct Logger {
    inner: sync::Arc<Inner>,
}

impl Logger {
    pub fn new(
        shipper: impl LogShipper + 'static,
        mask_patterns: Vec<String>,
    ) -> Result<Self, Exception> {
        Ok(Self {
            inner: sync::Arc::new(Inner {
                shipper: Box::new(shipper),
                masks: Masks::new(&mask_patterns)?,
                buffer: sync::Mutex::new(log_buffer::LogBuffer::new()),
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

    pub fn record(&self, mut event: LogEvent) {
        self.inner.masks.mask_event(&mut event);
        self.lock_buffer().push(event);
    }

    pub async fn direct_send(mut events: Vec<LogEvent>) -> Result<(), Exception> {
        let Some(logger) = Logger::get() else {
            events.iter().for_each(trace_event::trace_event);
            return Ok(());
        };
        for event in &mut events {
            logger.inner.masks.mask_event(event);
        }
        logger.inner.shipper.ship(&events).await
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
                    _ = ticker.tick() => logger.send_all().await,
                }
            }
            logger.flush().await;
        })
    }

    pub fn lock_buffer(&self) -> sync::MutexGuard<'_, log_buffer::LogBuffer> {
        mutex::lock(&self.inner.buffer)
    }

    pub fn buffered_len(&self) -> usize {
        self.lock_buffer().len()
    }

    pub async fn send_buffer(&self) -> Result<usize, Exception> {
        let _shipping = self.inner.shipping.lock().await;
        let batch = self.lock_buffer().take_chunk(log_buffer::SHIP_CHUNK);
        if batch.is_empty() {
            return Ok(0);
        }
        let sent = batch.len();
        if let Err(error) = self.inner.shipper.ship(&batch).await {
            self.lock_buffer().restore(batch);
            return Err(error);
        }
        self.lock_buffer().shipped();
        Ok(sent)
    }

    pub async fn send_all(&self) {
        while self.send_buffer().await.is_ok_and(|sent| sent > 0) {}
    }

    async fn flush(&self) {
        let mut failed = 0u8;
        loop {
            match self.send_buffer().await {
                Ok(0) => return,
                Ok(_) => {}
                Err(_) => {
                    failed += 1;
                    if failed >= FLUSH_RETRY_LIMIT {
                        let buffered = self.buffered_len();
                        if buffered > 0 {
                            tracing::error!(
                                buffered,
                                "cannot flush buffered log events before shutdown"
                            );
                        }
                        return;
                    }
                    tokio::time::sleep(FLUSH_RETRY_EVERY).await;
                }
            }
        }
    }
}

struct Inner {
    shipper: Box<dyn LogShipper>,
    masks: Masks,
    buffer: sync::Mutex<log_buffer::LogBuffer>,
    shipping: tokio::sync::Mutex<()>,
}
