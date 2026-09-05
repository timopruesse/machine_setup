use crate::error::{Error, Result};

/// Run sync host work off Tokio worker threads.
pub async fn run<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| Error::TaskJoin(e.to_string()))
}
