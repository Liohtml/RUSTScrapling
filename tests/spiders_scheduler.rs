use rust_scrapling::spiders::request::SpiderRequest;
use rust_scrapling::spiders::scheduler::Scheduler;

#[test]
fn test_enqueue_dequeue() {
    let mut sched = Scheduler::new(false, false, false);
    let req = SpiderRequest::new("https://example.com");
    assert!(sched.enqueue(req));
    assert_eq!(sched.len(), 1);
    let popped = sched.dequeue().unwrap();
    assert_eq!(popped.url(), "https://example.com");
    assert!(sched.is_empty());
}

#[test]
fn test_dedup_rejects_duplicates() {
    let mut sched = Scheduler::new(false, false, false);
    let r1 = SpiderRequest::new("https://example.com");
    let r2 = SpiderRequest::new("https://example.com");
    assert!(sched.enqueue(r1));
    assert!(!sched.enqueue(r2));
    assert_eq!(sched.len(), 1);
    assert_eq!(sched.seen_count(), 1);
}

#[test]
fn test_dont_filter_bypasses_dedup() {
    let mut sched = Scheduler::new(false, false, false);
    let r1 = SpiderRequest::new("https://example.com");
    let mut r2 = SpiderRequest::new("https://example.com");
    r2.set_dont_filter(true);

    assert!(sched.enqueue(r1));
    assert!(sched.enqueue(r2));
    assert_eq!(sched.len(), 2);
}

#[test]
fn test_priority_ordering() {
    let mut sched = Scheduler::new(false, false, false);
    let low = SpiderRequest::builder("https://example.com/low")
        .priority(1)
        .build();
    let high = SpiderRequest::builder("https://example.com/high")
        .priority(10)
        .build();
    let mid = SpiderRequest::builder("https://example.com/mid")
        .priority(5)
        .build();

    sched.enqueue(low);
    sched.enqueue(high);
    sched.enqueue(mid);

    assert_eq!(sched.dequeue().unwrap().priority(), 10);
    assert_eq!(sched.dequeue().unwrap().priority(), 5);
    assert_eq!(sched.dequeue().unwrap().priority(), 1);
}

#[test]
fn test_is_empty_and_len() {
    let mut sched = Scheduler::new(false, false, false);
    assert!(sched.is_empty());
    assert_eq!(sched.len(), 0);

    sched.enqueue(SpiderRequest::new("https://example.com"));
    assert!(!sched.is_empty());
    assert_eq!(sched.len(), 1);
}

#[test]
fn test_clear() {
    let mut sched = Scheduler::new(false, false, false);
    sched.enqueue(SpiderRequest::new("https://example.com/1"));
    sched.enqueue(SpiderRequest::new("https://example.com/2"));
    assert_eq!(sched.len(), 2);

    sched.clear();
    assert!(sched.is_empty());
    assert_eq!(sched.seen_count(), 0);
}

#[test]
fn test_dequeue_empty() {
    let mut sched = Scheduler::new(false, false, false);
    assert!(sched.dequeue().is_none());
}

#[test]
fn test_snapshot_captures_pending_and_seen() {
    let mut sched = Scheduler::new(false, false, false);
    sched.enqueue(SpiderRequest::new("https://example.com/pending"));
    sched.enqueue(SpiderRequest::new("https://example.com/crawled"));
    // Simulate one request having been dispatched already.
    let dispatched = sched.dequeue().unwrap();

    let (pending, seen) = sched.snapshot();
    assert_eq!(pending.len(), 1);
    assert_eq!(seen.len(), 2, "seen keeps fingerprints of dequeued URLs");
    assert!(seen.contains(&dispatched.fingerprint().to_string()));
}

#[test]
fn test_restore_seen_rejects_already_crawled_urls() {
    let mut first_run = Scheduler::new(false, false, false);
    first_run.enqueue(SpiderRequest::new("https://example.com/done"));
    let (_, seen) = first_run.snapshot();

    let mut resumed = Scheduler::new(false, false, false);
    resumed.restore_seen(seen);
    assert!(
        !resumed.enqueue(SpiderRequest::new("https://example.com/done")),
        "URL crawled before the pause must be filtered after resume"
    );
    assert!(
        resumed.enqueue(SpiderRequest::new("https://example.com/new")),
        "unseen URL must still be accepted after resume"
    );
}

