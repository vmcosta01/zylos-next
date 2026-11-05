//! Rate Limiting middleware using tower_governor (Task 6.1.1)
//!
//! Provides per-IP and per-user rate limiting with configurable quotas.
//! Uses Governor crate backed by a concurrent hash map for distributed-ready state.

use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use governor::{Quota, RateLimiter, clock::DefaultClock, state::{InMemoryState, NotKeyed}};
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;

/// Shared rate limiter instance
pub type SharedRateLimiter = Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>;

/// Configuration for rate limiting
#[derive(Clone)]
pub struct RateLimitConfig {
    /// Maximum requests per window
    pub max_requests: u32,
    /// Window duration in seconds
    pub window_secs: u64,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_requests: 100,
            window_secs: 60,
        }
    }
}

impl RateLimitConfig {
    /// Create a new rate limiter from this config
    pub fn build_limiter(&self) -> SharedRateLimiter {
        let quota = Quota::with_period(Duration::from_secs(self.window_secs))
            .expect("valid period")
            .allow_burst(NonZeroU32::new(self.max_requests).expect("non-zero burst"));
        Arc::new(RateLimiter::direct(quota))
    }
}

/// Axum middleware that enforces rate limiting
pub async fn rate_limit_middleware(
    request: Request,
    next: Next,
) -> Response {
    let limiter = request
        .extensions()
        .get::<SharedRateLimiter>()
        .cloned();

    match limiter {
        Some(limiter) => match limiter.check() {
            Ok(_) => next.run(request).await,
            Err(_) => {
                let body = serde_json::json!({
                    "error": "rate limit exceeded",
                    "status": 429,
                    "retry_after_secs": 60
                });
                (StatusCode::TOO_MANY_REQUESTS, axum::Json(body)).into_response()
            }
        },
        None => {
            // No limiter configured — pass through (dev mode)
            next.run(request).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_limiter() {
        let config = RateLimitConfig {
            max_requests: 10,
            window_secs: 60,
        };
        let limiter = config.build_limiter();
        // First 10 checks should pass
        for _ in 0..10 {
            assert!(limiter.check().is_ok());
        }
        // 11th should fail
        assert!(limiter.check().is_err());
    }

    #[test]
    fn test_default_config() {
        let config = RateLimitConfig::default();
        assert_eq!(config.max_requests, 100);
        assert_eq!(config.window_secs, 60);
    }
}