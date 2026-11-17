//! Search provider trait and built-in provider stubs.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::Result;

/// A single search result from any provider.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SearchResult {
    /// Title of the result.
    pub title: String,
    /// URL of the result.
    pub url: String,
    /// Snippet or description.
    pub snippet: String,
    /// Provider that returned this result.
    pub provider: String,
    /// Relevance score (0.0–1.0, higher is better).
    pub score: f64,
}

/// Metadata about a search execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchMeta {
    /// Total results returned after deduplication.
    pub total_results: usize,
    /// Number of providers queried.
    pub providers_queried: usize,
    /// Number of providers that succeeded.
    pub providers_succeeded: usize,
    /// Wall-clock time in milliseconds.
    pub elapsed_ms: u64,
    /// Per-provider timings in milliseconds.
    pub provider_timings: Vec<(String, u64)>,
}

/// Trait for pluggable search providers.
#[async_trait]
pub trait SearchProvider: Send + Sync {
    /// Human-readable provider name.
    fn name(&self) -> &str;

    /// Execute a search query and return results.
    async fn search(&self, query: &str, max_results: usize) -> Result<Vec<SearchResult>>;
}

/// A mock provider for testing that returns deterministic results.
pub struct MockProvider {
    name: String,
    results: Vec<SearchResult>,
    delay_ms: u64,
}

impl MockProvider {
    /// Create a mock provider that returns the given results with an optional delay.
    pub fn new(name: impl Into<String>, results: Vec<SearchResult>, delay_ms: u64) -> Self {
        Self {
            name: name.into(),
            results,
            delay_ms,
        }
    }
}

#[async_trait]
impl SearchProvider for MockProvider {
    fn name(&self) -> &str {
        &self.name
    }

    async fn search(&self, _query: &str, max_results: usize) -> Result<Vec<SearchResult>> {
        if self.delay_ms > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(self.delay_ms)).await;
        }
        Ok(self.results.iter().take(max_results).cloned().collect())
    }
}

/// A failing provider for testing error handling.
pub struct FailingProvider {
    name: String,
}

impl FailingProvider {
    /// Create a provider that always returns an error, useful for testing failure paths.
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

#[async_trait]
impl SearchProvider for FailingProvider {
    fn name(&self) -> &str {
        &self.name
    }

    async fn search(&self, query: &str, _max_results: usize) -> Result<Vec<SearchResult>> {
        Err(crate::error::SearchError::ProviderFailed(
            self.name.clone(),
            format!("Simulated failure for query: {}", query),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_provider_returns_results() {
        let results = vec![SearchResult {
            title: "Test".to_string(),
            url: "https://example.com".to_string(),
            snippet: "A test result".to_string(),
            provider: "mock".to_string(),
            score: 0.9,
        }];
        let provider = MockProvider::new("mock", results, 0);
        let res = provider.search("test", 10).await.unwrap();
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].title, "Test");
    }

    #[tokio::test]
    async fn test_failing_provider_returns_error() {
        let provider = FailingProvider::new("broken");
        let res = provider.search("test", 10).await;
        assert!(res.is_err());
    }
}
