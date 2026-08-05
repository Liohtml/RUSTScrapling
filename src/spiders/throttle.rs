//! AutoThrottle: adaptive per-domain crawl delays (port of upstream
//! Scrapling v0.4.12's autothrottle, which follows Scrapy's algorithm).
//!
//! Each domain gets a delay that adapts to the server's measured response
//! latency: fast healthy servers are crawled faster (down to the configured
//! floor), slow ones slower. Rate-limiting responses (429/503) double the
//! delay — or jump straight to the server's `Retry-After` — and healthy
//! responses ease it back down. The spider's static `download_delay` and the
//! site's robots.txt `Crawl-delay` act as floors that are never undercut.
//!
//! Spacing is enforced with a per-domain reservation clock
//! ([`AutoThrottle::reserve`]), so requests to one domain are spaced by the
//! current delay even when several tasks fetch that domain concurrently —
//! and domains never delay each other.

use std::collections::HashMap;
use std::time::Duration;
use tokio::time::Instant;

/// Statuses treated as rate-limiting: the server is explicitly pushing back.
fn is_throttling_status(status: u16) -> bool {
    status == 429 || status == 503
}

struct Slot {
    /// Current adaptive delay between requests to this domain, in seconds.
    delay: f64,
    /// The earliest time the next request to this domain may start.
    next_allowed: Instant,
}

/// Per-domain adaptive delay state (see the [module docs](self)).
///
/// The engine drives it with two calls per request:
/// [`AutoThrottle::reserve`] before sending (returns how long to wait) and
/// [`AutoThrottle::record`] when the response arrives (adapts the delay to
/// the measured latency and status).
pub struct AutoThrottle {
    start_delay: f64,
    max_delay: f64,
    target_concurrency: f64,
    slots: HashMap<String, Slot>,
}

impl AutoThrottle {
    /// `start_delay`: initial per-domain delay until latency data exists.
    /// `max_delay`: hard upper bound the delay never exceeds.
    /// `target_concurrency`: how many requests we would like a domain to be
    /// serving from us in parallel on average (Scrapy's
    /// AUTOTHROTTLE_TARGET_CONCURRENCY); the adaptive target delay is
    /// `latency / target_concurrency`.
    pub fn new(start_delay: f64, max_delay: f64, target_concurrency: f64) -> Self {
        Self {
            start_delay: start_delay.max(0.0),
            max_delay: max_delay.max(0.0),
            target_concurrency: target_concurrency.max(0.01),
            slots: HashMap::new(),
        }
    }

    /// Reserve the next request slot for `domain` and return how long the
    /// caller must wait before sending. `floor` is the minimum delay
    /// (max of the spider's download_delay and the domain's robots.txt
    /// Crawl-delay). Requests are spaced by the current adaptive delay via a
    /// per-domain reservation clock, so concurrent callers queue up behind
    /// each other instead of all firing at once.
    pub fn reserve(&mut self, domain: &str, floor: f64, now: Instant) -> Duration {
        let start = self.start_delay.max(floor);
        let slot = self.slots.entry(domain.to_string()).or_insert(Slot {
            delay: start,
            next_allowed: now,
        });
        let wait = slot.next_allowed.saturating_duration_since(now);
        let spacing = slot.delay.max(floor);
        let base = if slot.next_allowed > now {
            slot.next_allowed
        } else {
            now
        };
        slot.next_allowed = base + Duration::from_secs_f64(spacing);
        wait
    }

