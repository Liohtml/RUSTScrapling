//! Round-robin proxy rotation shared by the
//! [`Fetcher`](crate::fetchers::client::Fetcher)'s per-proxy client pool.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Rotates through a list of proxy URLs.
///
/// `next()` uses round-robin selection and is safe to call from multiple
/// threads simultaneously.  `random()` picks a proxy based on a simple hash
/// of the current position so as not to introduce a heavy RNG dependency.
#[derive(Debug, Clone)]
pub struct ProxyRotator {
    proxies: Arc<Vec<String>>,
    cursor: Arc<AtomicUsize>,
}

impl ProxyRotator {
    /// Create a new rotator from a list of proxy URLs.
    ///
    /// Returns `None` if the supplied list is empty.
    pub fn new(proxies: Vec<String>) -> Option<Self> {
        if proxies.is_empty() {
            return None;
        }
        Some(Self {
            proxies: Arc::new(proxies),
            cursor: Arc::new(AtomicUsize::new(0)),
        })
    }

    /// Return the next proxy in round-robin order, advancing the cursor.
    #[must_use]
    pub fn next(&self) -> &str {
        &self.proxies[self.next_index()]
    }

    /// Advance the cursor and return the index of the next proxy in
    /// round-robin order. Useful for indexing a parallel collection (e.g. a
    /// pool of pre-built HTTP clients) that shares the rotator's ordering.
    #[must_use]
    pub fn next_index(&self) -> usize {
        self.cursor.fetch_add(1, Ordering::Relaxed) % self.proxies.len()
    }

    /// Return a pseudo-random proxy, advancing the cursor so consecutive
    /// calls don't keep returning the same proxy.
    #[must_use]
    pub fn random(&self) -> &str {
        let pos = self.cursor.fetch_add(1, Ordering::Relaxed);
        // Simple pseudo-random selection: mix the position with a constant.
        let idx = pos.wrapping_mul(2654435761).wrapping_add(1013904223) % self.proxies.len();
        &self.proxies[idx]
    }

    /// Number of proxies in the rotator.
    #[must_use]
    pub fn len(&self) -> usize {
        self.proxies.len()
    }

    /// Whether the rotator holds no proxies (never true in practice, since
    /// [`ProxyRotator::new`] rejects empty lists).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.proxies.is_empty()
    }
}
