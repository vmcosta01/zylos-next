//! Headless anti-detect browser engine with fingerprint injection and persistent inbox.
//!
//! Provides a simulated headless browser that applies deterministic fingerprints,
//! enforces permission decisions via the persistent inbox, and exposes navigation/interaction APIs.

use serde::{Deserialize, Serialize};

use crate::context::{BrowserContext, BrowserContextConfig, PermissionDecision};
use crate::error::{BrowserError, Result};
use crate::fingerprint::BrowserFingerprint;
use crate::webmcp::WebMcpCompliance;

/// Configuration for the headless browser engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeadlessConfig {
    /// Browser context configuration.
    pub context_config: BrowserContextConfig,
    /// Whether to enable JavaScript execution simulation.
    pub js_enabled: bool,
    /// Maximum navigation timeout in milliseconds.
    pub navigation_timeout_ms: u64,
    /// Whether to block known tracking domains.
    pub block_trackers: bool,
}

impl Default for HeadlessConfig {
    fn default() -> Self {
        Self {
            context_config: BrowserContextConfig::default(),
            js_enabled: true,
            navigation_timeout_ms: 30_000,
            block_trackers: true,
        }
    }
}

/// Represents a loaded page in the headless browser.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageState {
    /// Current URL.
    pub url: String,
    /// Page title (simulated).
    pub title: String,
    /// Origin derived from URL.
    pub origin: String,
    /// Whether the page has finished loading.
    pub loaded: bool,
    /// Simulated DOM content hash.
    pub content_hash: String,
}

/// A navigation request result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavigationResult {
    /// Final URL after redirects.
    pub final_url: String,
    /// HTTP status code (simulated).
    pub status_code: u16,
    /// Time to load in milliseconds.
    pub load_time_ms: u64,
    /// Whether any permissions were requested during navigation.
    pub permissions_requested: Vec<String>,
}

/// Headless anti-detect browser engine.
#[derive(Debug)]
pub struct HeadlessBrowser {
    config: HeadlessConfig,
    context: BrowserContext,
    webmcp: WebMcpCompliance,
    current_page: Option<PageState>,
    navigation_history: Vec<String>,
    blocked_domains: Vec<String>,
}

impl HeadlessBrowser {
    /// Create a new headless browser with anti-detect fingerprinting.
    pub fn new(config: HeadlessConfig) -> Result<Self> {
        let context = BrowserContext::new(config.context_config.clone())?;

        tracing::info!(
            instance_id = %context.fingerprint.instance_id,
            user_agent = %context.fingerprint.user_agent,
            "Headless browser initialized with anti-detect fingerprint"
        );

        Ok(Self {
            config,
            context,
            webmcp: WebMcpCompliance::new(),
            current_page: None,
            navigation_history: Vec::new(),
            blocked_domains: vec![
                "doubleclick.net".to_string(),
                "googlesyndication.com".to_string(),
                "facebook.net".to_string(),
                "analytics.google.com".to_string(),
            ],
        })
    }

    /// Get the active fingerprint for this browser instance.
    pub fn fingerprint(&self) -> &BrowserFingerprint {
        &self.context.fingerprint
    }

    /// Get the underlying browser context.
    pub fn context(&self) -> &BrowserContext {
        &self.context
    }

    /// Get mutable access to the browser context.
    pub fn context_mut(&mut self) -> &mut BrowserContext {
        &mut self.context
    }

    /// Navigate to a URL with anti-detect headers and permission checks.
    pub async fn navigate(&mut self, url: &str) -> Result<NavigationResult> {
        // Validate URL format
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(BrowserError::NavigationFailed(format!(
                "Invalid URL scheme: {}",
                url
            )));
        }

        // Extract origin for permission/tracker checks
        let origin = extract_origin(url)?;

        // Check tracker blocking
        if self.config.block_trackers && self.is_blocked_domain(&origin) {
            tracing::warn!(domain = %origin, "Blocked tracker domain");
            return Err(BrowserError::NavigationFailed(format!(
                "Domain '{}' is blocked by tracker protection",
                origin
            )));
        }

