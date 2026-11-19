use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::Arc;

pub struct Callback<Input, Output = ()> {
    inner: Arc<dyn Erased<Input, Output> + Send + Sync>,
}

impl<Input, Output> Callback<Input, Output>
where
    Input: Send + 'static,
    Output: Send + 'static,
{
    pub fn from_sync(callback: impl Fn(Input) -> Output + Send + Sync + 'static) -> Self {
        Self::from_fn::<_, SyncKind>(callback)
    }

    pub fn from_async<F, Fut>(callback: F) -> Self
    where
        F: Fn(Input) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Output> + Send + 'static,
    {
        Self::from_fn::<_, AsyncKind>(callback)
    }

    pub fn from_fn<F, Kind>(callback: F) -> Self
    where
        F: Call<Input, Output, Kind> + 'static,
        Kind: Send + Sync + 'static,
    {
        Self {
            inner: Arc::new(Stored {
                callback,
                _kind: PhantomData,
            }),
        }
    }

    pub fn call(&self, input: Input) -> impl Future<Output = Output> + Send + '_ {
        self.inner.call(input)
    }
}

impl<Input, Output> Clone for Callback<Input, Output> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

pub struct SyncKind;
pub struct AsyncKind;

pub trait Call<Input, Output, Kind>: Send + Sync {
    fn call(&self, input: Input) -> Pin<Box<dyn Future<Output = Output> + Send + '_>>;
}

impl<Input, Output, F> Call<Input, Output, SyncKind> for F
where
    Input: Send + 'static,
    Output: Send + 'static,
    F: Fn(Input) -> Output + Send + Sync,
{
    fn call(&self, input: Input) -> Pin<Box<dyn Future<Output = Output> + Send + '_>> {
        Box::pin(async move { (self)(input) })
    }
}

impl<Input, Output, F, Fut> Call<Input, Output, AsyncKind> for F
where
    Input: Send + 'static,
    F: Fn(Input) -> Fut + Send + Sync,
    Fut: Future<Output = Output> + Send + 'static,
{
    fn call(&self, input: Input) -> Pin<Box<dyn Future<Output = Output> + Send + '_>> {
        Box::pin(async move { (self)(input).await })
    }
}

trait Erased<Input, Output>: Send + Sync {
    fn call(&self, input: Input) -> Pin<Box<dyn Future<Output = Output> + Send + '_>>;
}

struct Stored<F, Kind> {
    callback: F,
    _kind: PhantomData<Kind>,
}

impl<Input, Output, F, Kind> Erased<Input, Output> for Stored<F, Kind>
where
    F: Call<Input, Output, Kind>,
    Kind: Send + Sync,
{
    fn call(&self, input: Input) -> Pin<Box<dyn Future<Output = Output> + Send + '_>> {
        Call::<Input, Output, Kind>::call(&self.callback, input)
    }
}
