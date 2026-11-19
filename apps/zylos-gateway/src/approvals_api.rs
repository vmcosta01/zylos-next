//! Approvals API — list and resolve HITL approval requests
//!
//! Endpoints:
//! - `GET  /api/v1/approvals` — list pending approval requests
//! - `POST /api/v1/approval/resolve` — resolve an approval (existing, wired to store)

use axum::{
 extract::State,
 response::IntoResponse,
 routing::{get, post},
 Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use zylos_approval::{ApprovalDecision, ApprovalRequest, ApprovalStore};

#[derive(Clone)]
pub struct ApprovalsState {
    store: Arc<ApprovalStore>,
}

impl ApprovalsState {
    pub fn new(store: Arc<ApprovalStore>) -> Self {
        Self { store }
    }
}

#[derive(Debug, Serialize)]
struct ApprovalResponse {
    id: String,
    bot_id: String,
    tool_name: String,
    action_category: String,
    summary: String,
    arguments_json: String,
    status: String,
    decision: Option<String>,
    refuse_reason: Option<String>,
    created_at: String,
    decided_at: Option<String>,
    expires_at: Option<String>,
}

impl From<ApprovalRequest> for ApprovalResponse {
    fn from(req: ApprovalRequest) -> Self {
        Self {
            id: req.id,
            bot_id: req.bot_id,
            tool_name: req.tool_name,
            action_category: req.action_category,
            summary: req.summary,
            arguments_json: req.arguments_json,
            status: req.status,
            decision: req.decision,
            refuse_reason: req.refuse_reason,
            created_at: req.created_at,
            decided_at: req.decided_at,
            expires_at: req.expires_at,
        }
    }
}

#[derive(Debug, Deserialize)]
struct ResolveRequest {
    approval_id: String,
    decision: String,
    refuse_reason: Option<String>,
}

#[derive(Debug, Serialize)]
struct ResolveResponse {
    id: String,
    status: String,
    decided_at: String,
}

async fn list_approvals(State(state): State<ApprovalsState>) -> impl IntoResponse {
    match state.store.list_pending().await {
        Ok(approvals) => {
            let response: Vec<ApprovalResponse> = approvals.into_iter().map(Into::into).collect();
            Json(serde_json::json!({ "approvals": response }))
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to list pending approvals");
            Json(serde_json::json!({ "error": "failed to list approvals" }))
        }
    }
}

async fn resolve_approval(
    State(state): State<ApprovalsState>,
    Json(body): Json<ResolveRequest>,
) -> impl IntoResponse {
    let decision = match ApprovalDecision::parse(&body.decision) {
        Some(d) => d,
        None => {
            return axum::http::StatusCode::BAD_REQUEST.into_response();
        }
    };

    match state.store.decide(&body.approval_id, decision, body.refuse_reason).await {
        Ok(decided_at) => Json(serde_json::to_value(ResolveResponse {
            id: body.approval_id,
            status: match decision {
                ApprovalDecision::Refuse => "refused",
                _ => "approved",
            }
            .to_string(),
            decided_at,
        })
        .unwrap())
        .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to resolve approval");
            axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

pub fn create_approvals_router(state: ApprovalsState) -> Router {
    Router::new()
        .route("/approvals", get(list_approvals))
        .route("/approval/resolve", post(resolve_approval))
        .with_state(state)
}

#[cfg(test)]
mod tests {
 use super::*;
 use axum::body::Body;
 use axum::http::{Request, StatusCode};
 use tower::ServiceExt;
 use zylos_approval::{ApprovalStore, CreateApprovalRequest};

    async fn setup() -> (ApprovalsState, ApprovalStore) {
        let store = ApprovalStore::connect("sqlite::memory:").await.unwrap();
        store.run_migrations().await.unwrap();
        (ApprovalsState::new(Arc::new(store.clone())), store)
    }

    #[tokio::test]
    async fn list_approvals_empty() {
        let (state, _) = setup().await;
        let app = create_approvals_router(state);

        let response = app
            .oneshot(Request::get("/approvals").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn list_approvals_with_pending() {
        let (state, store) = setup().await;

        let req = CreateApprovalRequest {
            bot_id: "test-bot".to_string(),
            tool_name: "send_email".to_string(),
            action_category: "send_message".to_string(),
            summary: "Send email to user@example.com".to_string(),
            arguments_json: r#"{"to":"user@example.com"}"#.to_string(),
            expires_in_seconds: Some(300),
        };

        store.create(req).await.unwrap();

        let app = create_approvals_router(state);

        let response = app
            .oneshot(Request::get("/approvals").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn resolve_approval_success() {
        let (state, store) = setup().await;

        let req = CreateApprovalRequest {
            bot_id: "test-bot".to_string(),
            tool_name: "send_email".to_string(),
            action_category: "send_message".to_string(),
            summary: "Send email".to_string(),
            arguments_json: "{}".to_string(),
            expires_in_seconds: Some(300),
        };

        let id = store.create(req).await.unwrap();

        let app = create_approvals_router(state);

        let body = serde_json::json!({
            "approval_id": id,
            "decision": "allow_once"
        });

        let response = app
            .oneshot(
                Request::post("/approval/resolve")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}