//! Caching Middleware — Task 6.3.3
//!
//! Multi-layer HTTP caching for cockpit endpoints to achieve >80% cache hit rate
//! and p99 latency <200ms per Master Plan Sprint 6.3 targets.

use axum::{
    extract::Request,
    http::{header, HeaderValue, StatusCode},
    middleware::Next,
    response::Response,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Cache entry with expiration
#[derive(Clone)]
struct CacheEntry {
    body: Vec<u8>,
    status: StatusCode,
    content_type: String,
    created_at: Instant,
}

impl CacheEntry {
    fn is_expired(&self, ttl: Duration) -> bool {
        self.created_at.elapsed() > ttl
    }
}

/// In-memory HTTP cache with TTL-based eviction
#[derive(Clone)]
/// Estrutura HttpCache.
pub struct HttpCache {
    entries: Arc<RwLock<HashMap<String, CacheEntry>>>,
    default_ttl: Duration,
    max_entries: usize,
}

impl HttpCache {
    /// Função new.
    pub fn new(default_ttl_secs: u64, max_entries: usize) -> Self {
        // Variante Self.
        Self {
            entries: Arc::new(RwLock::new(HashMap::with_capacity(max_entries.min(10000)))),
            default_ttl: Duration::from_secs(default_ttl_secs),
            max_entries,
        }
    }

    /// Build cache key from method + path + query
    pub fn cache_key(request: &Request) -> String {
        let method = request.method().as_str();
        let uri = request.uri();
        match uri.query() {
            // Variante Some.
            Some(q) => format!("{}:{}?{}", method, uri.path(), q),
            // Variante None.
            None => format!("{}:{}", method, uri.path()),
        }
    }

    /// Check if a GET request is cacheable (only GET, no Authorization header)
    pub fn is_cacheable(request: &Request) -> bool {
        request.method().as_str() == "GET"
            && !request.headers().contains_key(header::AUTHORIZATION)
            && !request.uri().path().starts_with("/events")
            && !request.uri().path().starts_with("/ws")
    }

    /// Try to get a cached response
    pub async fn get(&self, key: &str) -> Option<Response> {
        let entries = self.entries.read().await;
        let entry = entries.get(key)?;

        if entry.is_expired(self.default_ttl) {
            return None;
        }

        let response = Response::builder()
            .status(entry.status)
            .header(header::CONTENT_TYPE, &entry.content_type)
            .header("X-Cache", "HIT")
            .body(axum::body::Body::from(entry.body.clone()))
            .ok()?;

        // Variante Some.
        Some(response)
    }

    /// Store a response in the cache
    pub async fn put(&self, key: String, response: &Response, body: Vec<u8>) {
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/octet-stream")
            .to_string();

        let entry = CacheEntry {
            body,
            status: response.status(),
            content_type,
            created_at: Instant::now(),
        };

        let mut entries = self.entries.write().await;

        // Evict oldest entries if at capacity
        if entries.len() >= self.max_entries {
            let drain_count = self.max_entries / 10;
            let keys_to_remove: Vec<String> = entries
                .iter()
                .take(drain_count)
                .map(|(k, _)| k.clone())
                .collect();
            for k in keys_to_remove {
                entries.remove(&k);
            }
        }

        entries.insert(key, entry);
    }

    /// Get cache statistics
    pub async fn stats(&self) -> CacheStats {
        let entries = self.entries.read().await;
        let total = entries.len();
        let expired = entries
            .values()
            .filter(|e| e.is_expired(self.default_ttl))
            .count();
        // Variante CacheStats.
        CacheStats {
            total_entries: total,
            active_entries: total - expired,
            expired_entries: expired,
            max_entries: self.max_entries,
        }
    }

    /// Clear all cache entries
    pub async fn clear(&self) {
        let mut entries = self.entries.write().await;
        entries.clear();
    }
}

/// Cache statistics
#[derive(Debug, Clone, serde::Serialize)]
/// Estrutura CacheStats.
pub struct CacheStats {
    /// Campo total entries.
    pub total_entries: usize,
    /// Campo active entries.
    pub active_entries: usize,
    /// Campo expired entries.
    pub expired_entries: usize,
    /// Campo max entries.
    pub max_entries: usize,
}

/// Caching middleware — serves cached responses for GET requests
pub async fn caching_middleware(
    axum::extract::State(cache): axum::extract::State<HttpCache>,
    request: Request,
    next: Next,
) -> Response {
    // Only cache eligible requests
    if !HttpCache::is_cacheable(&request) {
        let mut response = next.run(request).await;
        response
            .headers_mut()
            .insert("X-Cache", HeaderValue::from_static("BYPASS"));
        return response;
    }

    let key = HttpCache::cache_key(&request);

    // Try cache first
    if let Some(cached) = cache.get(&key).await {
        return cached;
    }

    // Cache miss — run handler
    let response = next.run(request).await;

    // Only cache successful responses
    if response.status().is_success() {
        // We need to read the body to cache it, then reconstruct
        let (parts, body) = response.into_parts();
        let body_bytes = match axum::body::to_bytes(body, 1024 * 1024).await {
            // Variante Ok.
            Ok(b) => b.to_vec(),
            // Variante Err.
            Err(_) => {
                // Can't read body — return without caching
                return Response::from_parts(parts, axum::body::Body::empty());
            }
        };

        let reconstructed = Response::from_parts(parts, axum::body::Body::from(body_bytes.clone()));

        // Store in cache
        cache.put(key, &reconstructed, body_bytes).await;

        let mut final_response = reconstructed;
        final_response
            .headers_mut()
            .insert("X-Cache", HeaderValue::from_static("MISS"));
        final_response
    } else {
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Uri;

    fn make_get_request(path: &str) -> Request {
        Request::builder()
            .method("GET")
            .uri(Uri::try_from(path).unwrap())
            .body(Body::empty())
            .unwrap()
    }

    fn make_post_request(path: &str) -> Request {
        Request::builder()
            .method("POST")
            .uri(Uri::try_from(path).unwrap())
            .body(Body::empty())
            .unwrap()
    }

    #[test]
    fn test_cache_key_generation() {
        let req = make_get_request("/api/v1/skills?page=1");
        let key = HttpCache::cache_key(&req);
        assert_eq!(key, "GET:/api/v1/skills?page=1");
    }

    #[test]
    fn test_cache_key_no_query() {
        let req = make_get_request("/api/v1/health");
        let key = HttpCache::cache_key(&req);
        assert_eq!(key, "GET:/api/v1/health");
    }

    #[test]
    fn test_is_cacheable_get() {
        let req = make_get_request("/api/v1/skills");
        assert!(HttpCache::is_cacheable(&req));
    }

    #[test]
    fn test_is_not_cacheable_post() {
        let req = make_post_request("/api/v1/skills");
        assert!(!HttpCache::is_cacheable(&req));
    }

    #[test]
    fn test_is_not_cacheable_sse() {
        let req = make_get_request("/events");
        assert!(!HttpCache::is_cacheable(&req));
    }

    #[test]
    fn test_is_not_cacheable_ws() {
        let req = make_get_request("/ws");
        assert!(!HttpCache::is_cacheable(&req));
    }

    #[tokio::test]
    async fn test_cache_put_and_get() {
        let cache = HttpCache::new(60, 100);
        let key = "GET:/test".to_string();

        let response = Response::builder()
            .status(200)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"status":"ok"}"#))
            .unwrap();

        cache
            .put(key.clone(), &response, br#"{"status":"ok"}"#.to_vec())
            .await;

        let cached = cache.get(&key).await;
        assert!(cached.is_some());
    }

    #[tokio::test]
    async fn test_cache_miss_returns_none() {
        let cache = HttpCache::new(60, 100);
        let result = cache.get("nonexistent").await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_cache_stats() {
        let cache = HttpCache::new(60, 100);

        for i in 0..5 {
            let key = format!("GET:/test/{}", i);
            let response = Response::builder()
                .status(200)
                .header(header::CONTENT_TYPE, "text/plain")
                .body(Body::from("ok"))
                .unwrap();
            cache.put(key, &response, b"ok".to_vec()).await;
        }

        let stats = cache.stats().await;
        assert_eq!(stats.total_entries, 5);
        assert_eq!(stats.active_entries, 5);
        assert_eq!(stats.expired_entries, 0);
    }

    #[tokio::test]
    async fn test_cache_clear() {
        let cache = HttpCache::new(60, 100);
        let key = "GET:/test".to_string();
        let response = Response::builder()
            .status(200)
            .header(header::CONTENT_TYPE, "text/plain")
            .body(Body::from("ok"))
            .unwrap();
        cache.put(key, &response, b"ok".to_vec()).await;

        assert_eq!(cache.stats().await.total_entries, 1);
        cache.clear().await;
        assert_eq!(cache.stats().await.total_entries, 0);
    }

    #[tokio::test]
    async fn test_cache_eviction() {
        let cache = HttpCache::new(60, 10);

        // Insert 15 entries into a cache with max 10
        for i in 0..15 {
            let key = format!("GET:/evict/{}", i);
            let response = Response::builder()
                .status(200)
                .header(header::CONTENT_TYPE, "text/plain")
                .body(Body::from("ok"))
                .unwrap();
            cache.put(key, &response, b"ok".to_vec()).await;
        }

        let stats = cache.stats().await;
        // After eviction, should be <= max_entries
        assert!(
            stats.total_entries <= 10,
            "Expected <= 10, got {}",
            stats.total_entries
        );
    }
}
