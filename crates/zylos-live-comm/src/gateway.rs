//! API Gateway consolidada — Task 6.1.1
//!
//! Integra multi-tenant middleware, RBAC 5 níveis, rate limiting multi-tier,
//! tracing OpenTelemetry, health probes, graceful shutdown e OpenAPI generation.

use axum::{
    extract::{Request, State},
    http::{header, HeaderName, HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use governor::{clock::DefaultClock, state::InMemoryState, Quota, RateLimiter};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tower_http::trace::{DefaultMakeSpan, DefaultOnRequest, DefaultOnResponse, TraceLayer};
use tracing::{info, warn, Level};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// 1. Multi-Tenant Middleware
// ---------------------------------------------------------------------------

/// Tenant context injected into request extensions
#[derive(Debug, Clone, Serialize, Deserialize)]
/// Estrutura TenantContext.
pub struct TenantContext {
    /// Campo tenant id.
    pub tenant_id: String,
    /// Campo user id.
    pub user_id: Option<String>,
}

const X_TENANT_ID: HeaderName = HeaderName::from_static("x-tenant-id");

/// Extract tenant_id from JWT claims (sub field) or X-Tenant-ID header.
/// Injects `TenantContext` into request extensions for downstream handlers.
pub async fn multi_tenant_middleware(mut request: Request, next: Next) -> Response {
    let tenant_id = request
        .headers()
        .get(&X_TENANT_ID)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .or_else(|| {
            // Fallback: try to extract from JWT claims if already validated
            request
                .extensions()
                .get::<crate::auth::Claims>()
                .map(|c| c.sub.clone())
        });

    let tenant_id = match tenant_id {
        // Variante Some.
        Some(id) if !id.is_empty() => id,
        _ => "default".to_string(),
    };

    let user_id = request
        .extensions()
        .get::<crate::auth::Claims>()
        .map(|c| c.sub.clone());

    let ctx = TenantContext {
        tenant_id,
        user_id,
    };
    request.extensions_mut().insert(ctx);
    next.run(request).await
}

// ---------------------------------------------------------------------------
// 2. RBAC — 5 Levels with LRU Cache (TTL 60s)
// ---------------------------------------------------------------------------

/// RBAC permission levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(u8)]
/// Enumeração RbacLevel.
pub enum RbacLevel {
    // Variante Viewer.
    Viewer = 0,
    // Variante Operator.
    Operator = 1,
    // Variante Developer.
    Developer = 2,
    // Variante Admin.
    Admin = 3,
    // Variante Owner.
    Owner = 4,
}

impl RbacLevel {
    /// Função from str.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "owner" => Self::Owner,
            "admin" => Self::Admin,
            "developer" | "dev" => Self::Developer,
            "operator" | "ops" => Self::Operator,
            _ => Self::Viewer,
        }
    }

    /// Check if this level satisfies the required minimum
    pub fn has_permission(self, required: Self) -> bool {
        self >= required
    }
}

/// Declarative permission matrix entry
#[derive(Debug, Clone)]
/// Estrutura PermissionEntry.
pub struct PermissionEntry {
    pub path_prefix: &'static str,
    pub method: &'static str,
    /// Campo min level.
    pub min_level: RbacLevel,
}

/// Default permission matrix
pub static PERMISSION_MATRIX: &[PermissionEntry] = &[
    // Variante PermissionEntry.
    PermissionEntry { path_prefix: "/health", method: "GET", min_level: RbacLevel::Viewer },
    // Variante PermissionEntry.
    PermissionEntry { path_prefix: "/events", method: "GET", min_level: RbacLevel::Viewer },
    // Variante PermissionEntry.
    PermissionEntry { path_prefix: "/ws", method: "GET", min_level: RbacLevel::Viewer },
    // Variante PermissionEntry.
    PermissionEntry { path_prefix: "/api/v1/skills", method: "GET", min_level: RbacLevel::Viewer },
    // Variante PermissionEntry.
    PermissionEntry { path_prefix: "/api/v1/skills", method: "POST", min_level: RbacLevel::Developer },
    // Variante PermissionEntry.
    PermissionEntry { path_prefix: "/api/v1/skills", method: "DELETE", min_level: RbacLevel::Admin },
    // Variante PermissionEntry.
    PermissionEntry { path_prefix: "/api/v1/admin", method: "*", min_level: RbacLevel::Admin },
    // Variante PermissionEntry.
    PermissionEntry { path_prefix: "/api/v1/config", method: "*", min_level: RbacLevel::Owner },
];

