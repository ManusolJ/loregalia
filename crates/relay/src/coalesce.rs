use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex};
use tokio::sync::OnceCell;

use crate::error::RelayError;

type Shared = Arc<OnceCell<Result<String, RelayError>>>;

#[derive(Default)]
pub struct Coalescer {
    in_flight: Mutex<HashMap<String, Shared>>,
}

impl Coalescer {
    pub async fn run(
        &self,
        key: &str,
        work: impl Future<Output = Result<String, RelayError>>,
    ) -> Result<String, RelayError> {
        let shared = {
            let mut in_flight = self
                .in_flight
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            in_flight.entry(key.to_owned()).or_default().clone()
        };

        let result = shared.get_or_init(|| work).await.clone();

        let mut in_flight = self
            .in_flight
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if in_flight
            .get(key)
            .is_some_and(|current| Arc::ptr_eq(current, &shared))
        {
            in_flight.remove(key);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::Coalescer;
    use crate::error::RelayError;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    #[tokio::test]
    async fn identical_requests_at_once_share_one_call() {
        let coalescer = Arc::new(Coalescer::default());
        let calls = Arc::new(AtomicUsize::new(0));

        let tasks: Vec<_> = (0..5)
            .map(|_| {
                let coalescer = coalescer.clone();
                let calls = calls.clone();
                tokio::spawn(async move {
                    coalescer
                        .run("zelda", async {
                            calls.fetch_add(1, Ordering::SeqCst);
                            tokio::time::sleep(Duration::from_millis(30)).await;
                            Ok("body".to_owned())
                        })
                        .await
                })
            })
            .collect();
        for task in tasks {
            assert_eq!(task.await.unwrap().unwrap(), "body");
        }

        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn later_requests_start_a_new_call() {
        let coalescer = Coalescer::default();
        let calls = AtomicUsize::new(0);
        let work = || async {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok::<_, RelayError>("body".to_owned())
        };

        coalescer.run("zelda", work()).await.unwrap();
        coalescer.run("zelda", work()).await.unwrap();

        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn a_shared_failure_reaches_every_caller() {
        let coalescer = Coalescer::default();

        assert_eq!(
            coalescer
                .run("zelda", async { Err(RelayError::UpstreamRejected) })
                .await,
            Err(RelayError::UpstreamRejected)
        );
    }
}
