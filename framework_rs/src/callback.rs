use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::Arc;

pub struct Callback<T, R = ()> {
    inner: Arc<dyn Erased<T, R> + Send + Sync>,
}

impl<T, R> Callback<T, R>
where
    T: Send + 'static,
    R: Send + 'static,
{
    pub fn new(callback: impl Fn(T) -> R + Send + Sync + 'static) -> Self {
        Self::from_fn::<_, SyncKind>(callback)
    }

    pub fn from_async<F, Fut>(callback: F) -> Self
    where
        F: Fn(T) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = R> + Send + 'static,
    {
        Self::from_fn::<_, AsyncKind>(callback)
    }

    pub fn from_fn<F, Kind>(callback: F) -> Self
    where
        F: Call<T, R, Kind> + 'static,
        Kind: Send + Sync + 'static,
    {
        Self {
            inner: Arc::new(Stored {
                callback,
                _kind: PhantomData,
            }),
        }
    }

    pub async fn call(&self, input: T) -> R {
        self.inner.call(input).await
    }
}

impl<T, R> Clone for Callback<T, R> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

pub enum SyncKind {}
pub enum AsyncKind {}

pub trait Call<T, R, Kind>: Send + Sync {
    fn call(&self, input: T) -> Pin<Box<dyn Future<Output = R> + Send>>;
}

impl<T, R, F> Call<T, R, SyncKind> for F
where
    T: 'static,
    R: Send + 'static,
    F: Fn(T) -> R + Send + Sync,
{
    fn call(&self, input: T) -> Pin<Box<dyn Future<Output = R> + Send>> {
        Box::pin(std::future::ready((self)(input)))
    }
}

impl<T, R, F, Fut> Call<T, R, AsyncKind> for F
where
    F: Fn(T) -> Fut + Send + Sync,
    Fut: Future<Output = R> + Send + 'static,
{
    fn call(&self, input: T) -> Pin<Box<dyn Future<Output = R> + Send>> {
        Box::pin((self)(input))
    }
}

trait Erased<T, R>: Send + Sync {
    fn call(&self, input: T) -> Pin<Box<dyn Future<Output = R> + Send>>;
}

struct Stored<F, Kind> {
    callback: F,
    _kind: PhantomData<Kind>,
}

impl<T, R, F, Kind> Erased<T, R> for Stored<F, Kind>
where
    F: Call<T, R, Kind>,
    Kind: Send + Sync,
{
    fn call(&self, input: T) -> Pin<Box<dyn Future<Output = R> + Send>> {
        Call::<T, R, Kind>::call(&self.callback, input)
    }
}