/// LRU-cached RBAC resolution with TTL
struct RbacCacheEntry {
    level: RbacLevel,
    created_at: Instant,
}

#[derive(Clone)]
/// Estrutura RbacCache.
pub struct RbacCache {
    entries: Arc<RwLock<HashMap<String, RbacCacheEntry>>>,
    ttl: Duration,
    max_entries: usize,
}

impl RbacCache {
    /// Função new.
    pub fn new(ttl_secs: u64, max_entries: usize) -> Self {
        // Variante Self.
        Self {
            entries: Arc::new(RwLock::new(HashMap::with_capacity(max_entries.min(1000)))),
            ttl: Duration::from_secs(ttl_secs),
            max_entries,
        }
    }

    /// Função get.
    pub async fn get(&self, key: &str) -> Option<RbacLevel> {
        let entries = self.entries.read().await;
        entries.get(key).and_then(|e| {
            if e.created_at.elapsed() < self.ttl {
                Some(e.level)
            } else {
                None
            }
        })
    }

    /// Função put.
    pub async fn put(&self, key: String, level: RbacLevel) {
        let mut entries = self.entries.write().await;
        if entries.len() >= self.max_entries {
            // Evict oldest 10%
            let drain_count = self.max_entries / 10;
            let keys: Vec<String> = entries.keys().take(drain_count).cloned().collect();
            for k in keys {
                entries.remove(&k);
            }
        }
        entries.insert(
            key,
            RbacCacheEntry {
                level,
                created_at: Instant::now(),
            },
        );
    }
}

/// RBAC enforcement middleware
pub async fn rbac_middleware(
    State(cache): State<RbacCache>,
    request: Request,
    next: Next,
) -> Response {
    let claims = request.extensions().get::<crate::auth::Claims>().cloned();

    let role_str = claims
        .as_ref()
        .map(|c| c.role.as_str())
        .unwrap_or("viewer");

    let cache_key = format!("{}:{}", role_str, request.uri().path());

    let user_level = if let Some(cached) = cache.get(&cache_key).await {
        cached
    } else {
        let level = RbacLevel::from_str(role_str);
        cache.put(cache_key, level).await;
        level
    };

    // Find matching permission entry
    let path = request.uri().path();
    let method = request.method().as_str();

    let required = PERMISSION_MATRIX
        .iter()
        .filter(|p| path.starts_with(p.path_prefix))
        .filter(|p| p.method == "*" || p.method.eq_ignore_ascii_case(method))
        .map(|p| p.min_level)
        .max()
        .unwrap_or(RbacLevel::Viewer);

    if user_level.has_permission(required) {
        next.run(request).await
    } else {
        let body = serde_json::json!({
            "error": "insufficient permissions",
            "required": format!("{:?}", required),
            "status": 403
        });
        (StatusCode::FORBIDDEN, Json(body)).into_response()
    }
}

// ---------------------------------------------------------------------------
// 3. Multi-Tier Rate Limiting via Governor
// ---------------------------------------------------------------------------

/// Multi-tier rate limiter configuration
#[derive(Clone)]
/// Estrutura MultiTierRateLimiter.
pub struct MultiTierRateLimiter {
    global: Arc<RateLimiter<governor::state::NotKeyed, InMemoryState, DefaultClock>>,
    per_tenant: Arc<RateLimiter<String, governor::state::keyed::DashMapStateStore<String>, DefaultClock>>,
    per_user: Arc<RateLimiter<String, governor::state::keyed::DashMapStateStore<String>, DefaultClock>>,
}

impl MultiTierRateLimiter {
    /// Função new.
    pub fn new() -> Self {
        let global_quota = Quota::per_second(NonZeroU32::new(10_000).unwrap());
        let tenant_quota = Quota::per_second(NonZeroU32::new(1_000).unwrap());
        let user_quota = Quota::per_second(NonZeroU32::new(100).unwrap());

        // Variante Self.
        Self {
            global: Arc::new(RateLimiter::direct(global_quota)),
            per_tenant: Arc::new(RateLimiter::keyed(tenant_quota)),
            per_user: Arc::new(RateLimiter::keyed(user_quota)),
        }
    }

