//! robots.txt compliance: fetching, RFC 9309 parsing (multi-agent groups,
//! wildcard patterns, longest-match-wins with Allow winning ties), and
//! `Crawl-delay` extraction. Enabled per spider via
//! [`Spider::robots_txt_obey`](crate::spiders::spider::Spider::robots_txt_obey).

use std::collections::HashMap;
use std::time::Duration;
use url::Url;

/// Fetches, parses, and caches robots.txt rules per origin, and answers
/// whether a URL may be crawled.
///
/// Rules are cached per scheme+authority origin (RFC 9309 scoping). When
/// the manager lives behind a lock, use the [`RobotsTxtManager::fetch_rules`]
/// / [`RobotsTxtManager::insert_rules`] split so the network round-trip
/// runs without holding the lock; otherwise
/// [`RobotsTxtManager::fetch_robots`] does both in one call.
pub struct RobotsTxtManager {
    cache: HashMap<String, RobotsRules>,
    user_agent: String,
}

/// Parsed rules for one origin. `rules` holds (allow, pattern) pairs from
/// `Allow:`/`Disallow:` lines; evaluation is longest-match-wins with Allow
/// winning ties (Google's published robots.txt algorithm, which RFC 9309
/// follows).
struct RobotsRules {
    rules: Vec<(bool, String)>,
    crawl_delay: Option<f64>,
}

/// One contiguous `User-agent:` group from a robots.txt file: a set of
/// applicable agents followed by their directives.
struct RobotsGroup {
    agents: Vec<String>,
    rules: Vec<(bool, String)>,
    crawl_delay: Option<f64>,
}

/// Parsed robots.txt rules for one domain, produced by
/// [`RobotsTxtManager::fetch_rules`] and stored via
/// [`RobotsTxtManager::insert_rules`]. Opaque on purpose: the split exists so
/// callers can run the (slow) network fetch WITHOUT borrowing the manager —
/// i.e. without holding a lock on it — and hand the result back afterwards.
pub struct FetchedRobots(RobotsRules);

impl RobotsTxtManager {
    /// Create an empty manager. `user_agent` is the identity robots.txt
    /// `User-agent:` groups are matched against (its product token — the
    /// part before any `/` or space — also matches, per RFC 9309).
    pub fn new(user_agent: &str) -> Self {
        Self {
            cache: HashMap::new(),
            user_agent: user_agent.to_string(),
        }
    }

    /// The user agent string robots.txt groups are matched against.
    pub fn user_agent(&self) -> &str {
        &self.user_agent
    }

    /// Derive the robots.txt origin for a request URL, preserving the URL's
    /// actual scheme and any explicit port (`http://shop.example:8080`), so
    /// robots.txt is fetched from where the site actually lives instead of a
    /// hardcoded `https://host/`. The origin doubles as the rules cache key
    /// — RFC 9309 scopes robots.txt per scheme + authority, so
    /// `http://example.com` and `https://example.com` keep distinct rules.
    /// Returns `None` for URLs without a host (data:, mailto:, malformed),
    /// which skip robots handling entirely.
    pub fn origin_and_key(url: &str) -> Option<(String, String)> {
        let u = Url::parse(url).ok()?;
        let host = u.host_str()?;
        let origin = match u.port() {
            Some(port) => format!("{}://{}:{}", u.scheme(), host, port),
            None => format!("{}://{}", u.scheme(), host),
        };
        Some((origin.clone(), origin))
    }

