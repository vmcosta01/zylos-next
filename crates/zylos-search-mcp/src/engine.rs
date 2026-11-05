//! Parallel search engine with fan-out, deduplication, and result merging.

use futures::future::join_all;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;

use crate::error::{Result, SearchError};
use crate::provider::{SearchMeta, SearchProvider, SearchResult};

/// Configuration for the parallel search engine.
#[derive(Debug, Clone)]
pub struct SearchEngineConfig {
    /// Maximum number of concurrent provider queries.
    pub max_concurrency: usize,
    /// Per-provider timeout in milliseconds.
    pub provider_timeout_ms: u64,
    /// Global timeout in milliseconds.
    pub global_timeout_ms: u64,
    /// Maximum results to return after merging.
    pub max_results: usize,
}

impl Default for SearchEngineConfig {
    fn default() -> Self {
        Self {
            max_concurrency: 5,
            provider_timeout_ms: 5000,
            global_timeout_ms: 10000,
            max_results: 50,
        }
    }
}

/// Response from a parallel search execution.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SearchResponse {
    /// Merged and deduplicated results.
    pub results: Vec<SearchResult>,
    /// Execution metadata.
    pub meta: SearchMeta,
}

/// Parallel search engine that fans out queries to multiple providers.
pub struct SearchEngine {
    config: SearchEngineConfig,
    providers: Vec<Arc<dyn SearchProvider>>,
}

impl SearchEngine {
    /// Create a new search engine with the given configuration.
    pub fn new(config: SearchEngineConfig) -> Self {
        Self {
            config,
            providers: Vec::new(),
        }
    }

    /// Register a search provider.
    pub fn add_provider(&mut self, provider: Arc<dyn SearchProvider>) {
        self.providers.push(provider);
    }

    /// Get the number of registered providers.
    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }

    /// Execute a parallel search across all registered providers.
    pub async fn search(&self, query: &str) -> Result<SearchResponse> {
        if query.trim().is_empty() {
            return Err(SearchError::InvalidQuery("Empty query".to_string()));
        }

        if self.providers.is_empty() {
            return Err(SearchError::AllProvidersFailed(query.to_string()));
        }

        let start = Instant::now();
        let per_provider_max = self.config.max_results / self.providers.len().max(1) + 1;

        // Fan out: query all providers concurrently
        let futures: Vec<_> = self
            .providers
            .iter()
            .map(|p| {
                let provider = p.clone();
                let q = query.to_string();
                let timeout = std::time::Duration::from_millis(self.config.provider_timeout_ms);
                async move {
                    let t = Instant::now();
                    let result = tokio::time::timeout(timeout, provider.search(&q, per_provider_max))
                        .await;
                    let elapsed = t.elapsed().as_millis() as u64;
                    match result {
                        Ok(Ok(results)) => (provider.name().to_string(), Ok(results), elapsed),
                        Ok(Err(e)) => (provider.name().to_string(), Err(e), elapsed),
                        Err(_) => (
                            provider.name().to_string(),
                            Err(SearchError::Timeout(timeout.as_secs())),
                            elapsed,
                        ),
                    }
                }
            })
            .collect();

        let outcomes = join_all(futures).await;

        // Merge results, track timings
        let mut all_results: Vec<SearchResult> = Vec::new();
        let mut provider_timings: Vec<(String, u64)> = Vec::new();
        let mut succeeded = 0usize;

        for (name, result, elapsed) in outcomes {
            provider_timings.push((name, elapsed));
            match result {
                Ok(results) => {
                    succeeded += 1;
                    all_results.extend(results);
                }
                Err(e) => {
                    tracing::warn!(provider = %provider_timings.last().unwrap().0, error = %e, "Provider failed");
                }
            }
        }

        if succeeded == 0 {
            return Err(SearchError::AllProvidersFailed(query.to_string()));
        }

        // Deduplicate by URL
        let deduped = deduplicate_results(all_results);

        // Sort by score descending
        let mut sorted = deduped;
        sorted.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        // Truncate to max_results
        sorted.truncate(self.config.max_results);

        let elapsed_ms = start.elapsed().as_millis() as u64;

        let total_results = sorted.len();

        Ok(SearchResponse {
            results: sorted,
            meta: SearchMeta {
                total_results,
                providers_queried: self.providers.len(),
                providers_succeeded: succeeded,
                elapsed_ms,
                provider_timings,
            },
        })
    }
}

