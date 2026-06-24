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

pub const USER_AGENTS: &[&str] = &[
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:133.0) Gecko/20100101 Firefox/133.0",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:133.0) Gecko/20100101 Firefox/133.0",
];

// Bot-blocking / anti-automation status codes only. 5xx server errors are
// deliberately excluded: they are genuine server failures, not bot blocks, and
// classifying them as blocks would wrongly suppress the on_error hook and burn
// the blocked-retry budget (444 is Nginx's non-standard "connection closed").
pub const BLOCKED_STATUS_CODES: &[u16] = &[401, 403, 407, 429, 444];

pub const ACCEPT_HEADER: &str =
    "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8";

pub const ACCEPT_LANGUAGE: &str = "en-US,en;q=0.9";

pub const ACCEPT_ENCODING: &str = "gzip, deflate, br";
