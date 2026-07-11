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
    for url in &pending {
        let mut req = SpiderRequest::new(url);
        req.set_dont_filter(true);
        assert!(resumed.enqueue(req));
    }
    assert_eq!(resumed.len(), 1);
}