    /// Fetch and parse robots.txt for an origin without borrowing a manager.
    /// `origin` is either a full origin (`http://host:8080`) or a bare host,
    /// in which case `https://` is assumed. Uses a purpose-built HTTP client
    /// with a short timeout so a hanging endpoint cannot block the caller
    /// indefinitely. Unreachable or non-2xx robots.txt endpoints yield
    /// allow-all rules.
    pub async fn fetch_rules(user_agent: &str, origin: &str) -> FetchedRobots {
        let url = if origin.contains("://") {
            format!("{}/robots.txt", origin)
        } else {
            format!("https://{}/robots.txt", origin)
        };
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        let rules = match client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => match resp.text().await {
                Ok(text) => Self::parse_robots(&text, user_agent),
                Err(_) => RobotsRules::allow_all(),
            },
            _ => RobotsRules::allow_all(),
        };
        FetchedRobots(rules)
    }

    /// Store previously fetched rules under a cache key (see
    /// [`Self::origin_and_key`]).
    pub fn insert_rules(&mut self, key: &str, rules: FetchedRobots) {
        self.cache.insert(key.to_string(), rules.0);
    }

    /// Fetch and store robots.txt for an origin (or bare domain, https
    /// assumed) in one step. Prefer the [`Self::fetch_rules`] +
    /// [`Self::insert_rules`] split when the manager lives behind a lock —
    /// this method borrows `self` for the whole network round-trip.
    pub async fn fetch_robots(&mut self, domain: &str) {
        // Normalize a bare domain to the https origin so the stored key
        // matches what is_allowed derives from https URLs.
        let key = if domain.contains("://") {
            domain.to_string()
        } else {
            format!("https://{}", domain)
        };
        let rules = Self::fetch_rules(&self.user_agent, domain).await;
        self.insert_rules(&key, rules);
    }

    /// Whether `url` is allowed by the stored rules for its origin. Origins
    /// without stored rules are allowed (robots.txt not fetched or absent).
    /// Matching runs against the URL's path plus query string, per the
    /// robots.txt matching rules.
    pub fn is_allowed(&self, url: &str) -> bool {
        let key = match Self::origin_and_key(url) {
            Some((_, key)) => key,
            None => return true,
        };
        let parsed = match Url::parse(url) {
            Ok(u) => u,
            Err(_) => return true,
        };
        let mut target = parsed.path().to_string();
        if let Some(q) = parsed.query() {
            target.push('?');
            target.push_str(q);
        }

        match self.cache.get(&key) {
            Some(rules) => rules.is_path_allowed(&target),
            None => true,
        }
    }

    /// Crawl-delay for a cache key (see [`Self::origin_and_key`]), if the
    /// origin's robots.txt declared one for our user agent.
    pub fn crawl_delay(&self, key: &str) -> Option<f64> {
        self.cache.get(key).and_then(|r| r.crawl_delay)
    }

    /// Whether robots.txt has already been fetched (cached) for this key.
    pub fn has_domain(&self, key: &str) -> bool {
        self.cache.contains_key(key)
    }

    fn parse_robots(text: &str, user_agent: &str) -> RobotsRules {
        let ua_lower = user_agent.to_lowercase();
        // RFC 9309 matches groups against the crawler's product token, so
        // "User-agent: MyBot" applies to a crawler identifying as
        // "MyBot/1.0 (+https://example.com/bot)".
        let ua_token = ua_lower
            .split(['/', ' '])
            .next()
            .unwrap_or(&ua_lower)
            .to_string();
        let groups = parse_groups(text);

        // Per RFC 9309 all groups matching the agent are combined. A
        // specific agent match wins over the wildcard groups.
        let specific: Vec<&RobotsGroup> = groups
            .iter()
            .filter(|g| g.agents.iter().any(|a| a == &ua_lower || a == &ua_token))
            .collect();
        let chosen: Vec<&RobotsGroup> = if specific.is_empty() {
            groups
                .iter()
                .filter(|g| g.agents.iter().any(|a| a == "*"))
                .collect()
        } else {
            specific
        };

        if chosen.is_empty() {
            return RobotsRules::allow_all();
        }
        RobotsRules {
            rules: chosen.iter().flat_map(|g| g.rules.clone()).collect(),
            crawl_delay: chosen.iter().find_map(|g| g.crawl_delay),
        }
    }
}

impl RobotsRules {
    fn allow_all() -> Self {
        Self {
            rules: Vec::new(),
            crawl_delay: None,
        }
    }

