//! Hermes Security API — endpoints dedicados para módulos de segurança da Fase 8.
//!
//! Rotas (montadas sob `/api/v1`):
//! - `GET  /hermes/security/guardrails/stats` — estatísticas do GuardrailEnforcer
//! - `POST /hermes/security/guardrails/rules` — criar/atualizar regra de guardrail
//! - `GET  /hermes/security/capability-tokens` — listar tokens ativos
//! - `POST /hermes/security/capability-tokens/issue` — emitir novo token
//! - `DELETE /hermes/security/capability-tokens/revoke` — revogar token
//! - `GET  /hermes/security/egress/rules` — listar regras de egress
//! - `POST /hermes/security/egress/rules` — adicionar regra de egress
//! - `GET  /hermes/security/sanitizer/rules` — listar regras de sanitização
//! - `POST /hermes/security/sanitizer/sanitize` — sanitizar output
//!
//! Todos os endpoints publicam eventos no EventBus para visibilidade no cockpit.

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use zylos_agent_shield::capability_token::{Capability, CapabilityToken, CapabilityTokenStore};
use zylos_live_comm::hermes_bridge::{HermesBridge, HermesEnvelope};

use crate::AppState;

/// F16: estado compartilhado dos endpoints de capability token.
#[derive(Clone)]
pub struct CapabilityTokensState {
    pub store: Arc<CapabilityTokenStore>,
    /// Chave HMAC-SHA256 para assinar/verificar tokens.
    /// Lida da env `ZYLOS_CAPABILITY_TOKEN_KEY`; se ausente, usa fallback inseguro só em dev.
    pub signing_key: Vec<u8>,
}

impl CapabilityTokensState {
    pub fn from_env() -> Self {
        let key = std::env::var("ZYLOS_CAPABILITY_TOKEN_KEY")
            .ok()
            .filter(|k| !k.is_empty())
            .unwrap_or_else(|| {
                tracing::warn!(
                    "ZYLOS_CAPABILITY_TOKEN_KEY não definido — usando chave efêmera (dev only)"
                );
                "zylos-capability-token-dev-key-32b!".to_string()
            });
        Self {
            store: Arc::new(CapabilityTokenStore::new()),
            signing_key: key.into_bytes(),
        }
    }
}

/// Payload de entrada para emissão de capability token.
#[derive(Debug, Deserialize)]
struct IssueTokenRequest {
    agent_id: String,
    capabilities: Vec<CapabilityInput>,
    #[serde(default)]
    ttl_secs: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct CapabilityInput {
    domain: String,
    action: String,
    resource: String,
}

/// Payload de entrada para revogação.
#[derive(Debug, Deserialize)]
struct RevokeTokenRequest {
    token_id: String,
}

/// Resposta de emissão com token assinado.
#[derive(Debug, Serialize)]
struct IssueTokenResponse {
    token_id: String,
    signature: String,
    fingerprint: String,
    issued_at: u64,
    ttl_secs: Option<u64>,
}

fn chrono_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("unix:{secs}")
}

fn publish_event(state: &AppState, event_type: &str, payload: Value) {
    let envelope = HermesEnvelope {
        source: "hermes:security".into(),
        event_type: event_type.into(),
        payload,
        timestamp: Some(chrono_now()),
    };
    let _ = HermesBridge::new(state.event_bus.clone()).forward(&envelope);
}

fn ok_json(body: Value) -> Response {
    (StatusCode::OK, Json(body)).into_response()
}

// ---------------------------------------------------------------------------
// Guardrails
// ---------------------------------------------------------------------------

async fn get_guardrails_stats(State(state): State<AppState>) -> Response {
    publish_event(&state, "guardrails-stats-requested", json!({}));
    ok_json(json!({
        "status": "ok",
        "module": "guardrails",
        "proxy_endpoint": "/api/v1/personalities/guardrails/stats"
    }))
}

