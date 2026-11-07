use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;

const MAX_TRACKED_CLIENTS: usize = 1024;

pub struct RateLimiter {
    limit: u32,
    window_seconds: i64,
    buckets: Mutex<HashMap<IpAddr, Window>>,
}

struct Window {
    started_at: i64,
    count: u32,
}

impl RateLimiter {
    pub fn new(limit: u32, window_seconds: i64) -> Self {
        Self {
            limit,
            window_seconds,
            buckets: Mutex::new(HashMap::new()),
        }
    }

    pub fn check(&self, client: IpAddr, now: i64) -> bool {
        let mut buckets = self
            .buckets
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        if buckets.len() > MAX_TRACKED_CLIENTS {
            buckets.retain(|_, window| now.saturating_sub(window.started_at) < self.window_seconds);
        }

        let window = buckets.entry(client).or_insert(Window {
            started_at: now,
            count: 0,
        });

        if now < window.started_at || now.saturating_sub(window.started_at) >= self.window_seconds {
            window.started_at = now;
            window.count = 0;
        }

        if window.count >= self.limit {
            return false;
        }

        window.count += 1;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::RateLimiter;
    use std::net::{IpAddr, Ipv4Addr};

    fn client(last: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(127, 0, 0, last))
    }

    #[test]
    fn a_client_can_spend_the_whole_limit() {
        let limiter = RateLimiter::new(3, 60);

        assert!(limiter.check(client(1), 100));
        assert!(limiter.check(client(1), 101));
        assert!(limiter.check(client(1), 102));
        assert!(!limiter.check(client(1), 103));
    }

    #[test]
    fn the_window_refills_after_it_expires() {
        let limiter = RateLimiter::new(1, 60);
        assert!(limiter.check(client(1), 100));
        assert!(!limiter.check(client(1), 101));

        assert!(limiter.check(client(1), 160));
    }

    #[test]
    fn clients_do_not_share_a_budget() {
        let limiter = RateLimiter::new(1, 60);
        assert!(limiter.check(client(1), 100));
        assert!(!limiter.check(client(1), 101));
        assert!(limiter.check(client(2), 101));
    }

    #[test]
    fn a_clock_that_moves_backwards_resets_the_window() {
        let limiter = RateLimiter::new(1, 60);
        assert!(limiter.check(client(1), 100));
        assert!(!limiter.check(client(1), 101));

        assert!(limiter.check(client(1), 50));
    }
}