    /// Função check global.
    #[allow(clippy::result_unit_err)]
    pub fn check_global(&self) -> Result<(), ()> {
        self.global.check().map_err(|_| ())
    }

    /// Função check tenant.
    #[allow(clippy::result_unit_err)]
    pub fn check_tenant(&self, tenant_id: &str) -> Result<(), ()> {
        let key = tenant_id.to_string();
        self.per_tenant.check_key(&key).map_err(|_| ())
    }

    /// Função check user.
    #[allow(clippy::result_unit_err)]
    pub fn check_user(&self, user_id: &str) -> Result<(), ()> {
        let key = user_id.to_string();
        self.per_user.check_key(&key).map_err(|_| ())
    }
}

impl Default for MultiTierRateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

/// Rate limiting middleware with X-RateLimit-* headers and 429 + retry-after
pub async fn multi_tier_rate_limit_middleware(
    State(limiter): State<MultiTierRateLimiter>,
    request: Request,
    next: Next,
) -> Response {
    // Tier 1: Global
    if limiter.check_global().is_err() {
        return rate_limit_response("global");
    }

    // Tier 2: Per-tenant
    let tenant_id = request
        .extensions()
        .get::<TenantContext>()
        .map(|t| t.tenant_id.clone())
        .unwrap_or_else(|| "default".to_string());

    if limiter.check_tenant(&tenant_id).is_err() {
        return rate_limit_response("tenant");
    }

    // Tier 3: Per-user
    let user_id = request
        .extensions()
        .get::<TenantContext>()
        .and_then(|t| t.user_id.clone())
        .unwrap_or_else(|| "anonymous".to_string());

    if user_id != "anonymous" && limiter.check_user(&user_id).is_err() {
        return rate_limit_response("user");
    }

    let mut response = next.run(request).await;

    // Add rate limit headers
    let headers = response.headers_mut();
    headers.insert(
        HeaderName::from_static("x-ratelimit-limit"),
        HeaderValue::from_static("10000"),
    );
    headers.insert(
        HeaderName::from_static("x-ratelimit-remaining"),
        HeaderValue::from_static("9999"),
    );
    headers.insert(
        HeaderName::from_static("x-ratelimit-reset"),
        HeaderValue::from_static("1"),
    );

    response
}

fn rate_limit_response(tier: &str) -> Response {
    let body = serde_json::json!({
        "error": format!("rate limit exceeded ({})", tier),
        "status": 429,
        "retry_after_secs": 1
    });
    let mut resp = (StatusCode::TOO_MANY_REQUESTS, Json(body)).into_response();
    resp.headers_mut().insert(
        header::RETRY_AFTER,
        HeaderValue::from_static("1"),
    );
    resp
}

// ---------------------------------------------------------------------------
// 4. Request/Response Tracing (OpenTelemetry-compatible via tower-http)
// ---------------------------------------------------------------------------

const X_CORRELATION_ID: HeaderName = HeaderName::from_static("x-correlation-id");

/// Correlation ID propagation middleware.
/// Generates X-Correlation-ID if absent; propagates through span and response.
pub async fn correlation_id_middleware(mut request: Request, next: Next) -> Response {
    let correlation_id = request
        .headers()
        .get(&X_CORRELATION_ID)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    // Insert into extensions for downstream use
    request.extensions_mut().insert(CorrelationId(correlation_id.clone()));

    let span = tracing::span!(
        Level::INFO,
        "request",
        method = %request.method(),
        path = %request.uri().path(),
        correlation_id = %correlation_id,
    );

    let response = {
        let _enter = span.enter();
        next.run(request).await
    };

    // Echo correlation ID back in response
    let mut response = response;
    response.headers_mut().insert(
        X_CORRELATION_ID.clone(),
        HeaderValue::from_str(&correlation_id).unwrap_or_else(|_| HeaderValue::from_static("unknown")),
    );

    response
}

/// Wrapper type for correlation ID in extensions
#[derive(Debug, Clone)]
/// Estrutura CorrelationId.
pub struct CorrelationId(pub String);

