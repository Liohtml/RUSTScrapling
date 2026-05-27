use crate::fetchers::client::Fetcher;
use crate::fetchers::config::FetcherConfig;
use crate::fetchers::response::Response;
use crate::spiders::request::SpiderRequest;
use std::collections::HashMap;

pub struct SessionManager {
    sessions: HashMap<String, Fetcher>,
    default_config: FetcherConfig,
}

impl SessionManager {
    pub fn new(default_config: FetcherConfig) -> Self {
        Self {
            sessions: HashMap::new(),
            default_config,
        }
    }

    pub fn add_session(&mut self, name: &str, config: FetcherConfig) {
        self.sessions.insert(name.to_string(), Fetcher::new(config));
    }

    pub fn ensure_default(&mut self) {
        if !self.sessions.contains_key("default") {
            self.sessions.insert(
                "default".to_string(),
                Fetcher::new(self.default_config.clone()),
            );
        }
    }

    /// Fetch using the session specified in the request (or "default").
    pub async fn fetch(&self, request: &SpiderRequest) -> Result<Response, String> {
        let session_id = if request.session_id().is_empty() {
            "default"
        } else {
            request.session_id()
        };
        let fetcher = self
            .sessions
            .get(session_id)
            .ok_or_else(|| format!("Session '{}' not found", session_id))?;

        match request.method() {
            "GET" => fetcher.get(request.url()).await.map_err(|e| e.to_string()),
            "POST" => fetcher
                .post(request.url(), request.body(), None)
                .await
                .map_err(|e| e.to_string()),
            "PUT" => fetcher
                .put(request.url(), request.body(), None)
                .await
                .map_err(|e| e.to_string()),
            "DELETE" => fetcher
                .delete(request.url())
                .await
                .map_err(|e| e.to_string()),
            m => Err(format!("Unsupported HTTP method: {}", m)),
        }
    }
}
