use std::collections::HashMap;
use url::Url;

pub struct RobotsTxtManager {
    cache: HashMap<String, RobotsRules>,
    user_agent: String,
}

struct RobotsRules {
    disallowed: Vec<String>,
    crawl_delay: Option<f64>,
}

impl RobotsTxtManager {
    pub fn new(user_agent: &str) -> Self {
        Self {
            cache: HashMap::new(),
            user_agent: user_agent.to_string(),
        }
    }

    /// Fetch and parse robots.txt for a domain.
    pub async fn fetch_robots(&mut self, domain: &str) {
        let url = format!("https://{}/robots.txt", domain);
        let rules = match reqwest::get(&url).await {
            Ok(resp) => {
                if resp.status().is_success() {
                    match resp.text().await {
                        Ok(text) => Self::parse_robots(&text, &self.user_agent),
                        Err(_) => RobotsRules {
                            disallowed: Vec::new(),
                            crawl_delay: None,
                        },
                    }
                } else {
                    // Non-success status: allow all
                    RobotsRules {
                        disallowed: Vec::new(),
                        crawl_delay: None,
                    }
                }
            }
            Err(_) => {
                // On error, allow all
                RobotsRules {
                    disallowed: Vec::new(),
                    crawl_delay: None,
                }
            }
        };
        self.cache.insert(domain.to_string(), rules);
    }

    pub fn is_allowed(&self, url: &str) -> bool {
        let domain = match Url::parse(url).ok().and_then(|u| u.host_str().map(|h| h.to_string())) {
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
            None => true, // No rules cached = allow
        }
    }

    pub fn crawl_delay(&self, domain: &str) -> Option<f64> {
        self.cache.get(domain).and_then(|r| r.crawl_delay)
    }

    fn parse_robots(text: &str, user_agent: &str) -> RobotsRules {
        let mut disallowed = Vec::new();
        let mut crawl_delay = None;
        let mut in_matching_section = false;
        let mut found_specific = false;
        let ua_lower = user_agent.to_lowercase();

        // First pass: look for specific user-agent match
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some(rest) = line.strip_prefix("User-agent:").or_else(|| line.strip_prefix("user-agent:")) {
                let agent = rest.trim().to_lowercase();
                if agent == ua_lower || agent == "*" {
                    // Prefer specific match over wildcard
                    if agent == ua_lower {
                        found_specific = true;
                        disallowed.clear();
                        crawl_delay = None;
                        in_matching_section = true;
                    } else {
                        in_matching_section = !found_specific;
                    }
                } else {
                    if !found_specific {
                        // Could be another agent section
                    }
                    in_matching_section = false;
                }
            } else if in_matching_section {
                if let Some(path) = line.strip_prefix("Disallow:").or_else(|| line.strip_prefix("disallow:")) {
                    let path = path.trim();
                    if !path.is_empty() {
                        disallowed.push(path.to_string());
                    }
                } else if let Some(delay) = line.strip_prefix("Crawl-delay:").or_else(|| line.strip_prefix("crawl-delay:")) {
                    if let Ok(d) = delay.trim().parse::<f64>() {
                        crawl_delay = Some(d);
                    }
                }
            }
        }

        RobotsRules {
            disallowed,
            crawl_delay,
        }
    }
}
