//! Hermes Skill Evolution API — endpoints dedicados para contratos, evolução, mineração e hierarquia de skills (Fase 8).
//!
//! Rotas (montadas sob `/api/v1`):
//! - `GET /hermes/skill-evolution/contracts` — listar contratos ativos
//! - `POST /hermes/skill-evolution/contracts/validate` — validar contrato formal (P,O,A,V,F)
//! - `GET /hermes/skill-evolution/graph` — grafo de evolução (Extends/Replaces/Repairs/DependsOn)
//! - `POST /hermes/skill-evolution/miner/run` — executar mineração de skills a partir de logs
//! - `POST /hermes/skill-evolution/trajectory/compile` — compilar HANDOFF/playbook → SkillContract
//! - `GET /hermes/skill-evolution/hierarchy` — listar skills por tier (Planning/Functional/Atomic)
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
        source: "hermes:skill-evolution".into(),
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
// Skill Contracts
// ---------------------------------------------------------------------------

async fn get_skill_contracts(State(state): State<AppState>) -> Response {
    publish_event(&state, "contracts-listed", json!({}));
    ok_json(json!({
        "status": "ok",
        "module": "skill_contracts",
        "proxy_endpoint": "/api/v1/skills/contracts"
    }))
}

async fn post_skill_contract_validate(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Response {
    publish_event(&state, "contract-validated", body.clone());
    ok_json(json!({
        "status": "accepted",
        "skill_id": body.get("skill_id").and_then(|v| v.as_str()).unwrap_or("unknown"),
        "proxy_endpoint": "/api/v1/skills/contracts/validate"
    }))
}

// ---------------------------------------------------------------------------
// Evolution Graph
// ---------------------------------------------------------------------------

async fn get_evolution_graph(State(state): State<AppState>) -> Response {
    publish_event(&state, "evolution-graph-requested", json!({}));
    ok_json(json!({
        "status": "ok",
        "module": "skill_evolution_graph",
        "edge_types": ["Extends", "Replaces", "Repairs", "DependsOn"],
        "proxy_endpoint": "/api/v1/skills/evolution/graph"
    }))
}

// ---------------------------------------------------------------------------
// Skill Miner
// ---------------------------------------------------------------------------

async fn post_skill_miner_run(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    publish_event(&state, "miner-executed", body.clone());
    ok_json(json!({
        "status": "accepted",
        "log_source": body.get("log_source").and_then(|v| v.as_str()).unwrap_or("gateway"),
        "proxy_endpoint": "/api/v1/skills/miner/run"
    }))
}

// ---------------------------------------------------------------------------
// Trajectory Compiler
// ---------------------------------------------------------------------------

async fn post_trajectory_compile(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Response {
    publish_event(&state, "trajectory-compiled", body.clone());
    ok_json(json!({
        "status": "accepted",
        "source_type": body.get("source").and_then(|v| v.as_str()).unwrap_or("handoff"),
        "proxy_endpoint": "/api/v1/skills/trajectory/compile"
    }))
}

// ---------------------------------------------------------------------------
// Skill Hierarchy
// ---------------------------------------------------------------------------

async fn get_skill_hierarchy(State(state): State<AppState>) -> Response {
    publish_event(&state, "hierarchy-requested", json!({}));
    ok_json(json!({
        "status": "ok",
        "module": "skill_hierarchy",
        "tiers": ["planning", "functional", "atomic"],
        "proxy_endpoint": "/api/v1/skills/hierarchy"
    }))
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn create_hermes_skill_evolution_router() -> Router<AppState> {
    Router::new()
        .route(
            "/hermes/skill-evolution/contracts",
            get(get_skill_contracts),
        )
        .route(
            "/hermes/skill-evolution/contracts/validate",
            post(post_skill_contract_validate),
        )
        .route("/hermes/skill-evolution/graph", get(get_evolution_graph))
        .route(
            "/hermes/skill-evolution/miner/run",
            post(post_skill_miner_run),
        )
        .route(
            "/hermes/skill-evolution/trajectory/compile",
            post(post_trajectory_compile),
        )
        .route(
            "/hermes/skill-evolution/hierarchy",
            get(get_skill_hierarchy),
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