        // Open a tab for this navigation
        self.context.open_tab()?;

        // Simulate navigation with fingerprint-consistent timing
        let load_time_ms = simulate_load_time(url);

        // Generate deterministic content hash from URL + fingerprint
        let content_hash = generate_content_hash(url, &self.context.fingerprint);

        let page = PageState {
            url: url.to_string(),
            title: derive_title(url),
            origin: origin.clone(),
            loaded: true,
            content_hash,
        };

        self.current_page = Some(page.clone());
        self.navigation_history.push(url.to_string());

        tracing::info!(
            url = %url,
            load_time_ms = load_time_ms,
            "Navigation completed"
        );

        Ok(NavigationResult {
            final_url: url.to_string(),
            status_code: 200,
            load_time_ms,
            permissions_requested: Vec::new(),
        })
    }

    /// Execute a permission-gated action on the current page.
    pub fn request_permission(
        &mut self,
        permission_type: &str,
    ) -> PermissionDecision {
        let origin = match &self.current_page {
            Some(page) => page.origin.clone(),
            None => return PermissionDecision::Deny,
        };

        let decision = self.context.check_permission(&origin, permission_type);

        tracing::debug!(
            origin = %origin,
            permission = %permission_type,
            decision = ?decision,
            "Permission check"
        );

        decision
    }

    /// Record a permission decision for the current page's origin.
    pub fn grant_permission(&mut self, permission_type: &str) {
        if let Some(page) = &self.current_page {
            self.context.record_decision(
                &page.origin,
                permission_type,
                PermissionDecision::Allow,
            );
        }
    }

    /// Deny a permission for the current page's origin.
    pub fn deny_permission(&mut self, permission_type: &str) {
        if let Some(page) = &self.current_page {
            self.context.record_decision(
                &page.origin,
                permission_type,
                PermissionDecision::Deny,
            );
        }
    }

    /// Get the current page state, if any.
    pub fn current_page(&self) -> Option<&PageState> {
        self.current_page.as_ref()
    }

    /// Get navigation history.
    pub fn history(&self) -> &[String] {
        &self.navigation_history
    }

    /// Get the WebMCP compliance checker.
    pub fn webmcp(&self) -> &WebMcpCompliance {
        &self.webmcp
    }

    /// Get mutable access to the WebMCP compliance checker.
    pub fn webmcp_mut(&mut self) -> &mut WebMcpCompliance {
        &mut self.webmcp
    }

    /// Verify that the browser fingerprint is consistent with expectations.
    pub fn verify_integrity(&self) -> bool {
        let expected = self.context.fingerprint.consistency_hash();
        self.context.verify_fingerprint(&expected)
    }

    /// Close the current page/tab.
    pub fn close_current_page(&mut self) {
        if self.current_page.is_some() {
            self.context.close_tab();
            self.current_page = None;
        }
    }

    /// Get user-agent string for HTTP requests.
    pub fn user_agent(&self) -> &str {
        &self.context.fingerprint.user_agent
    }

    fn is_blocked_domain(&self, origin: &str) -> bool {
        self.blocked_domains.iter().any(|d| origin.contains(d))
    }
}

/// Extract origin from a URL (scheme + host).
fn extract_origin(url: &str) -> Result<String> {
    // Simple extraction without pulling in the `url` crate
    let without_scheme = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .ok_or_else(|| BrowserError::NavigationFailed("Missing scheme".into()))?;

    let host = without_scheme.split('/').next().unwrap_or(without_scheme);
    Ok(host.to_string())
}

/// Simulate a deterministic load time based on URL length and fingerprint.
fn simulate_load_time(url: &str) -> u64 {
    // Deterministic but varied: 50-500ms range based on URL hash
    let hash_byte = url.bytes().fold(0u8, |acc, b| acc.wrapping_add(b));
    50 + ((hash_byte as u64 * 450) / 255)
}

