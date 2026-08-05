//! Shared constants for the fetchers: the stealth user-agent pool, default
//! header values, and the status codes treated as bot blocks.

/// Browser resource types typically blocked by scraping-oriented browser
/// automation to speed up page loads (fonts, images, media, stylesheets,
/// …). Not used by the plain HTTP fetcher itself; provided for browser
/// integrations.
pub const BLOCKED_RESOURCE_TYPES: &[&str] = &[
    "font",
    "image",
    "media",
    "beacon",
    "object",
    "imageset",
    "texttrack",
    "websocket",
    "csp_report",
    "stylesheet",
];

/// Pool of real browser User-Agent strings (current Chrome and Firefox on
/// Windows/macOS/Linux). When no fixed user agent is configured, one is
/// picked deterministically per URL.
pub const USER_AGENTS: &[&str] = &[
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:133.0) Gecko/20100101 Firefox/133.0",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:133.0) Gecko/20100101 Firefox/133.0",
];

/// Bot-blocking / anti-automation status codes only
/// (401/403/407/429/444). 5xx server errors are deliberately excluded:
/// they are genuine server failures, not bot blocks, and classifying them
/// as blocks would wrongly suppress the on_error hook and burn the
/// blocked-retry budget (444 is Nginx's non-standard "connection closed").
pub const BLOCKED_STATUS_CODES: &[u16] = &[401, 403, 407, 429, 444];

/// Default `Accept` header sent in stealth mode (a real browser's
/// navigation Accept value).
pub const ACCEPT_HEADER: &str =
    "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8";

/// Default `Accept-Language` header sent in stealth mode.
pub const ACCEPT_LANGUAGE: &str = "en-US,en;q=0.9";

/// Default `Accept-Encoding` header sent in stealth mode. All three
/// codings are actually supported by the client.
pub const ACCEPT_ENCODING: &str = "gzip, deflate, br";
