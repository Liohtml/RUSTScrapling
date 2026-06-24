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
/// set is held in memory only; it is not persisted across checkpoints.
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
    pub fn new(include_kwargs: bool, include_headers: bool, keep_fragments: bool) -> Self {
        Self {
            queue: BinaryHeap::new(),
            seen: HashSet::new(),
            include_kwargs,
            include_headers,
            keep_fragments,
        }
    }

    /// Enqueue a request. Returns true if accepted, false if duplicate.
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

    /// Pop highest priority request.
    pub fn dequeue(&mut self) -> Option<SpiderRequest> {
        self.queue.pop()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn seen_count(&self) -> usize {
        self.seen.len()
    }

    pub fn clear(&mut self) {
        self.queue.clear();
        self.seen.clear();
    }

    pub fn pending_requests(&self) -> Vec<&SpiderRequest> {
        self.queue.iter().collect()
    }
}