/// Generate a deterministic content hash from URL and fingerprint.
fn generate_content_hash(url: &str, fp: &BrowserFingerprint) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(url.as_bytes());
    hasher.update(fp.instance_id.as_bytes());
    hex::encode(hasher.finalize())[..32].to_string()
}

/// Derive a simple title from a URL.
fn derive_title(url: &str) -> String {
    let path = url
        .split("://")
        .nth(1)
        .and_then(|s| s.split('/').nth(1))
        .unwrap_or("index");
    format!("{} - Zylos Browser", path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_headless_browser_creation() {
        let config = HeadlessConfig {
            context_config: BrowserContextConfig {
                agent_seed: "headless-test".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };
        let browser = HeadlessBrowser::new(config).unwrap();
        assert!(!browser.fingerprint().instance_id.is_empty());
        assert!(browser.verify_integrity());
    }

    #[tokio::test]
    async fn test_navigation_success() {
        let config = HeadlessConfig {
            context_config: BrowserContextConfig {
                agent_seed: "nav-test".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };
        let mut browser = HeadlessBrowser::new(config).unwrap();
        let result = browser.navigate("https://example.com/page").await.unwrap();
        assert_eq!(result.status_code, 200);
        assert!(result.load_time_ms > 0);
        assert!(browser.current_page().is_some());
        assert_eq!(browser.history().len(), 1);
    }

    #[tokio::test]
    async fn test_navigation_invalid_scheme() {
        let config = HeadlessConfig::default();
        let mut browser = HeadlessBrowser::new(config).unwrap();
        let result = browser.navigate("ftp://example.com").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_tracker_blocking() {
        let config = HeadlessConfig {
            context_config: BrowserContextConfig {
                agent_seed: "tracker-test".to_string(),
                ..Default::default()
            },
            block_trackers: true,
            ..Default::default()
        };
        let mut browser = HeadlessBrowser::new(config).unwrap();
        let result = browser.navigate("https://doubleclick.net/ad").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_permission_flow() {
        let config = HeadlessConfig {
            context_config: BrowserContextConfig {
                agent_seed: "perm-test".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };
        let mut browser = HeadlessBrowser::new(config).unwrap();
        browser.navigate("https://example.com").await.unwrap();

        // First check returns Ask
        assert_eq!(
            browser.request_permission("geolocation"),
            PermissionDecision::Ask
        );

        // Grant permission
        browser.grant_permission("geolocation");
        assert_eq!(
            browser.request_permission("geolocation"),
            PermissionDecision::Allow
        );
    }

    #[tokio::test]
    async fn test_close_page() {
        let config = HeadlessConfig {
            context_config: BrowserContextConfig {
                agent_seed: "close-test".to_string(),
                max_tabs: 2,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut browser = HeadlessBrowser::new(config).unwrap();
        browser.navigate("https://example.com").await.unwrap();
        assert!(browser.current_page().is_some());

        browser.close_current_page();
        assert!(browser.current_page().is_none());
        assert_eq!(browser.context().active_tab_count(), 0);
    }

    #[tokio::test]
    async fn test_deterministic_fingerprint_across_instances() {
        let make_browser = || {
            let config = HeadlessConfig {
                context_config: BrowserContextConfig {
                    agent_seed: "deterministic-seed".to_string(),
                    ..Default::default()
                },
                ..Default::default()
            };
            HeadlessBrowser::new(config).unwrap()
        };

        let b1 = make_browser();
        let b2 = make_browser();
        assert_eq!(b1.fingerprint().instance_id, b2.fingerprint().instance_id);
        assert_eq!(b1.fingerprint().user_agent, b2.fingerprint().user_agent);
        assert_eq!(b1.fingerprint().canvas_hash, b2.fingerprint().canvas_hash);
    }

    #[tokio::test]
    async fn test_user_agent_contains_zylos_bot() {
        let config = HeadlessConfig {
            context_config: BrowserContextConfig {
                agent_seed: "ua-test".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };
        let browser = HeadlessBrowser::new(config).unwrap();
        assert!(browser.user_agent().contains("ZylosBot/"));
    }
}