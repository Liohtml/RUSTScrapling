use rust_scrapling::fetchers::config::FetcherConfig;
use rust_scrapling::spiders::session::SessionManager;

#[test]
fn test_session_manager_creation() {
    let mut sm = SessionManager::new(FetcherConfig::default());
    sm.ensure_default();
    // Default session should exist now
}

#[test]
fn test_session_manager_add_session() {
    let mut sm = SessionManager::new(FetcherConfig::default());
    sm.add_session("stealth", FetcherConfig::builder().stealth(true).build());
    sm.ensure_default();
}
