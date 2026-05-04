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
    let low = SpiderRequest::builder("https://example.com/low").priority(1).build();
    let high = SpiderRequest::builder("https://example.com/high").priority(10).build();
    let mid = SpiderRequest::builder("https://example.com/mid").priority(5).build();

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
