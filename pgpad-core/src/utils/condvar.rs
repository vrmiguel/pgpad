#![allow(clippy::new_without_default)]

use std::ops::Not;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Notify;

#[derive(Clone)]
pub struct Condvar {
    inner: Arc<CondvarInner>,
}

struct CondvarInner {
    is_set: AtomicBool,
    notify: Notify,
}

impl Condvar {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(CondvarInner {
                is_set: AtomicBool::new(false),
                notify: Notify::new(),
            }),
        }
    }

    pub fn set(&self) {
        let prev = self.inner.is_set.swap(true, Ordering::Release);
        if prev.not() {
            self.inner.notify.notify_waiters();
        }
    }

    pub async fn wait(&self) {
        // Condvar already set: return non-blocking
        if self.inner.is_set.load(Ordering::Acquire) {
            return;
        }

        loop {
            let notified = self.inner.notify.notified();

            if self.inner.is_set.load(Ordering::Acquire) {
                return;
            }

            notified.await;

            if self.inner.is_set.load(Ordering::Acquire) {
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use futures_util::FutureExt;

    use super::*;

    #[tokio::test]
    async fn test_condvar_now_or_never() {
        let condvar = Condvar::new();

        for _ in 0..10 {
            let condvar = condvar.clone();
            let wait_fut = condvar.wait();
            let poll_res = wait_fut.now_or_never();
            assert!(poll_res.is_none());
        }

        condvar.set();
        for _ in 0..10 {
            let condvar = condvar.clone();
            let wait_fut = condvar.wait();
            let poll_res = wait_fut.now_or_never();
            assert!(poll_res.is_some());
        }

        condvar.set();
        for _ in 0..10 {
            let condvar = condvar.clone();
            let wait_fut = condvar.wait();
            let poll_res = wait_fut.now_or_never();
            assert!(poll_res.is_some());
        }
    }

    #[tokio::test]
    async fn test_condvar_task() {
        let condvar = Condvar::new();

        let reader_handles: [tokio::task::JoinHandle<()>; 10] = std::array::from_fn(|_| {
            let condvar_clone = condvar.clone();
            tokio::task::spawn(async move {
                let now = Instant::now();
                condvar_clone.wait().await;
                let elapsed = now.elapsed();
                assert!(elapsed >= Duration::from_millis(100));
            })
        });

        let writer_handle = tokio::task::spawn(async move {
            tokio::time::sleep(Duration::from_millis(102)).await;
            condvar.set();
        });

        for handle in reader_handles {
            handle.await.unwrap();
        }
        writer_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_condvar_set_immediately() {
        for _ in 0..100 {
            let condvar = Condvar::new();
            let condvar_clone = condvar.clone();

            let waiter = tokio::spawn(async move {
                condvar_clone.wait().await;
            });

            condvar.set();

            tokio::time::timeout(Duration::from_millis(100), waiter)
                .await
                .expect("waiter should not hang!")
                .unwrap();
        }
    }
}
