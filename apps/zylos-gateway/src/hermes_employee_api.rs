//! Hermes Employee API — endpoints dedicados para governança de AI Employees da Fase 8.
//!
//! Rotas (montadas sob `/api/v1`):
//! - `GET /hermes/employee/roles` — listar roles padronizados (8 variantes)
//! - `GET /hermes/employee/routines` — catálogo de rotinas por role
//! - `POST /hermes/employee/plan-gate/submit` — submeter plano para aprovação humana
//! - `GET /hermes/employee/rate-limiter/stats` — estatísticas do rate limiter por employee/role
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
        source: "hermes:employee".into(),
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
// Employee Roles
// ---------------------------------------------------------------------------

async fn get_employee_roles(State(state): State<AppState>) -> Response {
    publish_event(&state, "roles-listed", json!({}));
    ok_json(json!({
        "status": "ok",
        "module": "employee_roles",
        "roles": [
            "developer", "designer", "devops", "qa_engineer",
            "product_manager", "support_agent", "content_creator", "security_auditor"
        ],
        "proxy_endpoint": "/api/v1/personalities/roles"
    }))
}

// ---------------------------------------------------------------------------
// Routine Catalog
// ---------------------------------------------------------------------------

async fn get_routine_catalog(State(state): State<AppState>) -> Response {
    publish_event(&state, "routine-catalog-listed", json!({}));
    ok_json(json!({
        "status": "ok",
        "module": "routine_catalog",
        "proxy_endpoint": "/api/v1/personalities/routines"
    }))
}

// ---------------------------------------------------------------------------
// Plan-First Gate
// ---------------------------------------------------------------------------

async fn post_plan_first_gate_submit(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Response {
    publish_event(&state, "plan-submitted", body.clone());
    ok_json(json!({
        "status": "accepted",
        "employee_id": body.get("employee_id").and_then(|v| v.as_str()).unwrap_or("unknown"),
        "routine_name": body.get("routine_name").and_then(|v| v.as_str()).unwrap_or("unknown"),
        "proxy_endpoint": "/api/v1/personalities/plan-gate/submit"
    }))
}

// ---------------------------------------------------------------------------
// Rate Limiter
// ---------------------------------------------------------------------------

async fn get_rate_limiter_stats(State(state): State<AppState>) -> Response {
    publish_event(&state, "rate-limiter-stats-requested", json!({}));
    ok_json(json!({
        "status": "ok",
        "module": "employee_rate_limiter",
        "proxy_endpoint": "/api/v1/personalities/rate-limiter/stats"
    }))
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn create_hermes_employee_router() -> Router<AppState> {
    Router::new()
        .route("/hermes/employee/roles", get(get_employee_roles))
        .route("/hermes/employee/routines", get(get_routine_catalog))
        .route(
            "/hermes/employee/plan-gate/submit",
            post(post_plan_first_gate_submit),
        )
        .route(
            "/hermes/employee/rate-limiter/stats",
            get(get_rate_limiter_stats),
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
