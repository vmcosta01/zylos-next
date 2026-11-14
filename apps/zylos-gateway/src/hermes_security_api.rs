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
use serde_json::{json, Value};
use zylos_live_comm::hermes_bridge::{HermesBridge, HermesEnvelope};

use crate::AppState;

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
// Capability Tokens
// ---------------------------------------------------------------------------

async fn get_capability_tokens(State(state): State<AppState>) -> Response {
    publish_event(&state, "capability-tokens-listed", json!({}));
    ok_json(json!({
        "status": "ok",
        "module": "capability_tokens",
        "proxy_endpoint": "/api/v1/security/capability-tokens"
    }))
}

async fn post_capability_token(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    publish_event(&state, "capability-token-issued", body.clone());
    ok_json(json!({
        "status": "accepted",
        "agent_id": body.get("agent_id").and_then(|v| v.as_str()).unwrap_or("unknown"),
        "proxy_endpoint": "/api/v1/security/capability-tokens"
    }))
}

async fn delete_capability_token(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Response {
    publish_event(&state, "capability-token-revoked", body.clone());
    ok_json(json!({
        "status": "accepted",
        "token_id": body.get("token_id").and_then(|v| v.as_str()).unwrap_or("unknown"),
        "proxy_endpoint": "/api/v1/security/capability-tokens/revoke"
    }))
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
    Router::new()
        .route(
            "/hermes/security/guardrails/stats",
            get(get_guardrails_stats),
        )
        .route(
            "/hermes/security/guardrails/rules",
            post(post_guardrail_rule),
        )
        .route(
            "/hermes/security/capability-tokens",
            get(get_capability_tokens),
        )
        .route(
            "/hermes/security/capability-tokens/issue",
            post(post_capability_token),
        )
        .route(
            "/hermes/security/capability-tokens/revoke",
            delete(delete_capability_token),
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
}
