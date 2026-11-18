//! Hermes Desktop API — endpoints dedicados para módulos desktop da Fase 8.
//!
//! Rotas (montadas sob `/api/v1`):
//! - `GET /hermes/desktop/workspace/state` — estado atual do cockpit
//! - `POST /hermes/desktop/workspace/save` — salvar estado explicitamente
//! - `POST /hermes/desktop/workspace/reset` — resetar para defaults
//! - `GET /hermes/desktop/awareness/states` — estados de todos os agentes
//! - `GET /hermes/desktop/awareness/attention` — agentes requerendo atenção
//! - `POST /hermes/desktop/awareness/update` — atualizar estado de um agente
//!
//! As rotas `/hermes/desktop/relay/*` (Phone Relay) vivem em [`crate::relay_api`].
//!
//! Todos os endpoints publicam eventos no EventBus para visibilidade no cockpit.

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
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
        source: "hermes:desktop".into(),
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
// Workspace Persistence
// ---------------------------------------------------------------------------

async fn get_workspace_state(State(state): State<AppState>) -> Response {
    publish_event(&state, "workspace-state-requested", json!({}));
    ok_json(json!({
        "status": "ok",
        "module": "workspace_persistence",
        "proxy_endpoint": "/api/v1/desktop/workspace/state"
    }))
}

async fn post_workspace_save(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    publish_event(&state, "workspace-state-saved", body.clone());
    ok_json(json!({
        "status": "accepted",
        "tabs_count": body.get("tabs").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0),
        "proxy_endpoint": "/api/v1/desktop/workspace/save"
    }))
}

async fn post_workspace_reset(State(state): State<AppState>) -> Response {
    publish_event(&state, "workspace-state-reset", json!({}));
    ok_json(json!({
        "status": "accepted",
        "proxy_endpoint": "/api/v1/desktop/workspace/reset"
    }))
}

// ---------------------------------------------------------------------------
// Agent Awareness
// ---------------------------------------------------------------------------

async fn get_agent_awareness_states(State(state): State<AppState>) -> Response {
    publish_event(&state, "awareness-states-requested", json!({}));
    ok_json(json!({
        "status": "ok",
        "module": "agent_awareness",
        "states": ["idle", "thinking", "executing", "waiting_approval", "error", "completed"],
        "proxy_endpoint": "/api/v1/desktop/agents/states"
    }))
}

async fn get_agents_requiring_attention(State(state): State<AppState>) -> Response {
    publish_event(&state, "awareness-attention-requested", json!({}));
    ok_json(json!({
        "status": "ok",
        "module": "agent_awareness",
        "filter": "requires_attention",
        "proxy_endpoint": "/api/v1/desktop/agents/attention"
    }))
}

async fn post_agent_state_update(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Response {
    publish_event(&state, "awareness-state-updated", body.clone());
    ok_json(json!({
        "status": "accepted",
        "agent_id": body.get("agent_id").and_then(|v| v.as_str()).unwrap_or("unknown"),
        "new_state": body.get("new_state").and_then(|v| v.as_str()).unwrap_or("unknown"),
        "proxy_endpoint": "/api/v1/desktop/agents/update"
    }))
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn create_hermes_desktop_router() -> Router<AppState> {
    Router::new()
        // Workspace Persistence
        .route("/hermes/desktop/workspace/state", get(get_workspace_state))
        .route("/hermes/desktop/workspace/save", post(post_workspace_save))
        .route(
            "/hermes/desktop/workspace/reset",
            post(post_workspace_reset),
        )
        // Agent Awareness
        .route(
            "/hermes/desktop/awareness/states",
            get(get_agent_awareness_states),
        )
        .route(
            "/hermes/desktop/awareness/attention",
            get(get_agents_requiring_attention),
        )
        .route(
            "/hermes/desktop/awareness/update",
            post(post_agent_state_update),
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
