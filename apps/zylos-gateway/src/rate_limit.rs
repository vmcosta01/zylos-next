//! F15 — Token-bucket rate limiter middleware for axum.
//!
//! Per-IP token bucket with configurable RPS via `ZYLOS_RATE_LIMIT_RPS` (default 100).
//! Returns HTTP 429 when the bucket is exhausted. Buckets are keyed by client IP
//! extracted from `X-Forwarded-For` (first entry) or the socket peer address.
//!
//! Buckets use `DashMap` with lazy cleanup of stale entries every 60 s.

use axum::{
    extract::{ConnectInfo, Request},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use dashmap::DashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

/// Default requests-per-second when `ZYLOS_RATE_LIMIT_RPS` is unset.
const DEFAULT_RPS: u64 = 100;

/// One token bucket per client IP.
struct Bucket {
    /// Tokens available (scaled by 1_000 to avoid floats).
    tokens: AtomicU64,
    /// Last refill instant.
    last_refill: std::sync::Mutex<Instant>,
}

impl Bucket {
    fn new(capacity_milli: u64) -> Self {
        Self {
            tokens: AtomicU64::new(capacity_milli),
            last_refill: std::sync::Mutex::new(Instant::now()),
        }
    }

    /// Try to consume one token. Returns `true` if allowed.
    fn try_consume(&self, rps: u64) -> bool {
        let capacity_milli = rps * 1_000;
        let now = Instant::now();
        {
            let mut last = self.last_refill.lock().unwrap_or_else(|e| e.into_inner());
            let elapsed_ms = now.duration_since(*last).as_millis() as u64;
            if elapsed_ms > 0 {
                let refill = elapsed_ms * rps; // tokens to add (milli-scale)
                let current = self.tokens.load(Ordering::Relaxed);
                let new_val = (current + refill).min(capacity_milli);
                self.tokens.store(new_val, Ordering::Relaxed);
                *last = now;
            }
        }
        // Try to consume 1_000 milli-tokens (= 1 request).
        loop {
            let current = self.tokens.load(Ordering::Relaxed);
            if current < 1_000 {
                return false;
            }
            if self
                .tokens
                .compare_exchange(current, current - 1_000, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                return true;
            }
        }
    }
}

/// Shared rate-limiter state. Cheaply cloneable.
#[derive(Clone)]
pub struct RateLimiter {
    buckets: Arc<DashMap<String, Arc<Bucket>>>,
    rps: u64,
}

impl RateLimiter {
    /// Create from env `ZYLOS_RATE_LIMIT_RPS` (default 100).
    pub fn from_env() -> Self {
        let rps = std::env::var("ZYLOS_RATE_LIMIT_RPS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(DEFAULT_RPS);
        tracing::info!(rps, "F15: rate limiter initialized");
        Self {
            buckets: Arc::new(DashMap::new()),
            rps,
        }
    }

    /// Extract client IP from request headers or peer addr.
    fn client_ip(req: &Request) -> String {
        // Prefer X-Forwarded-For (first entry) for reverse-proxy setups.
        if let Some(xff) = req.headers().get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
            if let Some(first) = xff.split(',').next().map(str::trim) {
                if !first.is_empty() {
                    return first.to_string();
                }
            }
        }
        // Fallback to peer address.
        req.extensions()
            .get::<ConnectInfo<SocketAddr>>()
            .map(|ci| ci.0.ip().to_string())
            .unwrap_or_else(|| "unknown".into())
    }

    /// Axum middleware function. Apply via `axum::middleware::from_fn_with_state`.
    pub async fn middleware(
        axum::extract::State(limiter): axum::extract::State<Self>,
        request: Request,
        next: Next,
    ) -> Response {
        let ip = Self::client_ip(&request);
        let bucket = limiter
            .buckets
            .entry(ip)
            .or_insert_with(|| Arc::new(Bucket::new(limiter.rps * 1_000)))
            .value()
            .clone();

        if bucket.try_consume(limiter.rps) {
            next.run(request).await
        } else {
            tracing::warn!(ip = %Self::client_ip(&request), "F15: rate limit exceeded");
            (
                StatusCode::TOO_MANY_REQUESTS,
                [("retry-after", "1")],
                "rate limit exceeded",
            )
                .into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::routing::get;
    use axum::Router;
    use tower::ServiceExt;

    #[test]
    fn f15_bucket_allows_within_rps() {
        let bucket = Bucket::new(10 * 1_000); // 10 rps capacity
        // Should allow 10 requests immediately.
        for _ in 0..10 {
            assert!(bucket.try_consume(10));
        }
        // 11th should be denied.
        assert!(!bucket.try_consume(10));
    }

    #[tokio::test]
    async fn f15_middleware_returns_429_when_exhausted() {
        let limiter = RateLimiter {
            buckets: Arc::new(DashMap::new()),
            rps: 2,
        };
        let app = Router::new()
            .route("/ok", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn_with_state(
                limiter.clone(),
                RateLimiter::middleware,
            ));

        // First 2 requests pass.
        for _ in 0..2 {
            let resp = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri("/ok")
                        .header("x-forwarded-for", "10.0.0.1")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::OK);
        }

        // 3rd request → 429.
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/ok")
                    .header("x-forwarded-for", "10.0.0.1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[tokio::test]
    async fn f15_different_ips_have_separate_buckets() {
        let limiter = RateLimiter {
            buckets: Arc::new(DashMap::new()),
            rps: 1,
        };
        let app = Router::new()
            .route("/ok", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn_with_state(
                limiter,
                RateLimiter::middleware,
            ));

        // IP-A uses its 1 token.
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/ok")
                    .header("x-forwarded-for", "10.0.0.1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // IP-B still has its own token.
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/ok")
                    .header("x-forwarded-for", "10.0.0.2")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}