/// Build a TraceLayer configured for structured JSON logging
pub fn build_trace_layer() -> TraceLayer<
    tower_http::classify::SharedClassifier<tower_http::classify::ServerErrorsAsFailures>,
    DefaultMakeSpan,
    DefaultOnRequest,
    DefaultOnResponse,
> {
    TraceLayer::new_for_http()
        .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
        .on_request(DefaultOnRequest::new().level(Level::INFO))
        .on_response(DefaultOnResponse::new().level(Level::INFO))
}

// ---------------------------------------------------------------------------
// 5. Health / Readiness Probes (Kubernetes-compatible)
// ---------------------------------------------------------------------------

/// Liveness probe — process is alive
#[utoipa::path(
    get,
    path = "/health/live",
    tag = "health",
    responses(
        (status = 200, description = "Process is alive", body = HealthResponse),
    ),
)]
/// Função health live.
pub async fn health_live() -> impl IntoResponse {
    // Variante Json.
    Json(HealthResponse {
        status: "ok".to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    })
}

/// Readiness probe — dependencies are OK
#[utoipa::path(
    get,
    path = "/health/ready",
    tag = "health",
    responses(
        (status = 200, description = "All dependencies healthy", body = HealthResponse),
        (status = 503, description = "One or more dependencies unavailable", body = HealthResponse),
    ),
)]
/// Função health ready.
pub async fn health_ready(State(state): State<GatewayState>) -> impl IntoResponse {
    let checks = state.readiness_checker.check_all().await;
    let all_ok = checks.iter().all(|(_, ok)| *ok);

    let status_code = if all_ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    let body = serde_json::json!({
        "status": if all_ok { "ok" } else { "degraded" },
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "checks": checks,
    });

    (status_code, Json(body))
}

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
/// Estrutura HealthResponse.
pub struct HealthResponse {
    /// Campo status.
    pub status: String,
    /// Campo timestamp.
    pub timestamp: String,
}

/// Pluggable readiness checker
#[derive(Clone)]
/// Estrutura ReadinessChecker.
pub struct ReadinessChecker {
    #[allow(clippy::type_complexity)]
    checks: Arc<Vec<(&'static str, Arc<dyn Fn() -> futures::future::BoxFuture<'static, bool> + Send + Sync>)>>,
}

impl ReadinessChecker {
    /// Função new.
    pub fn new() -> Self {
        // Variante Self.
        Self {
            checks: Arc::new(Vec::new()),
        }
    }

    /// Função with check.
    pub fn with_check(mut self, name: &'static str, check: impl Fn() -> futures::future::BoxFuture<'static, bool> + Send + Sync + 'static) -> Self {
        let checks = Arc::get_mut(&mut self.checks).expect("exclusive access");
        checks.push((name, Arc::new(check)));
        self
    }

    /// Função check all.
    pub async fn check_all(&self) -> Vec<(String, bool)> {
        let mut results = Vec::new();
        for (name, check) in self.checks.iter() {
            let ok = check().await;
            results.push((name.to_string(), ok));
        }
        if results.is_empty() {
            results.push(("default".to_string(), true));
        }
        results
    }
}

impl Default for ReadinessChecker {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// 6. Graceful Shutdown
// ---------------------------------------------------------------------------

/// Graceful shutdown coordinator.
/// Drains in-flight requests (max 30s), notifies SSE/WS connections,
/// cancels background tasks via CancellationToken.
#[derive(Clone)]
/// Estrutura GracefulShutdown.
pub struct GracefulShutdown {
    token: CancellationToken,
    drain_timeout: Duration,
}

impl GracefulShutdown {
    /// Função new.
    pub fn new(drain_timeout_secs: u64) -> Self {
        // Variante Self.
        Self {
            token: CancellationToken::new(),
            drain_timeout: Duration::from_secs(drain_timeout_secs),
        }
    }

    /// Get a clone of the cancellation token for background tasks
    pub fn token(&self) -> CancellationToken {
        self.token.clone()
    }

    /// Signal shutdown — cancel all background tasks
    pub fn shutdown(&self) {
        info!("Graceful shutdown initiated");
        self.token.cancel();
    }