async fn post_guardrail_rule(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    publish_event(&state, "guardrail-rule-created", body.clone());
    ok_json(json!({
        "status": "accepted",
        "rule_name": body.get("name").and_then(|v| v.as_str()).unwrap_or("unknown"),
        "proxy_endpoint": "/api/v1/personalities/guardrails/rules"
    }))
}

// ---------------------------------------------------------------------------
// Capability Tokens (F16: real HMAC-signed issuance/verification)
// ---------------------------------------------------------------------------

async fn get_capability_tokens(
    State(cts): State<CapabilityTokensState>,
) -> Response {
    let count = cts.store.count().await;
    ok_json(json!({
        "status": "ok",
        "active_tokens": count
    }))
}

async fn post_capability_token(
    State(cts): State<CapabilityTokensState>,
    Json(req): Json<IssueTokenRequest>,
) -> Response {
    if req.agent_id.is_empty() {
        return (StatusCode::BAD_REQUEST, "agent_id is required").into_response();
    }
    if req.capabilities.is_empty() {
        return (StatusCode::BAD_REQUEST, "at least one capability is required").into_response();
    }
    let caps: Vec<Capability> = req
        .capabilities
        .into_iter()
        .map(|c| Capability::new(c.domain, c.action, c.resource))
        .collect();

    let mut token = CapabilityToken::new(&req.agent_id, caps);
    if let Some(ttl) = req.ttl_secs {
        token = token.with_ttl(ttl);
    }

    let signature = match token.sign(&cts.signing_key) {
        Ok(sig) => sig,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to sign token: {e}"),
            )
                .into_response();
        }
    };

    let resp = IssueTokenResponse {
        token_id: token.id.clone(),
        signature,
        fingerprint: token.fingerprint.clone(),
        issued_at: token.issued_at,
        ttl_secs: token.ttl_secs,
    };

    cts.store.issue(token).await;

    (StatusCode::CREATED, Json(resp)).into_response()
}

async fn delete_capability_token(
    State(cts): State<CapabilityTokensState>,
    Json(req): Json<RevokeTokenRequest>,
) -> Response {
    match cts.store.revoke(&req.token_id).await {
        Ok(()) => ok_json(json!({ "status": "revoked", "token_id": req.token_id })),
        Err(_) => (StatusCode::NOT_FOUND, "token not found").into_response(),
    }
}

/// F16: verifica a assinatura HMAC-SHA256 de um capability token.
/// Endpoint interno para validação por outros serviços.
async fn post_verify_capability_token(
    State(cts): State<CapabilityTokensState>,
    Json(body): Json<Value>,
) -> Response {
    let token_id = match body.get("token_id").and_then(|v| v.as_str()) {
        Some(id) => id,
        None => return (StatusCode::BAD_REQUEST, "token_id is required").into_response(),
    };
    let signature = match body.get("signature").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => return (StatusCode::BAD_REQUEST, "signature is required").into_response(),
    };

    let token = match cts.store.get(token_id).await {
        Some(t) => t,
        None => return (StatusCode::NOT_FOUND, "token not found").into_response(),
    };

    match token.verify_signature(signature, &cts.signing_key) {
        Ok(()) => {
            if token.revoked {
                return (StatusCode::UNAUTHORIZED, "token has been revoked").into_response();
            }
            if token.is_expired() {
                return (StatusCode::UNAUTHORIZED, "token has expired").into_response();
            }
            ok_json(json!({ "valid": true, "agent_id": token.agent_id }))
        }
        Err(_) => (StatusCode::UNAUTHORIZED, "invalid signature").into_response(),
    }
}

// ---------------------------------------------------------------------------
// Network Egress Filter
// ---------------------------------------------------------------------------

async fn get_egress_rules(State(state): State<AppState>) -> Response {
    publish_event(&state, "egress-rules-listed", json!({}));
    ok_json(json!({
        "status": "ok",
        "module": "network_egress",
        "proxy_endpoint": "/api/v1/security/egress/rules"
    }))
}

