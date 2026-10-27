use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const MAX_KEYS: usize = 4096;

pub struct RateLimiter<K> {
    max: u32,
    window: Duration,
    hits: Mutex<HashMap<K, (Instant, u32)>>,
}

impl<K: Hash + Eq> RateLimiter<K> {
    pub fn new(max: u32, window: Duration) -> Self {
        Self {
            max,
            window,
            hits: Mutex::new(HashMap::new()),
        }
    }

    pub fn allow(&self, key: K) -> bool {
        self.allow_at(key, Instant::now())
    }

    fn allow_at(&self, key: K, now: Instant) -> bool {
        let mut hits = self.hits.lock().unwrap_or_else(|p| p.into_inner());
        if hits.len() >= MAX_KEYS {
            hits.retain(|_, (start, _)| now.duration_since(*start) < self.window);
        }
        let entry = hits.entry(key).or_insert((now, 0));
        if now.duration_since(entry.0) >= self.window {
            *entry = (now, 0);
        }
        entry.1 += 1;
        entry.1 <= self.max
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_per_key_and_resets_after_window() {
        let rl = RateLimiter::new(2, Duration::from_secs(60));
        let t0 = Instant::now();
        assert!(rl.allow_at("a", t0));
        assert!(rl.allow_at("a", t0));
        assert!(!rl.allow_at("a", t0));
        assert!(rl.allow_at("b", t0), "keys are independent");
        assert!(rl.allow_at("a", t0 + Duration::from_secs(61)));
    }

    #[test]
    fn stale_keys_are_purged() {
        let rl = RateLimiter::new(1, Duration::from_secs(1));
        let t0 = Instant::now();
        for k in 0..MAX_KEYS {
            rl.allow_at(k, t0);
        }
        rl.allow_at(MAX_KEYS, t0 + Duration::from_secs(2));
        assert_eq!(rl.hits.lock().unwrap().len(), 1);
    }
}
