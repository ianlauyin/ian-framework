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
    let callback = callback::Callback::from_sync(double);
    assert_eq!(callback.call(21u32).await, 42);
}

#[tokio::test]
async fn an_async_function_uses_the_input_after_it_waits() {
    let callback = callback::Callback::from_async(value_after_wait);
    assert_eq!(callback.call(4u32).await, 4);
}

#[tokio::test]
async fn a_sync_function_runs_when_the_caller_awaits_call() {
    let count = Arc::new(AtomicUsize::new(0));
    let calls = count.clone();
    let callback = callback::Callback::from_sync(move |value: u32| {
        calls.fetch_add(1, Ordering::SeqCst);
        value
    });
    let pending = callback.call(7u32);
    assert_eq!(count.load(Ordering::SeqCst), 0);
    assert_eq!(pending.await, 7);
    assert_eq!(count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn a_sync_function_does_not_run_when_the_caller_drops_the_future() {
    let count = Arc::new(AtomicUsize::new(0));
    let calls = count.clone();
    let callback = callback::Callback::from_sync(move |_: u32| {
        calls.fetch_add(1, Ordering::SeqCst);
    });
    drop(callback.call(0u32));
    assert_eq!(count.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn an_async_function_runs_when_the_caller_awaits_call() {
    let count = Arc::new(AtomicUsize::new(0));
    let calls = count.clone();
    let callback = callback::Callback::from_async(move |value: u32| {
        calls.fetch_add(1, Ordering::SeqCst);
        async move { value }
    });
    let pending = callback.call(4u32);
    assert_eq!(count.load(Ordering::SeqCst), 0);
    assert_eq!(pending.await, 4);
    assert_eq!(count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn calls_the_same_callback_again() {
    let count = Arc::new(AtomicUsize::new(0));
    let calls = count.clone();
    let callback = callback::Callback::from_sync(move |_| {
        calls.fetch_add(1, Ordering::SeqCst);
    });
    callback.call(0u8).await;
    callback.call(0u8).await;
    assert_eq!(count.load(Ordering::SeqCst), 2);
}