#[test]
fn test_restored_pending_reenqueues_with_dont_filter() {
    // Mirrors the engine resume path: pending URLs are part of the restored
    // seen set, so they are re-enqueued with dont_filter to bypass the dedup.
    let mut first_run = Scheduler::new(false, false, false);
    first_run.enqueue(SpiderRequest::new("https://example.com/pending"));
    let (pending, seen) = first_run.snapshot();

    let mut resumed = Scheduler::new(false, false, false);
    resumed.restore_seen(seen);
    for mut req in pending {
        req.set_dont_filter(true);
        assert!(resumed.enqueue(req));
    }
    assert_eq!(resumed.len(), 1);
}

// --- crawl strategy (upstream v0.5) ---

use rust_scrapling::spiders::scheduler::CrawlStrategy;

fn drain(sched: &mut Scheduler) -> Vec<String> {
    std::iter::from_fn(|| sched.dequeue())
        .map(|r| r.url().to_string())
        .collect()
}

fn urls(names: &[&str]) -> Vec<String> {
    names
        .iter()
        .map(|n| format!("https://example.com/{n}"))
        .collect()
}

#[test]
fn bfs_serves_equal_priority_requests_oldest_first() {
    // The default. Before v0.5 parity, ties were broken by fingerprint
    // hash, i.e. an arbitrary order unrelated to discovery order.
    let mut sched = Scheduler::new(false, false, false);
    for url in urls(&["c", "a", "d", "b"]) {
        sched.enqueue(SpiderRequest::new(&url));
    }
    assert_eq!(drain(&mut sched), urls(&["c", "a", "d", "b"]));
}

#[test]
fn dfs_serves_equal_priority_requests_newest_first() {
    let mut sched = Scheduler::new(false, false, false).crawl_strategy(CrawlStrategy::Dfs);
    for url in urls(&["c", "a", "d", "b"]) {
        sched.enqueue(SpiderRequest::new(&url));
    }
    assert_eq!(drain(&mut sched), urls(&["b", "d", "a", "c"]));
}

#[test]
fn priority_beats_the_crawl_strategy() {
    for strategy in [CrawlStrategy::Bfs, CrawlStrategy::Dfs] {
        let mut sched = Scheduler::new(false, false, false).crawl_strategy(strategy);
        sched.enqueue(SpiderRequest::new("https://example.com/low-1"));
        sched.enqueue(
            SpiderRequest::builder("https://example.com/high")
                .priority(10)
                .build(),
        );
        sched.enqueue(SpiderRequest::new("https://example.com/low-2"));
        let order = drain(&mut sched);
        assert_eq!(order[0], "https://example.com/high", "{strategy:?}");
    }
}

#[test]
fn snapshot_and_restore_keep_the_dequeue_order_under_both_strategies() {
    for strategy in [CrawlStrategy::Bfs, CrawlStrategy::Dfs] {
        let mut original = Scheduler::new(false, false, false).crawl_strategy(strategy);
        for url in urls(&["c", "a", "d", "b"]) {
            original.enqueue(SpiderRequest::new(&url));
        }
        original.enqueue(
            SpiderRequest::builder("https://example.com/high")
                .priority(5)
                .build(),
        );

        let (pending, seen) = original.snapshot();
        let expected = drain(&mut original);
        // The snapshot itself is already in dequeue order.
        let snapshot_order: Vec<String> = pending.iter().map(|r| r.url().to_string()).collect();
        assert_eq!(snapshot_order, expected, "{strategy:?}");

        let mut resumed = Scheduler::new(false, false, false).crawl_strategy(strategy);
        resumed.restore_seen(seen);
        resumed.restore_pending(pending);
        assert_eq!(drain(&mut resumed), expected, "{strategy:?}");
    }
}