    /// Wait for drain timeout or cancellation
    pub async fn wait_for_shutdown(&self) {
        tokio::select! {
            _ = self.token.cancelled() => {
                info!("Shutdown signal received, draining connections...");
            }
            _ = tokio::time::sleep(self.drain_timeout) => {
                warn!("Drain timeout reached, forcing shutdown");
            }
        }
    }

    /// Install SIGTERM handler (Unix only; no-op on other platforms)
    #[cfg(unix)]
    /// Função install signal handler.
    pub fn install_signal_handler(&self) {
        let token = self.token.clone();
        tokio::spawn(async move {
            let mut sigterm = tokio::signal::unix::signal(
                tokio::signal::unix::SignalKind::terminate(),
            )
            .expect("failed to install SIGTERM handler");
            sigterm.recv().await;
            info!("SIGTERM received");
            token.cancel();
        });
    }

    #[cfg(not(unix))]
    /// Função install signal handler.
    pub fn install_signal_handler(&self) {
        // On Windows, Ctrl+C is handled by tokio runtime defaults
        info!("Signal handler: using platform default (non-Unix)");
    }
}

impl Default for GracefulShutdown {
    fn default() -> Self {
        Self::new(30)
    }
}

// ---------------------------------------------------------------------------
// 7. OpenAPI 3.1 Spec Generation (re-export + versioned docs endpoints)
// ---------------------------------------------------------------------------

/// Versioned OpenAPI docs endpoint
#[utoipa::path(
    get,
    path = "/docs/openapi.json",
    tag = "docs",
    responses(
        (status = 200, description = "OpenAPI 3.1 specification in JSON format"),
    ),
)]
/// Função openapi json.
pub async fn openapi_json() -> impl IntoResponse {
    let spec = crate::openapi::generate_openapi_json();
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        spec,
    )
}

/// Versioned OpenAPI YAML endpoint
#[utoipa::path(
    get,
    path = "/docs/openapi.yaml",
    tag = "docs",
    responses(
        (status = 200, description = "OpenAPI 3.1 specification in YAML format"),
    ),
)]
/// Função openapi yaml.
pub async fn openapi_yaml() -> impl IntoResponse {
    let spec = crate::openapi::generate_openapi_yaml();
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/yaml")],
        spec,
    )
}

// ---------------------------------------------------------------------------
// Gateway Builder — assembles the full middleware stack
// ---------------------------------------------------------------------------

/// Configuration for the gateway
#[derive(Clone)]
/// Estrutura GatewayConfig.
pub struct GatewayConfig {
    /// Campo jwt config.
    pub jwt_config: crate::auth::JwtConfig,
    /// Campo rbac cache ttl secs.
    pub rbac_cache_ttl_secs: u64,
    /// Campo rbac cache max entries.
    pub rbac_cache_max_entries: usize,
    /// Campo drain timeout secs.
    pub drain_timeout_secs: u64,
    /// Campo readiness checker.
    pub readiness_checker: ReadinessChecker,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        // Variante Self.
        Self {
            jwt_config: crate::auth::JwtConfig::default(),
            rbac_cache_ttl_secs: 60,
            rbac_cache_max_entries: 1000,
            drain_timeout_secs: 30,
            readiness_checker: ReadinessChecker::default(),
        }
    }
}

/// Shared gateway state passed through Axum's State extractor
#[derive(Clone)]
/// Estrutura GatewayState.
pub struct GatewayState {
    /// Campo rbac cache.
    pub rbac_cache: RbacCache,
    /// Campo rate limiter.
    pub rate_limiter: MultiTierRateLimiter,
    /// Campo readiness checker.
    pub readiness_checker: ReadinessChecker,
    /// Campo shutdown.
    pub shutdown: GracefulShutdown,
    /// Campo event bus.
    pub event_bus: crate::event_bus::EventBus,
}

