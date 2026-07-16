use tokio::select;
use tokio_util::sync::CancellationToken;

// Accounting for cancellation in Err variant to ease the use of ? operator
pub type CancellableResult<T, E> = Result<T, Option<E>>;

pub async fn unless_cancelled<T, E>(fut: impl Future<Output=Result<T, E>>, tok: &CancellationToken)
    -> CancellableResult<T, E>
{
    select! {
        biased;
        res = fut => res.map_err(|e| Some(e)),
        _ = tok.cancelled() => Err(None),
    }
}

pub async fn as_cancellable<T, E>(fut: impl Future<Output=Result<T, E>>)
-> CancellableResult<T, E>
{
    fut.await.map_err(|e| Some(e))
}
