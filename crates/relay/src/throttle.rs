use std::future::Future;
use std::time::Duration;
use tokio::sync::{Mutex, Semaphore};
use tokio::time::Instant;

use crate::error::RelayError;

pub struct Throttle {
    open: Semaphore,
    next_start: Mutex<Instant>,
    spacing: Duration,
    max_wait: Duration,
}

impl Throttle {
    pub fn new(max_open: usize, spacing: Duration, max_wait: Duration) -> Self {
        Self {
            open: Semaphore::new(max_open),
            next_start: Mutex::new(Instant::now()),
            spacing,
            max_wait,
        }
    }

    pub async fn run<T>(&self, work: impl Future<Output = T>) -> Result<T, RelayError> {
        let admission = async {
            let permit = self
                .open
                .acquire()
                .await
                .map_err(|_| RelayError::Internal)?;
            let start = {
                let mut next_start = self.next_start.lock().await;
                let start = (*next_start).max(Instant::now());
                *next_start = start + self.spacing;
                start
            };
            tokio::time::sleep_until(start).await;
            Ok::<_, RelayError>(permit)
        };

        let permit = tokio::time::timeout(self.max_wait, admission)
            .await
            .map_err(|_| RelayError::UpstreamBusy)??;
        let output = work.await;
        drop(permit);
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::Throttle;
    use crate::error::RelayError;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;
    use tokio::time::Instant;

    #[tokio::test]
    async fn calls_are_spaced_apart() {
        let throttle = Throttle::new(8, Duration::from_millis(40), Duration::from_secs(5));
        let started = Instant::now();

        for _ in 0..4 {
            throttle.run(async {}).await.unwrap();
        }

        assert!(started.elapsed() >= Duration::from_millis(120));
    }

    #[tokio::test]
    async fn no_more_than_the_limit_run_at_once() {
        let throttle = Arc::new(Throttle::new(2, Duration::ZERO, Duration::from_secs(5)));
        let running = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));

        let tasks: Vec<_> = (0..6)
            .map(|_| {
                let throttle = throttle.clone();
                let running = running.clone();
                let peak = peak.clone();
                tokio::spawn(async move {
                    throttle
                        .run(async {
                            let now = running.fetch_add(1, Ordering::SeqCst) + 1;
                            peak.fetch_max(now, Ordering::SeqCst);
                            tokio::time::sleep(Duration::from_millis(20)).await;
                            running.fetch_sub(1, Ordering::SeqCst);
                        })
                        .await
                })
            })
            .collect();
        for task in tasks {
            task.await.unwrap().unwrap();
        }

        assert_eq!(peak.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn a_long_queue_gives_up_as_busy() {
        let throttle = Throttle::new(1, Duration::from_millis(200), Duration::from_millis(50));
        throttle.run(async {}).await.unwrap();

        assert_eq!(
            throttle.run(async {}).await.unwrap_err(),
            RelayError::UpstreamBusy
        );
    }
}
