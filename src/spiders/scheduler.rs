//! The crawl [`Scheduler`]: a priority queue of pending requests with
//! fingerprint-based deduplication.

use super::request::SpiderRequest;
use std::cmp::{Ordering, Reverse};
use std::collections::{BinaryHeap, HashSet};

/// Order in which requests of **equal priority** leave the queue. Priority
/// always wins first; the strategy only breaks ties.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CrawlStrategy {
    /// Breadth-first: the oldest queued request goes first, so a crawl
    /// visits pages level by level from the start URLs. The default.
    #[default]
    Bfs,
    /// Depth-first: the newest queued request goes first, so a crawl
    /// follows a chain of links to its end before backtracking.
    Dfs,
}

/// A queued request with its arrival counter. The heap pops the highest
/// priority first and, among equals, the smallest counter — the counter
/// counts up for [`CrawlStrategy::Bfs`] (oldest first) and down for
/// [`CrawlStrategy::Dfs`] (newest first).
struct QueueEntry {
    counter: i64,
    request: SpiderRequest,
}

impl QueueEntry {
    fn key(&self) -> (i32, Reverse<i64>) {
        (self.request.priority(), Reverse(self.counter))
    }
}

impl PartialEq for QueueEntry {
    fn eq(&self, other: &Self) -> bool {
        self.key() == other.key()
    }
}
impl Eq for QueueEntry {}
impl PartialOrd for QueueEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for QueueEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        self.key().cmp(&other.key())
    }
}

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
    queue: BinaryHeap<QueueEntry>,
    /// Next arrival counter; steps by +1 (BFS) or -1 (DFS).
    next_counter: i64,
    strategy: CrawlStrategy,
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
    /// settings). Equal-priority requests are served breadth-first; see
    /// [`Scheduler::crawl_strategy`] to change that.
    pub fn new(include_kwargs: bool, include_headers: bool, keep_fragments: bool) -> Self {
        Self {
            queue: BinaryHeap::new(),
            next_counter: 0,
            strategy: CrawlStrategy::Bfs,
            seen: HashSet::new(),
            include_kwargs,
            include_headers,
            keep_fragments,
        }
    }

    /// Set how equal-priority requests are ordered. Call this before
    /// enqueueing: requests already queued keep the order they were given.
    #[must_use]
    pub fn crawl_strategy(mut self, strategy: CrawlStrategy) -> Self {
        self.strategy = strategy;
        self
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

        let counter = self.next_counter;
        self.next_counter += match self.strategy {
            CrawlStrategy::Bfs => 1,
            CrawlStrategy::Dfs => -1,
        };
        self.queue.push(QueueEntry { counter, request });
        true
    }

    /// Pop the highest-priority pending request. Ties are broken by the
    /// [`CrawlStrategy`]: oldest first (BFS, the default) or newest first
    /// (DFS).
    pub fn dequeue(&mut self) -> Option<SpiderRequest> {
        self.queue.pop().map(|entry| entry.request)
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
        self.queue.iter().map(|entry| &entry.request).collect()
    }

    /// Snapshot the scheduler state for checkpointing: a full copy of all
    /// pending requests (method, headers, body, meta, priority — everything,
    /// not just the URL) **in the order they would be dequeued**, plus a
    /// copy of the seen fingerprint set. Feed the pending list back through
    /// [`Scheduler::restore_pending`] to resume in the same order.
    pub fn snapshot(&self) -> (Vec<SpiderRequest>, Vec<String>) {
        let mut entries: Vec<&QueueEntry> = self.queue.iter().collect();
        entries.sort_by(|a, b| b.cmp(a));
        let pending = entries.into_iter().map(|e| e.request.clone()).collect();
        let seen = self.seen.iter().cloned().collect();
        (pending, seen)
    }

    /// Re-queue pending requests from a checkpoint, given in dequeue order
    /// (as [`Scheduler::snapshot`] returns them), so the resumed crawl
    /// continues in that same order under either [`CrawlStrategy`]. The
    /// requests bypass the dedup filter: their fingerprints are usually in
    /// the restored seen set already, and they must be crawled regardless.
    pub fn restore_pending(&mut self, requests: Vec<SpiderRequest>) {
        let ordered: Box<dyn Iterator<Item = SpiderRequest>> = match self.strategy {
            CrawlStrategy::Bfs => Box::new(requests.into_iter()),
            // Newest-first: queue the last-to-run first so the first-to-run
            // ends up newest.
            CrawlStrategy::Dfs => Box::new(requests.into_iter().rev()),
        };
        for mut request in ordered {
            request.set_dont_filter(true);
            self.enqueue(request);
        }
    }

    /// Restore the seen fingerprint set from a checkpoint so a resumed crawl
    /// does not re-visit URLs that were already crawled before the pause.
    pub fn restore_seen(&mut self, fingerprints: impl IntoIterator<Item = String>) {
        self.seen.extend(fingerprints);
    }
}
