use std::convert::Infallible;
use std::time::Duration;

use axum::response::sse::{Event, KeepAlive, Sse};
use futures_util::stream::{Stream, unfold};
use serde::Serialize;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;

use crate::shutdown::ShutdownReceiver;

pub fn broadcast_sse<T>(
    receiver: broadcast::Receiver<T>,
    shutdown: ShutdownReceiver,
    keep_alive: Duration,
) -> Sse<impl Stream<Item = Result<Event, Infallible>> + Send + 'static>
where
    T: Clone + Serialize + Send + 'static,
{
    let stream = unfold(
        (receiver, shutdown),
        |(mut receiver, shutdown)| async move {
            let event = next_event(&mut receiver, &shutdown).await?;
            Some((Ok(event), (receiver, shutdown)))
        },
    );
    Sse::new(stream).keep_alive(KeepAlive::new().interval(keep_alive))
}

async fn next_event<T: Clone + Serialize>(
    receiver: &mut broadcast::Receiver<T>,
    shutdown: &ShutdownReceiver,
) -> Option<Event> {
    let wait = shutdown.wait();
    tokio::pin!(wait);
    loop {
        let message = tokio::select! {
            message = receiver.recv() => message,
            () = &mut wait => return None,
        };
        match message {
            Ok(message) => {
                if let Ok(event) = Event::default().json_data(&message) {
                    return Some(event);
                }
            }
            Err(RecvError::Lagged(_)) => {}
            Err(RecvError::Closed) => return None,
        }
    }
}
