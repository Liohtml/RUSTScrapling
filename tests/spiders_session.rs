use rust_scrapling::fetchers::config::FetcherConfig;
use rust_scrapling::spiders::session::SessionManager;

#[test]
fn test_session_manager_creation() {
    let mut sm = SessionManager::new(FetcherConfig::default());
    sm.ensure_default().expect("default session should build");
    // Default session should exist now
}

#[test]
fn test_session_manager_add_session() {
    let mut sm = SessionManager::new(FetcherConfig::default());
    sm.add_session("stealth", FetcherConfig::builder().stealth(true).build())
        .expect("session should build");
    sm.ensure_default().expect("default session should build");
}
