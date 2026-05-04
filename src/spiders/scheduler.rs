use std::collections::{BinaryHeap, HashSet};
use super::request::SpiderRequest;

pub struct Scheduler {
    queue: BinaryHeap<SpiderRequest>,
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
        request.update_fingerprint(self.include_kwargs, self.include_headers, self.keep_fragments);

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
