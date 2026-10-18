//! A small per client rate limiter for `/search`.
//!
//! Upstream's limiter (`searx/limiter.py`, `limiter.toml`) is a bot detection
//! suite with link tokens and Redis counters. This port keeps the "protect the
//! instance from a flooding client" part and is documented as a simplification:
//! a sliding window per client address, disabled unless `server.limiter` is on.

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Sliding window rate limiter.
#[derive(Debug)]
pub struct Limiter {
    enabled: bool,
    window: Duration,
    max_requests: u32,
    hits: Mutex<HashMap<String, VecDeque<Instant>>>,
}

impl Limiter {
    /// A limiter for `max_requests` requests per `window` seconds.
    pub fn new(enabled: bool, window: Duration, max_requests: u32) -> Self {
        Self {
            enabled,
            window: window.max(Duration::from_millis(1)),
            max_requests: max_requests.max(1),
            hits: Mutex::new(HashMap::new()),
        }
    }

    /// A disabled limiter (used when `server.limiter` is `false`).
    pub fn disabled() -> Self {
        Self::new(false, Duration::from_secs(60), 30)
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Record a request; `Err` carries the seconds to wait.
    pub fn check(&self, client: &str) -> Result<(), u64> {
        if !self.enabled {
            return Ok(());
        }
        let now = Instant::now();
        let mut hits = self
            .hits
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        // Keep the map from growing without bound.
        if hits.len() > 10_000 {
            hits.retain(|_, times| {
                times
                    .back()
                    .is_some_and(|last| now.duration_since(*last) < self.window)
            });
        }
        let queue = hits.entry(client.to_string()).or_default();
        while let Some(front) = queue.front() {
            if now.duration_since(*front) > self.window {
                queue.pop_front();
            } else {
                break;
            }
        }
        if queue.len() as u32 >= self.max_requests {
            let oldest = queue.front().copied().unwrap_or(now);
            let remaining = self.window.saturating_sub(now.duration_since(oldest));
            // Round up, the client has to wait at least one second.
            let wait = (remaining.as_secs() + u64::from(remaining.subsec_nanos() > 0)).max(1);
            return Err(wait);
        }
        queue.push_back(now);
        Ok(())
    }

    /// Number of tracked clients (used by /stats).
    pub fn tracked(&self) -> usize {
        self.hits
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_limiter_allows_everything() {
        let limiter = Limiter::disabled();
        for _ in 0..100 {
            assert!(limiter.check("1.2.3.4").is_ok());
        }
    }

    #[test]
    fn exceeding_the_window_is_refused() {
        let limiter = Limiter::new(true, Duration::from_secs(60), 3);
        assert!(limiter.check("a").is_ok());
        assert!(limiter.check("a").is_ok());
        assert!(limiter.check("a").is_ok());
        assert_eq!(limiter.check("a"), Err(60));
        // another client is unaffected
        assert!(limiter.check("b").is_ok());
        assert_eq!(limiter.tracked(), 2);
    }

    #[test]
    fn short_ttl_allows_again_later() {
        let limiter = Limiter::new(true, Duration::from_millis(1), 1);
        assert!(limiter.check("a").is_ok());
        std::thread::sleep(Duration::from_millis(5));
        assert!(limiter.check("a").is_ok());
    }
}
