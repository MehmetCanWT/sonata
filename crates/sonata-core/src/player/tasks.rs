//! Cancellation-scoped async task management for playback resolution and precaching.
//!
//! 1. **PlaybackTask**: Detached result handle supporting non-blocking `try_take` and explicit `abort` across restarts.
//! 2. **PlaybackTaskSet**: `CancellationToken`-scoped `JoinSet` providing atomic `restart` and bounded `shutdown`.

use std::future::Future;
use std::time::Duration;
use tokio::sync::oneshot;
use tokio::task::{AbortHandle, JoinSet};
use tokio_util::sync::CancellationToken;

pub struct PlaybackTask<T> {
    abort: AbortHandle,
    receiver: oneshot::Receiver<T>,
}

impl<T> PlaybackTask<T> {
    /// Spawn work that can survive a foreground playback-task restart.
    ///
    /// Next-track precache uses this while its target becomes the current
    /// track. Callers own cancellation through [`Self::abort`].
    pub fn spawn_detached<F>(future: F) -> Self
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        let (sender, receiver) = oneshot::channel();
        let join = tokio::spawn(async move {
            let _ = sender.send(future.await);
        });
        Self {
            abort: join.abort_handle(),
            receiver,
        }
    }

    pub fn try_take(&mut self) -> Option<T> {
        self.receiver.try_recv().ok()
    }

    pub fn abort(self) {
        self.abort.abort();
    }
}

pub struct PlaybackTaskSet {
    cancellation: CancellationToken,
    tasks: JoinSet<()>,
}

impl Default for PlaybackTaskSet {
    fn default() -> Self {
        Self {
            cancellation: CancellationToken::new(),
            tasks: JoinSet::new(),
        }
    }
}

impl PlaybackTaskSet {
    pub fn restart(&mut self) {
        self.cancellation.cancel();
        self.tasks.abort_all();
        self.tasks = JoinSet::new();
        self.cancellation = CancellationToken::new();
    }

    pub fn spawn<F>(&mut self, future: F) -> AbortHandle
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let cancellation = self.cancellation.clone();
        self.tasks.spawn(async move {
            tokio::select! {
                _ = cancellation.cancelled() => {}
                _ = future => {}
            }
        })
    }

    pub fn spawn_result<F, T>(&mut self, future: F) -> PlaybackTask<T>
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        let (sender, receiver) = oneshot::channel();
        let abort = self.spawn(async move {
            let _ = sender.send(future.await);
        });
        PlaybackTask { abort, receiver }
    }

    pub fn pending_count(&self) -> usize {
        self.tasks.len()
    }

    pub async fn shutdown(&mut self, wait: Duration) -> bool {
        self.cancellation.cancel();
        let joined = tokio::time::timeout(wait, async {
            while self.tasks.join_next().await.is_some() {}
        })
        .await
        .is_ok();
        if !joined {
            self.tasks.abort_all();
            while self.tasks.join_next().await.is_some() {}
        }
        joined
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn shutdown_cancels_and_joins_pending_session_tasks() {
        let mut tasks = PlaybackTaskSet::default();
        tasks.spawn(std::future::pending());
        assert_eq!(tasks.pending_count(), 1);
        assert!(tasks.shutdown(Duration::from_millis(100)).await);
        assert_eq!(tasks.pending_count(), 0);
    }

    #[tokio::test]
    async fn restart_drops_old_results_and_accepts_new_session_work() {
        let mut tasks = PlaybackTaskSet::default();
        let old = tasks.spawn_result(std::future::pending::<u8>());
        tasks.restart();
        tokio::task::yield_now().await;
        assert!(old.receiver.await.is_err());

        let current = tasks.spawn_result(async { 7_u8 });
        assert_eq!(current.receiver.await, Ok(7));
        assert!(tasks.shutdown(Duration::from_millis(100)).await);
    }
}