    /// Longest-match-wins over all matching Allow/Disallow patterns;
    /// specificity is the pattern's length and Allow wins exact ties. No
    /// matching rule means allowed.
    fn is_path_allowed(&self, target: &str) -> bool {
        let mut best: Option<(usize, bool)> = None;
        for (allow, pattern) in &self.rules {
            if pattern_matches(pattern, target) {
                let specificity = pattern.len();
                best = match best {
                    None => Some((specificity, *allow)),
                    Some((best_len, best_allow)) => {
                        if specificity > best_len
                            || (specificity == best_len && *allow && !best_allow)
                        {
                            Some((specificity, *allow))
                        } else {
                            Some((best_len, best_allow))
                        }
                    }
                };
            }
        }
        best.map(|(_, allow)| allow).unwrap_or(true)
    }
}

/// robots.txt pattern matching: patterns are path prefixes, `*` matches any
/// character sequence, and a trailing `$` anchors the match to the end of
/// the target.
fn pattern_matches(pattern: &str, target: &str) -> bool {
    let (pat, anchored) = match pattern.strip_suffix('$') {
        Some(p) => (p, true),
        None => (pattern, false),
    };
    let parts: Vec<&str> = pat.split('*').collect();

    if parts.len() == 1 {
        // No wildcards: plain prefix, or exact match when anchored.
        return if anchored {
            target == parts[0]
        } else {
            target.starts_with(parts[0])
        };
    }

    // First fragment is anchored at the start.
    if !target.starts_with(parts[0]) {
        return false;
    }
    let mut pos = parts[0].len();

    // For anchored patterns the LAST fragment must sit at the very end of
    // the target, so reserve that region and match the middle fragments
    // leftmost-first inside what remains. (A naive leftmost match of the
    // last fragment would wrongly reject e.g. pattern `/*.php$` against
    // `/a.php.php`.)
    let (middle, limit) = if anchored {
        let last = parts[parts.len() - 1];
        if !target.ends_with(last) || target.len() < pos + last.len() {
            return false;
        }
        (&parts[1..parts.len() - 1], target.len() - last.len())
    } else {
        (&parts[1..], target.len())
    };

    for part in middle {
        if part.is_empty() {
            continue; // consecutive '*' collapse
        }
        match target[pos..limit].find(part) {
            Some(idx) => pos += idx + part.len(),
            None => return false,
        }
    }
    pos <= limit
}

/// Parse robots.txt into groups. A group starts at one or more consecutive
/// `User-agent:` lines and ends when the next `User-agent:` line appears
/// after at least one directive (handling the standard multi-agent grouping
/// where several `User-agent:` lines share a single rule block).
fn parse_groups(text: &str) -> Vec<RobotsGroup> {
    let mut groups: Vec<RobotsGroup> = Vec::new();
    let mut current: Option<RobotsGroup> = None;
    let mut saw_directive = false;

    for raw in text.lines() {
        // Strip inline comments, then whitespace.
        let line = match raw.find('#') {
            Some(idx) => raw[..idx].trim(),
            None => raw.trim(),
        };
        if line.is_empty() {
            continue;
        }

        if let Some(rest) = strip_prefix_ci(line, "user-agent:") {
            if saw_directive {
                if let Some(g) = current.take() {
                    groups.push(g);
                }
                saw_directive = false;
            }
            let g = current.get_or_insert_with(|| RobotsGroup {
                agents: Vec::new(),
                rules: Vec::new(),
                crawl_delay: None,
            });
            g.agents.push(rest.trim().to_lowercase());
        } else if let Some(g) = current.as_mut() {
            if let Some(rest) = strip_prefix_ci(line, "disallow:") {
                let path = rest.trim();
                // An empty Disallow means "allow everything" — no rule.
                if !path.is_empty() {
                    g.rules.push((false, path.to_string()));
                }
                saw_directive = true;
            } else if let Some(rest) = strip_prefix_ci(line, "allow:") {
                let path = rest.trim();
                if !path.is_empty() {
                    g.rules.push((true, path.to_string()));
                }
                saw_directive = true;
            } else if let Some(rest) = strip_prefix_ci(line, "crawl-delay:") {
                // Reject non-finite, negative, and absurd values: f64
                // parsing accepts "inf"/"NaN"/"1e300", and a hostile
                // robots.txt must not be able to panic the crawler
                // (Duration::from_secs_f64 panics on non-finite/oversized
                // input) or stall it forever. 24h is already unreasonable.
                if let Ok(d) = rest.trim().parse::<f64>() {
                    if d.is_finite() && (0.0..=86_400.0).contains(&d) {
                        g.crawl_delay = Some(d);
                    }
                }
                saw_directive = true;
            }
        }
    }
    if let Some(g) = current.take() {
        groups.push(g);
    }
    groups
}

