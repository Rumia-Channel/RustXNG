//! In-memory HTTP response cache, ported from `searx/cache.py`.
//!
//! Upstream configuration equivalents are `cache.ttl` (default 300 s) and
//! `cache.max_bytes` (default 8 MB); the cache is a bounded LRU with a TTL.

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// Cached response payload (a clone of the parts engines look at).
#[derive(Debug, Clone)]
pub struct CachedResponse {
    pub status: u16,
    pub final_url: String,
    pub headers: std::collections::BTreeMap<String, String>,
    pub body: std::sync::Arc<Vec<u8>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    pub hit: u64,
    pub miss: u64,
    pub entries: usize,
    pub bytes: usize,
    pub error: u64,
}

struct Entry {
    stored: Instant,
    response: CachedResponse,
    size: usize,
}

#[derive(Default)]
struct Inner {
    entries: HashMap<String, Entry>,
    order: VecDeque<String>,
    bytes: usize,
}

/// A TTL + LRU cache for engine responses.
pub struct ResponseCache {
    inner: Mutex<Inner>,
    ttl: Duration,
    max_bytes: usize,
    hit: AtomicU64,
    miss: AtomicU64,
    error: AtomicU64,
}

impl Default for ResponseCache {
    fn default() -> Self {
        Self::new(Duration::from_secs(300), 8 * 1024 * 1024)
    }
}

impl ResponseCache {
    pub fn new(ttl: Duration, max_bytes: usize) -> Self {
        Self {
            inner: Mutex::new(Inner::default()),
            ttl,
            max_bytes,
            hit: AtomicU64::new(0),
            miss: AtomicU64::new(0),
            error: AtomicU64::new(0),
        }
    }

    /// The cache key of a request: method + URL + body.
    pub fn key(method: &str, url: &str, body: &str) -> String {
        format!("{method} {url} {body}")
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn get(&self, key: &str) -> Option<CachedResponse> {
        let mut inner = self.lock();
        let expired = inner
            .entries
            .get(key)
            .map(|entry| entry.stored.elapsed() > self.ttl)
            .unwrap_or(false);
        if expired {
            self.remove_locked(&mut inner, key);
            self.miss.fetch_add(1, Ordering::Relaxed);
            return None;
        }
        if let Some(entry) = inner.entries.get(key) {
            let response = entry.response.clone();
            self.hit.fetch_add(1, Ordering::Relaxed);
            Some(response)
        } else {
            self.miss.fetch_add(1, Ordering::Relaxed);
            None
        }
    }

    pub fn set(&self, key: &str, response: CachedResponse) {
        let size = response.body.len() + key.len() + 128;
        if size > self.max_bytes {
            self.error.fetch_add(1, Ordering::Relaxed);
            return;
        }
        let mut inner = self.lock();
        self.remove_locked(&mut inner, key);
        inner.bytes += size;
        inner.entries.insert(
            key.to_string(),
            Entry {
                stored: Instant::now(),
                response,
                size,
            },
        );
        inner.order.push_back(key.to_string());
        self.evict_locked(&mut inner);
    }

    fn remove_locked(&self, inner: &mut Inner, key: &str) {
        if let Some(entry) = inner.entries.remove(key) {
            inner.bytes = inner.bytes.saturating_sub(entry.size);
            if let Some(position) = inner.order.iter().position(|candidate| candidate == key) {
                inner.order.remove(position);
            }
        }
    }

    fn evict_locked(&self, inner: &mut Inner) {
        while inner.bytes > self.max_bytes {
            let Some(key) = inner.order.pop_front() else {
                break;
            };
            if let Some(entry) = inner.entries.remove(&key) {
                inner.bytes = inner.bytes.saturating_sub(entry.size);
            }
        }
    }

    /// Drop every entry (used by tests and `/stats`).
    pub fn clear(&self) {
        let mut inner = self.lock();
        inner.entries.clear();
        inner.order.clear();
        inner.bytes = 0;
    }

    pub fn stats(&self) -> CacheStats {
        let inner = self.lock();
        CacheStats {
            hit: self.hit.load(Ordering::Relaxed),
            miss: self.miss.load(Ordering::Relaxed),
            entries: inner.entries.len(),
            bytes: inner.bytes,
            error: self.error.load(Ordering::Relaxed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(body: &str) -> CachedResponse {
        CachedResponse {
            status: 200,
            final_url: "https://example.com/".into(),
            headers: Default::default(),
            body: std::sync::Arc::new(body.as_bytes().to_vec()),
        }
    }

    #[test]
    fn stored_responses_are_returned() {
        let cache = ResponseCache::default();
        cache.set("GET https://example.com/ ", response("hello"));
        let cached = cache.get("GET https://example.com/ ").expect("cached");
        assert_eq!(cached.body.len(), 5);
        assert_eq!(cache.stats().hit, 1);
    }

    #[test]
    fn missing_keys_count_as_misses() {
        let cache = ResponseCache::default();
        assert!(cache.get("nope").is_none());
        assert_eq!(cache.stats().miss, 1);
    }

    #[test]
    fn entries_expire_after_the_ttl() {
        let cache = ResponseCache::new(Duration::from_millis(0), 1024);
        cache.set("key", response("x"));
        std::thread::sleep(Duration::from_millis(2));
        assert!(cache.get("key").is_none());
    }

    #[test]
    fn the_cache_stays_within_its_size_budget() {
        let cache = ResponseCache::new(Duration::from_secs(60), 200);
        for index in 0..50 {
            cache.set(&format!("key-{index}"), response(&"x".repeat(64)));
        }
        assert!(cache.stats().bytes <= 200);
    }

    #[test]
    fn oversized_responses_are_not_cached() {
        let cache = ResponseCache::new(Duration::from_secs(60), 16);
        cache.set("key", response(&"x".repeat(100)));
        assert!(cache.get("key").is_none());
    }
}
