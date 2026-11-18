use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::callback;

fn double(value: u32) -> u32 {
    value * 2
}

async fn value_after_wait(value: u32) -> u32 {
    tokio::task::yield_now().await;
    value
}

#[tokio::test]
async fn calls_a_sync_function() {
    let callback = callback::Callback::new(double);
    assert_eq!(callback.call(21u32).await, 42);
}

#[tokio::test]
async fn an_async_function_uses_the_input_after_it_waits() {
    let callback = callback::Callback::from_async(value_after_wait);
    assert_eq!(callback.call(4u32).await, 4);
}

#[tokio::test]
async fn calls_the_same_callback_again() {
    let count = Arc::new(AtomicUsize::new(0));
    let calls = count.clone();
    let callback = callback::Callback::new(move |_| {
        calls.fetch_add(1, Ordering::SeqCst);
    });
    callback.call(0u8).await;
    callback.call(0u8).await;
    assert_eq!(count.load(Ordering::SeqCst), 2);
}