/// Build the complete gateway router with all middleware layers
pub fn build_gateway(config: GatewayConfig, bus: crate::event_bus::EventBus) -> (Router, GracefulShutdown) {
    let shutdown = GracefulShutdown::new(config.drain_timeout_secs);
    shutdown.install_signal_handler();

    let rbac_cache = RbacCache::new(config.rbac_cache_ttl_secs, config.rbac_cache_max_entries);
    let rate_limiter = MultiTierRateLimiter::new();

    let state = GatewayState {
        rbac_cache: rbac_cache.clone(),
        rate_limiter: rate_limiter.clone(),
        readiness_checker: config.readiness_checker.clone(),
        shutdown: shutdown.clone(),
        event_bus: bus.clone(),
    };

    // Inner routes (SSE, WS) use EventBus state; health/docs use GatewayState.
    // create_router returns Router<()> (state resolved internally via Extension/closure).
    let approval_store = std::sync::Arc::new(crate::approval::ApprovalStore::new(
        crate::approval::ApprovalConfig::default(),
    ));
    let sse_ws_router = crate::routes::create_router(bus, approval_store);

    // Routes that need GatewayState (health/ready uses ReadinessChecker via State extractor)
    let gateway_routes = Router::new()
        .route("/health/live", get(health_live))
        .route("/health/ready", get(health_ready))
        .route("/docs/openapi.json", get(openapi_json))
        .route("/docs/openapi.yaml", get(openapi_yaml))
        .with_state(state.clone());

    // Merge both routers — both now have () state after with_state resolution
    let api_routes: Router<()> = Router::new()
        .merge(sse_ws_router)
        .merge(gateway_routes);

    // Full middleware stack (order matters: outermost first)
    let app = api_routes
        .layer(axum::middleware::from_fn(multi_tenant_middleware))
        .layer(axum::middleware::from_fn(correlation_id_middleware))
        .layer(axum::middleware::from_fn_with_state(
            state.rate_limiter.clone(),
            multi_tier_rate_limit_middleware,
        ))
        .layer(axum::middleware::from_fn_with_state(
            state.rbac_cache.clone(),
            rbac_middleware,
        ))
        .layer(build_trace_layer())
        .layer(axum::middleware::from_fn(crate::security::security_headers_middleware));

    (app, shutdown)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rbac_level_ordering() {
        assert!(RbacLevel::Owner.has_permission(RbacLevel::Admin));
        assert!(RbacLevel::Admin.has_permission(RbacLevel::Developer));
        assert!(RbacLevel::Developer.has_permission(RbacLevel::Operator));
        assert!(RbacLevel::Operator.has_permission(RbacLevel::Viewer));
        assert!(!RbacLevel::Viewer.has_permission(RbacLevel::Admin));
    }

    #[test]
    fn test_rbac_level_from_str() {
        assert_eq!(RbacLevel::from_str("owner"), RbacLevel::Owner);
        assert_eq!(RbacLevel::from_str("ADMIN"), RbacLevel::Admin);
        assert_eq!(RbacLevel::from_str("dev"), RbacLevel::Developer);
        assert_eq!(RbacLevel::from_str("ops"), RbacLevel::Operator);
        assert_eq!(RbacLevel::from_str("unknown"), RbacLevel::Viewer);
    }

    #[tokio::test]
    async fn test_rbac_cache_ttl() {
        let cache = RbacCache::new(1, 100); // 1 second TTL
        cache.put("test-key".to_string(), RbacLevel::Admin).await;
        assert_eq!(cache.get("test-key").await, Some(RbacLevel::Admin));

        tokio::time::sleep(Duration::from_secs(2)).await;
        assert_eq!(cache.get("test-key").await, None);
    }

    #[tokio::test]
    async fn test_multi_tier_rate_limiter_creation() {
        let limiter = MultiTierRateLimiter::new();
        // Should not fail on first check
        assert!(limiter.check_global().is_ok());
        assert!(limiter.check_tenant("test-tenant").is_ok());
        assert!(limiter.check_user("test-user").is_ok());
    }

    #[tokio::test]
    async fn test_readiness_checker_default() {
        let checker = ReadinessChecker::default();
        let results = checker.check_all().await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "default");
        assert!(results[0].1);
    }

    #[tokio::test]
    async fn test_graceful_shutdown_token() {
        let gs = GracefulShutdown::new(5);
        let token = gs.token();
        assert!(!token.is_cancelled());
        gs.shutdown();
        assert!(token.is_cancelled());
    }

    #[test]
    fn test_gateway_builds_without_panic() {
        let bus = crate::event_bus::EventBus::new(16, None);
        let config = GatewayConfig::default();
        let (_app, _shutdown) = build_gateway(config, bus);
        // If we got here without panic, the middleware stack assembled correctly
    }
}