/// Deduplicate search results by URL, keeping the highest-scoring entry.
fn deduplicate_results(results: Vec<SearchResult>) -> Vec<SearchResult> {
    let mut seen_urls: HashSet<String> = HashSet::new();
    let mut deduped: Vec<SearchResult> = Vec::with_capacity(results.len());

    // Sort by score first so we keep the best version
    let mut sorted = results;
    sorted.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

    for result in sorted {
        let url_hash = url_hash(&result.url);
        if seen_urls.insert(url_hash) {
            deduped.push(result);
        }
    }

    deduped
}

/// Compute a normalized hash of a URL for deduplication.
fn url_hash(url: &str) -> String {
    let normalized = url.trim_end_matches('/').to_lowercase();
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::{FailingProvider, MockProvider};

    fn make_result(title: &str, url: &str, score: f64, provider: &str) -> SearchResult {
        SearchResult {
            title: title.to_string(),
            url: url.to_string(),
            snippet: format!("Snippet for {}", title),
            provider: provider.to_string(),
            score,
        }
    }

    #[tokio::test]
    async fn test_parallel_search_merges_results() {
        let mut engine = SearchEngine::new(SearchEngineConfig::default());

        let p1 = Arc::new(MockProvider::new(
            "provider-a",
            vec![
                make_result("Result A1", "https://a.com/1", 0.9, "provider-a"),
                make_result("Result A2", "https://a.com/2", 0.7, "provider-a"),
            ],
            0,
        ));
        let p2 = Arc::new(MockProvider::new(
            "provider-b",
            vec![
                make_result("Result B1", "https://b.com/1", 0.8, "provider-b"),
            ],
            0,
        ));

        engine.add_provider(p1);
        engine.add_provider(p2);

        let response = engine.search("test query").await.unwrap();
        assert_eq!(response.meta.providers_queried, 2);
        assert_eq!(response.meta.providers_succeeded, 2);
        assert_eq!(response.results.len(), 3);
        // Should be sorted by score
        assert!(response.results[0].score >= response.results[1].score);
    }

    #[tokio::test]
    async fn test_deduplication_by_url() {
        let mut engine = SearchEngine::new(SearchEngineConfig::default());

        let p1 = Arc::new(MockProvider::new(
            "provider-a",
            vec![make_result("Title A", "https://same.com/page", 0.9, "provider-a")],
            0,
        ));
        let p2 = Arc::new(MockProvider::new(
            "provider-b",
            vec![make_result("Title B", "https://same.com/page", 0.5, "provider-b")],
            0,
        ));

        engine.add_provider(p1);
        engine.add_provider(p2);

        let response = engine.search("dup test").await.unwrap();
        assert_eq!(response.results.len(), 1);
        // Higher score should win
        assert_eq!(response.results[0].score, 0.9);
    }

    #[tokio::test]
    async fn test_partial_provider_failure() {
        let mut engine = SearchEngine::new(SearchEngineConfig::default());

        let good = Arc::new(MockProvider::new(
            "good",
            vec![make_result("Good", "https://good.com", 0.8, "good")],
            0,
        ));
        let bad = Arc::new(FailingProvider::new("bad"));

        engine.add_provider(good);
        engine.add_provider(bad);

        let response = engine.search("partial fail").await.unwrap();
        assert_eq!(response.meta.providers_succeeded, 1);
        assert_eq!(response.meta.providers_queried, 2);
        assert_eq!(response.results.len(), 1);
    }

    #[tokio::test]
    async fn test_all_providers_fail() {
        let mut engine = SearchEngine::new(SearchEngineConfig::default());
        engine.add_provider(Arc::new(FailingProvider::new("fail1")));
        engine.add_provider(Arc::new(FailingProvider::new("fail2")));

        let result = engine.search("all fail").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_empty_query_rejected() {
        let engine = SearchEngine::new(SearchEngineConfig::default());
        let result = engine.search("").await;
        assert!(matches!(result, Err(SearchError::InvalidQuery(_))));
    }

    #[tokio::test]
    async fn test_no_providers_returns_error() {
        let engine = SearchEngine::new(SearchEngineConfig::default());
        let result = engine.search("no providers").await;
        assert!(matches!(result, Err(SearchError::AllProvidersFailed(_))));
    }
}