    /// Record a completed response for `domain` and adapt its delay
    /// (Scrapy's `_adjust_delay`).
    ///
    /// - 429/503: the delay doubles (at least 1s, so a zero start delay
    ///   still backs off), or jumps to `retry_after` when the server sent a
    ///   larger one — capped at `max_delay` even if `Retry-After` asks for
    ///   more — and the domain's clock is pushed out so the very next
    ///   request already honors the new delay.
    /// - otherwise the delay moves halfway toward
    ///   `latency / target_concurrency`, but jumps straight to the target
    ///   when the target is higher (a suddenly slow server backs off in one
    ///   response, not several). Only a 200 may shrink the delay: other
    ///   statuses' latencies are not representative of full-page loads.
    ///
    /// The result is always clamped to `[floor, max_delay]`.
    pub fn record(
        &mut self,
        domain: &str,
        latency: f64,
        status: u16,
        retry_after: Option<f64>,
        floor: f64,
        now: Instant,
    ) {
        let start = self.start_delay.max(floor);
        let slot = self.slots.entry(domain.to_string()).or_insert(Slot {
            delay: start,
            next_allowed: now,
        });

        let new_delay = if is_throttling_status(status) {
            let min_backoff = self.start_delay.max(1.0).min(self.max_delay);
            let doubled = (slot.delay * 2.0).max(min_backoff);
            doubled.max(retry_after.unwrap_or(0.0))
        } else {
            let target = latency.max(0.0) / self.target_concurrency;
            // Halfway toward the target, but never below it: when the
            // server got slower, back off in a single step (Scrapy's
            // max(target_delay, new_delay)).
            let candidate = ((slot.delay + target) / 2.0).max(target);
            if status == 200 {
                candidate
            } else {
                slot.delay.max(candidate)
            }
        };
        slot.delay = new_delay.clamp(floor.min(self.max_delay), self.max_delay);

        if is_throttling_status(status) {
            // Make the back-off effective immediately: without this, the
            // reservation made before this response was known would still
            // use the old (small) delay for the next request.
            let backoff = now + Duration::from_secs_f64(slot.delay);
            if backoff > slot.next_allowed {
                slot.next_allowed = backoff;
            }
        }
    }

    /// The current adaptive delay (seconds) for a domain, if any request
    /// to it has been reserved or recorded yet. Mainly for introspection
    /// and tests.
    #[must_use]
    pub fn current_delay(&self, domain: &str) -> Option<f64> {
        self.slots.get(domain).map(|s| s.delay)
    }
}

