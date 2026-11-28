//! Token-bucket rate limiter (specs #149, #193, #283).
//! Per-IP and per-API-key/tenant buckets with headers.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct RateLimiter {
    buckets: Mutex<HashMap<String, Bucket>>,
    max_rate: f64, // tokens per second
    capacity: u64,
}

#[derive(Debug, Clone)]
struct Bucket {
    tokens: f64,
    last: Instant,
}

impl RateLimiter {
    pub fn new(max_rate: f64, capacity: u64) -> Self {
        Self { buckets: Mutex::new(HashMap::new()), max_rate, capacity }
    }

    /// Returns (allowed, remaining_tokens, retry_after_secs).
    pub fn check(&self, key: &str) -> (bool, u64, u64) {
        let now = Instant::now();
        let mut buckets = self.buckets.lock().unwrap();
        let bucket = buckets.entry(key.to_string()).or_insert(Bucket {
            tokens: self.capacity as f64,
            last: now,
        });
        // Refill tokens based on elapsed time.
        let elapsed = now.duration_since(bucket.last).as_secs_f64();
        bucket.tokens = (bucket.tokens + elapsed * self.max_rate).min(self.capacity as f64);
        bucket.last = now;
        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            (true, bucket.tokens as u64, 0)
        } else {
            let retry_after = ((1.0 - bucket.tokens) / self.max_rate).ceil() as u64;
            (false, 0, retry_after.max(1))
        }
    }

    pub fn prune(&self) {
        // Optional: prune stale buckets to bound memory (spec #240).
        let mut buckets = self.buckets.lock().unwrap();
        buckets.retain(|_, b| b.tokens < b.capacity as f64 || b.last.elapsed().as_secs() < 3600);
    }
}

pub struct RateLimitHeaders {
    pub limit: u64,
    pub remaining: u64,
    pub retry_after: Option<u64>,
}

impl RateLimitHeaders {
    pub fn from_check(allowed: bool, remaining: u64, retry_after: u64, capacity: u64) -> Self {
        Self { limit: capacity, remaining, retry_after: if allowed { None } else { Some(retry_after) } }
    }
}
