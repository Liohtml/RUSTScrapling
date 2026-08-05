//! The crawl [`Scheduler`]: a priority queue of pending requests with
//! fingerprint-based deduplication.

use super::request::SpiderRequest;
use std::collections::{BinaryHeap, HashSet};

/// Priority queue of pending requests plus a dedup set.
///
/// # Memory
///
/// `seen` records one fingerprint (a hex SHA string, ~64 bytes) for every
/// unique URL ever enqueued and is never compacted, so its footprint grows
/// linearly with the number of distinct URLs visited — roughly 100 MB per
/// ~1M unique URLs once `HashSet` overhead is included. For very large or
/// unbounded crawls, set `allowed_domains` on the spider to bound scope. The
/// set is persisted in checkpoints (via [`Scheduler::snapshot`]) so resumed
/// crawls do not re-visit already-seen URLs.
pub struct Scheduler {
    queue: BinaryHeap<SpiderRequest>,
    /// Fingerprints of every URL enqueued so far (dedup filter). Grows
    /// unbounded with the number of unique URLs — see the type-level note.
    seen: HashSet<String>,
    include_kwargs: bool,
    include_headers: bool,
    keep_fragments: bool,
}

impl Scheduler {
    /// Create an empty scheduler. The three flags are the fingerprint
    /// options applied to every enqueued request (whether meta, headers,
    /// and URL fragments participate in dedup — the spider's `fp_*`
    /// settings).
    pub fn new(include_kwargs: bool, include_headers: bool, keep_fragments: bool) -> Self {
        Self {
            queue: BinaryHeap::new(),
            seen: HashSet::new(),
            include_kwargs,
            include_headers,
            keep_fragments,
        }
    }

    /// Enqueue a request. Returns `true` if accepted, `false` if it was a
    /// duplicate.
    ///
    /// The request's fingerprint is (re)computed with this scheduler's
    /// fingerprint options first. Unless the request has `dont_filter`
    /// set, a fingerprint already in the seen set rejects it, and an
    /// accepted one is recorded — note the fingerprint is recorded at
    /// *enqueue* time, not on completion, so a URL stays deduplicated even
    /// while its request is still pending. `dont_filter` requests skip
    /// both the check and the recording.
    pub fn enqueue(&mut self, mut request: SpiderRequest) -> bool {
        request.update_fingerprint(
            self.include_kwargs,
            self.include_headers,
            self.keep_fragments,
        );

        if !request.dont_filter() {
            let fp = request.fingerprint().to_string();
            if self.seen.contains(&fp) {
                return false;
            }
            self.seen.insert(fp);
        }

        self.queue.push(request);
        true
    }

    /// Pop the highest-priority pending request (ties broken by
    /// fingerprint for determinism).
    pub fn dequeue(&mut self) -> Option<SpiderRequest> {
        self.queue.pop()
    }

    /// Whether no requests are pending (the seen set may still be
    /// non-empty).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Number of pending requests in the queue.
    #[must_use]
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Number of unique fingerprints recorded in the dedup set so far.
    #[must_use]
    pub fn seen_count(&self) -> usize {
        self.seen.len()
    }

    /// Drop all pending requests *and* the dedup set.
    pub fn clear(&mut self) {
        self.queue.clear();
        self.seen.clear();
    }

    /// Borrow all pending requests, in unspecified (heap) order.
    #[must_use]
    pub fn pending_requests(&self) -> Vec<&SpiderRequest> {
        self.queue.iter().collect()
    }

    /// Snapshot the scheduler state for checkpointing: a full copy of all
    /// pending requests (method, headers, body, meta, priority — everything,
    /// not just the URL) plus a copy of the seen fingerprint set.
    pub fn snapshot(&self) -> (Vec<SpiderRequest>, Vec<String>) {
        let pending = self.queue.iter().cloned().collect();
        let seen = self.seen.iter().cloned().collect();
        (pending, seen)
    }

    /// Restore the seen fingerprint set from a checkpoint so a resumed crawl
    /// does not re-visit URLs that were already crawled before the pause.
    pub fn restore_seen(&mut self, fingerprints: impl IntoIterator<Item = String>) {
        self.seen.extend(fingerprints);
    }
}
