use std::hash::{DefaultHasher, Hash, Hasher};
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};

use lru::LruCache;

const CAPACITY: usize = 256;
/// How many typed chars to look back for a completion the user is typing through.
const LOOKBACK_CHARS: usize = 64;

pub fn key(prefix: &str, suffix: &str) -> u64 {
    let mut h = DefaultHasher::new();
    prefix.hash(&mut h);
    suffix.hash(&mut h);
    h.finish()
}

pub struct Cache(Mutex<LruCache<u64, Arc<str>>>);

impl Cache {
    pub fn new() -> Self {
        Self(Mutex::new(LruCache::new(
            NonZeroUsize::new(CAPACITY).unwrap(),
        )))
    }

    pub fn insert(&self, key: u64, completion: Arc<str>) {
        self.0.lock().unwrap().put(key, completion);
    }

    /// Exact hit, or the remainder of a cached completion whose beginning the
    /// user has already typed.
    pub fn get(&self, prefix: &str, suffix: &str) -> Option<Arc<str>> {
        let mut lru = self.0.lock().unwrap();
        if let Some(c) = lru.get(&key(prefix, suffix)) {
            return Some(c.clone());
        }
        for (cut, _) in prefix.char_indices().rev().take(LOOKBACK_CHARS) {
            let typed = &prefix[cut..];
            if let Some(c) = lru.get(&key(&prefix[..cut], suffix))
                && c.len() > typed.len()
                && c.starts_with(typed)
            {
                return Some(c[typed.len()..].into());
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_and_typed_ahead() {
        let cache = Cache::new();
        cache.insert(key("let x", "\n"), " = foo();".into());
        assert_eq!(cache.get("let x", "\n").as_deref(), Some(" = foo();"));
        assert_eq!(cache.get("let x = f", "\n").as_deref(), Some("oo();"));
        // Typed past the whole completion: nothing left to suggest.
        assert_eq!(cache.get("let x = foo();", "\n"), None);
        // Diverged from the completion.
        assert_eq!(cache.get("let x = b", "\n"), None);
        // Different suffix.
        assert_eq!(cache.get("let x = f", "}\n"), None);
    }
}