fn strip_prefix_ci<'a>(line: &'a str, prefix: &str) -> Option<&'a str> {
    if line.len() >= prefix.len()
        && line.as_bytes()[..prefix.len()].eq_ignore_ascii_case(prefix.as_bytes())
    {
        Some(&line[prefix.len()..])
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mgr_with(txt: &str, ua: &str, key: &str) -> RobotsTxtManager {
        let mut mgr = RobotsTxtManager::new(ua);
        mgr.insert_rules(key, FetchedRobots(RobotsTxtManager::parse_robots(txt, ua)));
        mgr
    }

    #[test]
    fn multi_agent_group_applies_to_each_agent() {
        let txt = "User-agent: MyBot\nUser-agent: Googlebot\nDisallow: /private\n";
        for ua in ["MyBot", "Googlebot"] {
            let mgr = mgr_with(txt, ua, "https://example.com");
            assert!(!mgr.is_allowed("https://example.com/private/x"));
            assert!(mgr.is_allowed("https://example.com/public"));
        }
    }

    #[test]
    fn specific_agent_wins_over_wildcard() {
        let txt = "User-agent: *\nDisallow: /all\n\nUser-agent: MyBot\nDisallow: /mine\n";
        let mgr = mgr_with(txt, "MyBot", "https://example.com");
        assert!(!mgr.is_allowed("https://example.com/mine"));
        assert!(mgr.is_allowed("https://example.com/all"));
    }

    #[test]
    fn wildcard_applies_when_no_specific_match() {
        let txt = "User-agent: *\nDisallow: /all\n";
        let mgr = mgr_with(txt, "OtherBot", "https://example.com");
        assert!(!mgr.is_allowed("https://example.com/all"));
    }

    #[test]
    fn unknown_agent_with_no_wildcard_is_allow_all() {
        let txt = "User-agent: SomeoneElse\nDisallow: /\n";
        let mgr = mgr_with(txt, "MyBot", "https://example.com");
        assert!(mgr.is_allowed("https://example.com/anything"));
    }

    #[test]
    fn crawl_delay_is_parsed() {
        let txt = "User-agent: *\nCrawl-delay: 5\n";
        let rules = RobotsTxtManager::parse_robots(txt, "MyBot");
        assert_eq!(rules.crawl_delay, Some(5.0));
    }

    #[test]
    fn hostile_crawl_delay_values_are_rejected() {
        // f64 parsing accepts "inf"/"NaN"/"1e300"; a hostile robots.txt
        // must not be able to panic the crawler (Duration::from_secs_f64
        // panics on non-finite/oversized input) or stall it forever.
        for bad in ["inf", "-inf", "NaN", "1e300", "-5", "999999999"] {
            let txt = format!("User-agent: *\nCrawl-delay: {}\n", bad);
            let rules = RobotsTxtManager::parse_robots(&txt, "MyBot");
            assert_eq!(rules.crawl_delay, None, "value {:?} must be rejected", bad);
        }
        // Sane values still parse.
        let rules = RobotsTxtManager::parse_robots("User-agent: *\nCrawl-delay: 2.5\n", "MyBot");
        assert_eq!(rules.crawl_delay, Some(2.5));
    }

    #[test]
    fn allow_overrides_broader_disallow() {
        // The classic pattern the old prefix-only matcher got wrong:
        // Disallow: / + Allow: /public must NOT block everything.
        let txt = "User-agent: *\nDisallow: /\nAllow: /public\n";
        let mgr = mgr_with(txt, "MyBot", "https://example.com");
        assert!(mgr.is_allowed("https://example.com/public"));
        assert!(mgr.is_allowed("https://example.com/public/page"));
        assert!(!mgr.is_allowed("https://example.com/private"));
        assert!(!mgr.is_allowed("https://example.com/"));
    }

    #[test]
    fn longest_match_wins_regardless_of_order() {
        let txt = "User-agent: *\nAllow: /a\nDisallow: /a/b\nAllow: /a/b/c\n";
        let mgr = mgr_with(txt, "MyBot", "https://example.com");
        assert!(mgr.is_allowed("https://example.com/a/x"));
        assert!(!mgr.is_allowed("https://example.com/a/b/x"));
        assert!(mgr.is_allowed("https://example.com/a/b/c/x"));
    }

    #[test]
    fn allow_wins_specificity_ties() {
        let txt = "User-agent: *\nDisallow: /page\nAllow: /page\n";
        let mgr = mgr_with(txt, "MyBot", "https://example.com");
        assert!(mgr.is_allowed("https://example.com/page"));
    }

    #[test]
    fn star_wildcard_matches_inside_paths() {
        // `?` is a literal in robots patterns: `/*?sessionid` requires
        // sessionid directly after the `?`; `/*sessionid=` matches it
        // anywhere in the path-plus-query target.
        let txt = "User-agent: *\nDisallow: /*?sessionid\nDisallow: /*sessionid=\n";
        let mgr = mgr_with(txt, "MyBot", "https://example.com");
        // Query strings are part of the match target.
        assert!(!mgr.is_allowed("https://example.com/page?sessionid=42"));
        assert!(!mgr.is_allowed("https://example.com/a/b?x=1&sessionid=2"));
        assert!(mgr.is_allowed("https://example.com/page"));

        let only_direct = mgr_with(
            "User-agent: *\nDisallow: /*?sessionid\n",
            "MyBot",
            "https://example.com",
        );
        // Literal `?sessionid` does not match `&sessionid`.
        assert!(only_direct.is_allowed("https://example.com/a/b?x=1&sessionid=2"));
    }

    #[test]
    fn dollar_anchors_to_end() {
        let txt = "User-agent: *\nDisallow: /*.php$\n";
        let mgr = mgr_with(txt, "MyBot", "https://example.com");
        assert!(!mgr.is_allowed("https://example.com/index.php"));
        // A later occurrence of the suffix still anchors: /a.php.php ends
        // with .php even though the FIRST .php is mid-string.
        assert!(!mgr.is_allowed("https://example.com/a.php.php"));
        assert!(mgr.is_allowed("https://example.com/a.php.bak"));
    }

    #[test]
    fn pattern_matching_edge_cases() {
        assert!(pattern_matches("/", "/anything"));
        assert!(pattern_matches("/a*b*c", "/a-x-b-y-c-z"));
        assert!(!pattern_matches("/a*b$", "/a-b-tail"));
        assert!(pattern_matches("/a*b$", "/a-x-b"));
        assert!(pattern_matches("/a**b", "/ab"));
        assert!(pattern_matches("/exact$", "/exact"));
        assert!(!pattern_matches("/exact$", "/exact/more"));
        // Overlap guard: pattern /a$ with wildcard prefix cannot match a
        // target shorter than first+last fragments combined.
        assert!(!pattern_matches("/aa*aa$", "/aaa"));
    }

    #[test]
    fn inline_comments_are_stripped() {
        let txt = "User-agent: * # everyone\nDisallow: /private # keep out\n";
        let mgr = mgr_with(txt, "MyBot", "https://example.com");
        assert!(!mgr.is_allowed("https://example.com/private/x"));
        assert!(mgr.is_allowed("https://example.com/open"));
    }

    #[test]
    fn multiple_groups_for_same_agent_are_combined() {
        let txt = "User-agent: MyBot\nDisallow: /one\n\nUser-agent: MyBot\nDisallow: /two\n";
        let mgr = mgr_with(txt, "MyBot", "https://example.com");
        assert!(!mgr.is_allowed("https://example.com/one"));
        assert!(!mgr.is_allowed("https://example.com/two"));
        assert!(mgr.is_allowed("https://example.com/three"));
    }

    #[test]
    fn origin_and_key_preserve_scheme_and_port() {
        assert_eq!(
            RobotsTxtManager::origin_and_key("http://shop.example:8080/x?y=1"),
            Some((
                "http://shop.example:8080".to_string(),
                "http://shop.example:8080".to_string()
            ))
        );
        assert_eq!(
            RobotsTxtManager::origin_and_key("https://example.com/path"),
            Some((
                "https://example.com".to_string(),
                "https://example.com".to_string()
            ))
        );
        // Default port is normalized away by the URL parser.
        assert_eq!(
            RobotsTxtManager::origin_and_key("https://example.com:443/x"),
            Some((
                "https://example.com".to_string(),
                "https://example.com".to_string()
            ))
        );
        assert_eq!(RobotsTxtManager::origin_and_key("data:text/plain,hi"), None);
    }

    #[test]
    fn distinct_ports_have_distinct_rules() {
        let mut mgr = RobotsTxtManager::new("MyBot");
        mgr.insert_rules(
            "http://example.com:8080",
            FetchedRobots(RobotsTxtManager::parse_robots(
                "User-agent: *\nDisallow: /\n",
                "MyBot",
            )),
        );
        assert!(!mgr.is_allowed("http://example.com:8080/x"));
        // No rules stored for the default-port origin -> allowed.
        assert!(mgr.is_allowed("http://example.com/x"));
    }

    #[test]
    fn distinct_schemes_have_distinct_rules() {
        // RFC 9309 scopes robots.txt per scheme + authority: rules fetched
        // for the http origin must not govern the https origin.
        let mut mgr = RobotsTxtManager::new("MyBot");
        mgr.insert_rules(
            "http://example.com",
            FetchedRobots(RobotsTxtManager::parse_robots(
                "User-agent: *\nDisallow: /\n",
                "MyBot",
            )),
        );
        assert!(!mgr.is_allowed("http://example.com/x"));
        assert!(mgr.is_allowed("https://example.com/x"));
    }

    #[test]
    fn user_agent_product_token_matches_group() {
        // A crawler identifying as "MyBot/1.0 (+url)" must match the
        // "User-agent: MyBot" group (RFC 9309 product-token matching).
        let txt = "User-agent: MyBot\nDisallow: /private\n";
        let mgr = mgr_with(
            txt,
            "MyBot/1.0 (+https://example.com/bot)",
            "https://example.com",
        );
        assert!(!mgr.is_allowed("https://example.com/private/x"));
        assert!(mgr.is_allowed("https://example.com/open"));
    }

    #[test]
    fn insert_rules_stores_fetched_rules_under_domain() {
        // The fetch_rules/insert_rules split exists so the (slow) network
        // fetch can run without borrowing the manager; verify the two-step
        // path behaves like the one-step fetch_robots.
        let mut mgr = RobotsTxtManager::new("MyBot");
        assert!(!mgr.has_domain("https://example.com"));
        assert_eq!(mgr.user_agent(), "MyBot");

        let rules = FetchedRobots(RobotsTxtManager::parse_robots(
            "User-agent: *\nDisallow: /private\nCrawl-delay: 2\n",
            "MyBot",
        ));
        mgr.insert_rules("https://example.com", rules);

        assert!(mgr.has_domain("https://example.com"));
        assert!(!mgr.is_allowed("https://example.com/private/page"));
        assert!(mgr.is_allowed("https://example.com/public"));
        assert_eq!(mgr.crawl_delay("https://example.com"), Some(2.0));
    }

    #[tokio::test]
    async fn fetch_robots_normalizes_bare_domain_to_https_key() {
        // The .invalid TLD (RFC 2606) can never resolve: the fetch must
        // fall back to allow-all rather than erroring or hanging. The bare
        // domain is stored under its https origin so https-URL lookups hit
        // the entry.
        let mut mgr = RobotsTxtManager::new("MyBot");
        mgr.fetch_robots("unreachable.invalid").await;
        assert!(mgr.has_domain("https://unreachable.invalid"));
        assert!(mgr.is_allowed("https://unreachable.invalid/anything"));
    }
}