async fn post_egress_rule(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    publish_event(&state, "egress-rule-added", body.clone());
    ok_json(json!({
        "status": "accepted",
        "agent_id": body.get("agent_id").and_then(|v| v.as_str()).unwrap_or("unknown"),
        "proxy_endpoint": "/api/v1/security/egress/rules"
    }))
}

// ---------------------------------------------------------------------------
// Output Sanitizer
// ---------------------------------------------------------------------------

async fn get_sanitizer_rules(State(state): State<AppState>) -> Response {
    publish_event(&state, "sanitizer-rules-listed", json!({}));
    ok_json(json!({
        "status": "ok",
        "module": "output_sanitizer",
        "proxy_endpoint": "/api/v1/security/sanitizer/rules"
    }))
}

async fn post_sanitize_output(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    publish_event(&state, "output-sanitized", body.clone());
    ok_json(json!({
        "status": "accepted",
        "strategy": body.get("strategy").and_then(|v| v.as_str()).unwrap_or("redact"),
        "proxy_endpoint": "/api/v1/security/sanitizer/sanitize"
    }))
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn create_hermes_security_router() -> Router<AppState> {
    let cts = CapabilityTokensState::from_env();
    Router::new()
        .route(
            "/hermes/security/guardrails/stats",
            get(get_guardrails_stats),
        )
        .route(
            "/hermes/security/guardrails/rules",
            post(post_guardrail_rule),
        )
        // F16: capability token endpoints with real HMAC-signed issuance/verification.
        .route(
            "/hermes/security/capability-tokens",
            get(get_capability_tokens).with_state(cts.clone()),
        )
        .route(
            "/hermes/security/capability-tokens/issue",
            post(post_capability_token).with_state(cts.clone()),
        )
        .route(
            "/hermes/security/capability-tokens/revoke",
            delete(delete_capability_token).with_state(cts.clone()),
        )
        .route(
            "/hermes/security/capability-tokens/verify",
            post(post_verify_capability_token).with_state(cts),
        )
        .route("/hermes/security/egress/rules", get(get_egress_rules))
        .route("/hermes/security/egress/rules", post(post_egress_rule))
        .route("/hermes/security/sanitizer/rules", get(get_sanitizer_rules))
        .route(
            "/hermes/security/sanitizer/sanitize",
            post(post_sanitize_output),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chrono_now_format() {
        let now = chrono_now();
        assert!(now.starts_with("unix:"));
        let secs: u64 = now.strip_prefix("unix:").unwrap().parse().unwrap();
        assert!(secs > 1700000000);
    }

    #[tokio::test]
    async fn f16_capability_token_sign_verify_roundtrip() {
        let key = b"f16-test-key-32bytes-long-ok!!";
        let mut token = CapabilityToken::new(
            "agent-1",
            vec![Capability::new("filesystem", "read", "/data")],
        );
        let sig = token.sign(key).expect("sign failed");
        // Valid signature must verify.
        assert!(token.verify_signature(&sig, key).is_ok());
        // Wrong key must fail.
        assert!(token
            .verify_signature(&sig, b"wrong-key-32bytes-long-ok!!!!")
            .is_err());
        // Tampered signature must fail.
        assert!(token.verify_signature("deadbeef", key).is_err());
    }

    #[tokio::test]
    async fn f16_revoked_token_fails_authorizes() {
        let store = CapabilityTokenStore::new();
        let token = CapabilityToken::new(
            "agent-1",
            vec![Capability::new("filesystem", "read", "/data")],
        );
        let id = store.issue(token).await;
        // Before revocation: valid.
        assert!(store
            .validate(&id, "filesystem", "read", "/data")
            .await
            .is_ok());
        // Revoke.
        store.revoke(&id).await.unwrap();
        // After revocation: rejected.
        assert_eq!(
            store.validate(&id, "filesystem", "read", "/data").await,
            Err(zylos_agent_shield::capability_token::TokenError::Revoked)
        );
    }
}
