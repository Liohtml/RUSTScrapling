use std::collections::HashMap;
use std::time::Duration;
use url::Url;

pub struct RobotsTxtManager {
    cache: HashMap<String, RobotsRules>,
    user_agent: String,
}

struct RobotsRules {
    disallowed: Vec<String>,
    crawl_delay: Option<f64>,
}

/// One contiguous `User-agent:` group from a robots.txt file: a set of
/// applicable agents followed by their directives.
struct RobotsGroup {
    agents: Vec<String>,
    disallow: Vec<String>,
    crawl_delay: Option<f64>,
}

impl RobotsTxtManager {
    pub fn new(user_agent: &str) -> Self {
        Self {
            cache: HashMap::new(),
            user_agent: user_agent.to_string(),
        }
    }

    /// Fetch and parse robots.txt for a domain. Uses a purpose-built HTTP
    /// client with a short timeout so a hanging endpoint cannot block the
    /// crawl setup indefinitely.
    pub async fn fetch_robots(&mut self, domain: &str) {
        let url = format!("https://{}/robots.txt", domain);
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        let rules = match client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => match resp.text().await {
                Ok(text) => Self::parse_robots(&text, &self.user_agent),
                Err(_) => RobotsRules::allow_all(),
            },
            _ => RobotsRules::allow_all(),
        };
        self.cache.insert(domain.to_string(), rules);
    }

    pub fn is_allowed(&self, url: &str) -> bool {
        let domain = match Url::parse(url)
            .ok()
            .and_then(|u| u.host_str().map(|h| h.to_string()))
        {
            Some(d) => d,
            None => return true,
        };

        let path = Url::parse(url)
            .map(|u| u.path().to_string())
            .unwrap_or_else(|_| "/".to_string());

        match self.cache.get(&domain) {
            Some(rules) => {
                for disallowed in &rules.disallowed {
                    if disallowed.is_empty() {
                        continue;
                    }
                    if path.starts_with(disallowed) {
                        return false;
                    }
                }
                true
            }
            None => true,
        }
    }

    pub fn crawl_delay(&self, domain: &str) -> Option<f64> {
        self.cache.get(domain).and_then(|r| r.crawl_delay)
    }

    fn parse_robots(text: &str, user_agent: &str) -> RobotsRules {
        let ua_lower = user_agent.to_lowercase();
        let groups = parse_groups(text);

        // Specific agent match wins over the wildcard group.
        let specific = groups
            .iter()
            .find(|g| g.agents.iter().any(|a| a == &ua_lower));
        let chosen = specific.or_else(|| groups.iter().find(|g| g.agents.iter().any(|a| a == "*")));

        match chosen {
            Some(g) => RobotsRules {
                disallowed: g.disallow.clone(),
                crawl_delay: g.crawl_delay,
            },
            None => RobotsRules::allow_all(),
        }
    }
}

impl RobotsRules {
    fn allow_all() -> Self {
        Self {
            disallowed: Vec::new(),
            crawl_delay: None,
        }
    }
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
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
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
                disallow: Vec::new(),
                crawl_delay: None,
            });
            g.agents.push(rest.trim().to_lowercase());
        } else if let Some(g) = current.as_mut() {
            if let Some(rest) = strip_prefix_ci(line, "disallow:") {
                let path = rest.trim();
                if !path.is_empty() {
                    g.disallow.push(path.to_string());
                }
                saw_directive = true;
            } else if let Some(rest) = strip_prefix_ci(line, "crawl-delay:") {
                if let Ok(d) = rest.trim().parse::<f64>() {
                    g.crawl_delay = Some(d);
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

    #[test]
    fn multi_agent_group_applies_to_each_agent() {
        let txt = "User-agent: MyBot\nUser-agent: Googlebot\nDisallow: /private\n";
        let rules = RobotsTxtManager::parse_robots(txt, "MyBot");
        assert_eq!(rules.disallowed, vec!["/private".to_string()]);
        let rules = RobotsTxtManager::parse_robots(txt, "Googlebot");
        assert_eq!(rules.disallowed, vec!["/private".to_string()]);
    }

    #[test]
    fn specific_agent_wins_over_wildcard() {
        let txt = "User-agent: *\nDisallow: /all\n\nUser-agent: MyBot\nDisallow: /mine\n";
        let rules = RobotsTxtManager::parse_robots(txt, "MyBot");
        assert_eq!(rules.disallowed, vec!["/mine".to_string()]);
    }

    #[test]
    fn wildcard_applies_when_no_specific_match() {
        let txt = "User-agent: *\nDisallow: /all\n";
        let rules = RobotsTxtManager::parse_robots(txt, "OtherBot");
        assert_eq!(rules.disallowed, vec!["/all".to_string()]);
    }

    #[test]
    fn unknown_agent_with_no_wildcard_is_allow_all() {
        let txt = "User-agent: SomeoneElse\nDisallow: /\n";
        let rules = RobotsTxtManager::parse_robots(txt, "MyBot");
        assert!(rules.disallowed.is_empty());
    }

    #[test]
    fn crawl_delay_is_parsed() {
        let txt = "User-agent: *\nCrawl-delay: 5\n";
        let rules = RobotsTxtManager::parse_robots(txt, "MyBot");
        assert_eq!(rules.crawl_delay, Some(5.0));
    }
}
