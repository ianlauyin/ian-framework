use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::shutdown::ShutdownReceiver;

use super::mask::{Masks, MASK};
use super::{LogEvent, ShipperConfig, SHIP_CHUNK};

pub(crate) const SHIP_TIMEOUT: Duration = Duration::from_secs(10);
pub(crate) const CAPACITY_WARN_NOTE: &str = "SHIPPER/capacityMeets80%";

const CAPACITY: usize = 10_000;
const SHIP_EVERY: Duration = Duration::from_secs(5);

#[derive(Clone)]
pub struct LogShipper {
    pub(crate) inner: Arc<Inner>,
}

impl LogShipper {
    pub fn new(config: ShipperConfig) -> Self {
        let mut post_api_key =
            reqwest::header::HeaderValue::from_str(&format!("Bearer {}", config.post_api_key))
                .expect("post API key is not a valid header value");
        post_api_key.set_sensitive(true);
        let masks = Masks::new(&config.mask_patterns);
        let http = ship_client().expect("log shipper HTTP client cannot be built");
        tracing::info!(app = %config.app, "log shipping posts to the log server");
        Self {
            inner: Arc::new(Inner {
                app: config.app,
                endpoint: config.endpoint,
                post_api_key,
                masks,
                buffer: Mutex::new(Buffer {
                    events: VecDeque::new(),
                    capacity_warned: false,
                }),
                http,
                shipping: tokio::sync::Mutex::new(()),
            }),
        }
    }

    pub fn record(&self, mut event: LogEvent) {
        self.inner.masks.mask_event(&mut event);
        let mut buffer = self.lock_buffer();
        while buffer.events.len() >= CAPACITY {
            buffer.events.pop_front();
        }
        buffer.events.push_back(event);
        if !buffer.capacity_warned && meets_capacity_warn(buffer.events.len(), CAPACITY) {
            while buffer.events.len() >= CAPACITY {
                buffer.events.pop_front();
            }
            buffer.events.push_back(capacity_warn_event());
            buffer.capacity_warned = true;
        }
    }

    pub fn spawn_ship_loop(&self, shutdown: ShutdownReceiver) -> tokio::task::JoinHandle<()> {
        let shipper = self.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(SHIP_EVERY);
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

    pub(crate) fn lock_buffer(&self) -> std::sync::MutexGuard<'_, Buffer> {
        self.inner
            .buffer
            .lock()
            .unwrap_or_else(|err| err.into_inner())
    }

    pub(crate) fn buffered_len(&self) -> usize {
        self.lock_buffer().events.len()
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

    pub(crate) async fn ship_batch(&self) {
        let _shipping = self.inner.shipping.lock().await;
        let batch = {
            let mut buffer = self.lock_buffer();
            let count = SHIP_CHUNK.min(buffer.events.len());
            buffer.events.drain(..count).collect::<Vec<_>>()
        };
        if batch.is_empty() {
            return;
        }
        let client = self.inner.http.clone();
        let url = self.batch_url();
        let posted = match serde_json::to_string(&batch) {
            Ok(body) => {
                let request = client
                    .post(&url)
                    .timeout(SHIP_TIMEOUT)
                    .header(reqwest::header::CONTENT_TYPE, "application/json")
                    .header(
                        reqwest::header::AUTHORIZATION,
                        self.inner.post_api_key.clone(),
                    );
                match request.body(body).send().await {
                    Ok(response) => response.status().is_success(),
                    Err(_) => false,
                }
            }
            Err(_) => false,
        };
        if !posted {
            self.restore_batch(batch);
        } else {
            let mut buffer = self.lock_buffer();
            if !meets_capacity_warn(buffer.events.len(), CAPACITY) {
                buffer.capacity_warned = false;
            }
        }
    }

    fn restore_batch(&self, batch: Vec<LogEvent>) {
        let mut buffer = self.lock_buffer();
        for event in batch.into_iter().rev() {
            buffer.events.push_front(event);
        }
        while buffer.events.len() > CAPACITY {
            buffer.events.pop_back();
        }
    }

    fn batch_url(&self) -> String {
        format!("{}/log/{}", self.inner.endpoint, self.inner.app)
    }
}

impl std::fmt::Debug for LogShipper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LogShipper")
            .field("app", &self.inner.app)
            .field("endpoint", &self.inner.endpoint)
            .field("post_api_key", &MASK)
            .field("buffered", &self.buffered_len())
            .finish_non_exhaustive()
    }
}

pub(crate) struct Buffer {
    pub(crate) events: VecDeque<LogEvent>,
    capacity_warned: bool,
}

pub(crate) struct Inner {
    pub(crate) app: String,
    pub(crate) endpoint: String,
    pub(crate) post_api_key: reqwest::header::HeaderValue,
    pub(crate) masks: Masks,
    buffer: Mutex<Buffer>,
    pub(crate) http: reqwest::Client,
    shipping: tokio::sync::Mutex<()>,
}

fn ship_client() -> Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder()
        .connect_timeout(SHIP_TIMEOUT)
        .timeout(SHIP_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("ian_framework/", env!("CARGO_PKG_VERSION")))
        .build()
}

fn meets_capacity_warn(len: usize, capacity: usize) -> bool {
    (len as u128) * 5 >= (capacity as u128) * 4
}

fn capacity_warn_event() -> LogEvent {
    LogEvent::warn(CAPACITY_WARN_NOTE)
}