/// Parse an HTTP `Retry-After` header value: only the delta-seconds form is
/// supported (the HTTP-date form is rare on rate-limit responses and would
/// need a date parser); returns `None` for dates or garbage.
#[must_use]
pub fn parse_retry_after(value: &str) -> Option<f64> {
    let v = value.trim();
    let secs: f64 = v.parse().ok()?;
    if secs.is_finite() && secs >= 0.0 {
        Some(secs)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t0() -> Instant {
        Instant::now()
    }

    #[test]
    fn first_reserve_is_immediate_and_spaces_the_next() {
        let mut at = AutoThrottle::new(1.0, 60.0, 1.0);
        let now = t0();
        assert_eq!(at.reserve("d", 0.0, now), Duration::ZERO);
        // Second reservation at the same instant must wait the start delay.
        let wait = at.reserve("d", 0.0, now);
        assert!((wait.as_secs_f64() - 1.0).abs() < 1e-6, "wait {:?}", wait);
        // Third waits two delays.
        let wait = at.reserve("d", 0.0, now);
        assert!((wait.as_secs_f64() - 2.0).abs() < 1e-6);
    }

    #[test]
    fn domains_do_not_delay_each_other() {
        let mut at = AutoThrottle::new(1.0, 60.0, 1.0);
        let now = t0();
        assert_eq!(at.reserve("a", 0.0, now), Duration::ZERO);
        assert_eq!(at.reserve("b", 0.0, now), Duration::ZERO);
    }

    #[test]
    fn fast_healthy_responses_shrink_the_delay_toward_latency() {
        let mut at = AutoThrottle::new(4.0, 60.0, 1.0);
        let now = t0();
        at.reserve("d", 0.0, now);
        // 100ms latency, 200 OK: delay halves toward 0.1 each time.
        at.record("d", 0.1, 200, None, 0.0, now);
        let d1 = at.current_delay("d").unwrap();
        assert!((d1 - 2.05).abs() < 1e-6, "got {}", d1);
        at.record("d", 0.1, 200, None, 0.0, now);
        let d2 = at.current_delay("d").unwrap();
        assert!(d2 < d1);
    }

    #[test]
    fn delay_never_undercuts_the_floor() {
        let mut at = AutoThrottle::new(4.0, 60.0, 1.0);
        let now = t0();
        at.reserve("d", 2.0, now);
        for _ in 0..20 {
            at.record("d", 0.01, 200, None, 2.0, now);
        }
        assert!((at.current_delay("d").unwrap() - 2.0).abs() < 1e-9);
    }

    #[test]
    fn throttling_status_doubles_and_respects_larger_retry_after() {
        let mut at = AutoThrottle::new(1.0, 60.0, 1.0);
        let now = t0();
        at.reserve("d", 0.0, now);
        at.record("d", 0.1, 429, None, 0.0, now);
        assert!((at.current_delay("d").unwrap() - 2.0).abs() < 1e-9);
        // Retry-After larger than doubling wins.
        at.record("d", 0.1, 429, Some(30.0), 0.0, now);
        assert!((at.current_delay("d").unwrap() - 30.0).abs() < 1e-9);
        // Doubling larger than Retry-After wins too.
        at.record("d", 0.1, 503, Some(1.0), 0.0, now);
        assert!((at.current_delay("d").unwrap() - 60.0).abs() < 1e-9);
    }

    #[test]
    fn throttling_pushes_the_reservation_clock_immediately() {
        let mut at = AutoThrottle::new(0.05, 60.0, 1.0);
        let now = t0();
        at.reserve("d", 0.0, now);
        at.record("d", 0.1, 429, Some(5.0), 0.0, now);
        // The next reservation must already wait ~5s, not the old 50ms.
        let wait = at.reserve("d", 0.0, now);
        assert!(
            wait.as_secs_f64() >= 4.9,
            "back-off must apply to the very next request, waited {:?}",
            wait
        );
    }

    #[test]
    fn delay_is_clamped_at_max() {
        let mut at = AutoThrottle::new(1.0, 10.0, 1.0);
        let now = t0();
        at.reserve("d", 0.0, now);
        for _ in 0..10 {
            at.record("d", 0.1, 429, None, 0.0, now);
        }
        assert!((at.current_delay("d").unwrap() - 10.0).abs() < 1e-9);
    }

    #[test]
    fn non_success_non_throttle_never_shrinks_the_delay() {
        let mut at = AutoThrottle::new(4.0, 60.0, 1.0);
        let now = t0();
        at.reserve("d", 0.0, now);
        at.record("d", 0.01, 500, None, 0.0, now);
        assert!((at.current_delay("d").unwrap() - 4.0).abs() < 1e-9);
        // But a slow error response may grow it.
        at.record("d", 20.0, 500, None, 0.0, now);
        assert!(at.current_delay("d").unwrap() > 4.0);
    }

    #[test]
    fn target_concurrency_divides_the_latency_target() {
        let mut at = AutoThrottle::new(2.0, 60.0, 2.0);
        let now = t0();
        at.reserve("d", 0.0, now);
        // latency 2s at target concurrency 2 -> target delay 1s -> avg 1.5s.
        at.record("d", 2.0, 200, None, 0.0, now);
        assert!((at.current_delay("d").unwrap() - 1.5).abs() < 1e-9);
    }

    #[test]
    fn sudden_slowdown_backs_off_in_one_step() {
        // Scrapy's max(target_delay, new_delay): when the measured latency
        // exceeds the current delay, jump straight to the target instead of
        // approaching it over several responses.
        let mut at = AutoThrottle::new(1.0, 60.0, 1.0);
        let now = t0();
        at.reserve("d", 0.0, now);
        at.record("d", 10.0, 200, None, 0.0, now);
        assert!((at.current_delay("d").unwrap() - 10.0).abs() < 1e-9);
    }

    #[test]
    fn non_200_success_does_not_shrink_the_delay() {
        // Scrapy only shrinks on exactly 200: a 204's latency is not
        // representative of a full page load.
        let mut at = AutoThrottle::new(4.0, 60.0, 1.0);
        let now = t0();
        at.reserve("d", 0.0, now);
        at.record("d", 0.01, 204, None, 0.0, now);
        assert!((at.current_delay("d").unwrap() - 4.0).abs() < 1e-9);
    }

    #[test]
    fn zero_start_delay_still_backs_off_on_429() {
        // Doubling from zero sticks at zero; the throttling branch enforces
        // a minimum 1s back-off so a 429 without Retry-After still slows
        // the crawl down.
        let mut at = AutoThrottle::new(0.0, 60.0, 1.0);
        let now = t0();
        at.reserve("d", 0.0, now);
        at.record("d", 0.05, 429, None, 0.0, now);
        assert!(at.current_delay("d").unwrap() >= 1.0);
    }

    #[test]
    fn parse_retry_after_seconds_only() {
        assert_eq!(parse_retry_after("5"), Some(5.0));
        assert_eq!(parse_retry_after(" 2.5 "), Some(2.5));
        assert_eq!(parse_retry_after("-1"), None);
        assert_eq!(parse_retry_after("Wed, 21 Oct 2026 07:28:00 GMT"), None);
        assert_eq!(parse_retry_after("soon"), None);
    }
}
