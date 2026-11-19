//! Approvals API — list and resolve HITL approval requests
//!
//! Endpoints:
//! - `GET  /api/v1/approvals` — list pending approval requests
//! - `POST /api/v1/approvals` — create an approval request (agents / services); publishes
//!   `ApprovalRequested` on the SSE bus and schedules an `ApprovalTimeout` notice
//! - `POST /api/v1/approvals/resolve` — resolve an approval; publishes `ApprovalResolved`
//! - `GET  /api/v1/approvals/rules` — regras "sempre permitir" ativas
//! - `POST /api/v1/approvals/rules/revoke` — revoga uma regra (admin)

use axum::{
    extract::State,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use zylos_approval::{ApprovalDecision, ApprovalRequest, ApprovalStore, CreateApprovalRequest};
use zylos_live_comm::event_types::ApprovalRequested;
use zylos_live_comm::{CustomEvent, EventBus, ZylosEvent};

#[derive(Clone)]
pub struct ApprovalsState {
    store: Arc<ApprovalStore>,
    bus: Option<EventBus>,
}

impl ApprovalsState {
    pub fn new(store: Arc<ApprovalStore>) -> Self {
        Self { store, bus: None }
    }

    /// Liga o barramento de eventos: o cockpit recebe aprovações em tempo real pelo SSE.
    pub fn with_event_bus(mut self, bus: EventBus) -> Self {
        self.bus = Some(bus);
        self
    }

    async fn notify(&self, topic: &str, event: ZylosEvent) {
        if let Some(bus) = &self.bus {
            if let Err(e) = bus.publish(topic, event, 1).await {
                tracing::warn!(error = %e, topic, "failed to publish approval event");
            }
        }
    }

    async fn notify_custom(&self, topic: &str, name: &str, payload: serde_json::Value) {
        self.notify(
            topic,
            ZylosEvent::Custom(CustomEvent {
                name: name.to_string(),
                payload,
            }),
        )
        .await;
    }
}

#[derive(Debug, Deserialize)]
struct CreateBody {
    bot_id: String,
    tool_name: String,
    action_category: String,
    summary: String,
    #[serde(default = "empty_args")]
    arguments_json: String,
    expires_in_seconds: Option<u64>,
}

fn empty_args() -> String {
    "{}".to_string()
}

#[derive(Debug, Serialize)]
struct CreateResponse {
    id: String,
    status: String,
    expires_at: Option<String>,
}

/// `POST /approvals` — cria um pedido de aprovação, avisa o cockpit e agenda o aviso de expiração.
async fn create_approval(
    State(state): State<ApprovalsState>,
    Json(body): Json<CreateBody>,
) -> impl IntoResponse {
    if body.summary.trim().is_empty() || body.bot_id.trim().is_empty() {
        return axum::http::StatusCode::BAD_REQUEST.into_response();
    }
    let id = match state
        .store
        .create(CreateApprovalRequest {
            bot_id: body.bot_id.clone(),
            tool_name: body.tool_name,
            action_category: body.action_category,
            summary: body.summary.clone(),
            arguments_json: body.arguments_json,
            expires_in_seconds: body.expires_in_seconds,
        })
        .await
    {
        Ok(id) => id,
        Err(zylos_approval::ApprovalError::InvalidCategory(_)) => {
            return axum::http::StatusCode::BAD_REQUEST.into_response();
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to create approval");
            return axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let created = state.store.get(&id).await.ok();
    let expires_at = created.as_ref().and_then(|r| r.expires_at.clone());

    state
        .notify(
            "approvals.requested",
            ZylosEvent::ApprovalRequested(ApprovalRequested {
                approval_id: id.clone(),
                requester_id: body.bot_id,
                action_description: body.summary.clone(),
                expires_at: expires_at
                    .as_deref()
                    .and_then(|e| chrono::DateTime::parse_from_rfc3339(e).ok())
                    .map(|d| d.with_timezone(&chrono::Utc)),
            }),
        )
        .await;

    if let Some(secs) = body.expires_in_seconds {
        let (st, aid, summary) = (state.clone(), id.clone(), body.summary);
        tokio::spawn(async move {
            // Margem de 250 ms: `get_pending` só marca expirada depois de `expires_at`.
            tokio::time::sleep(
                std::time::Duration::from_secs(secs) + std::time::Duration::from_millis(250),
            )
            .await;
            // Só avisa se ninguém decidiu: `get_pending` marca como expirada.
            if matches!(
                st.store.get_pending(&aid).await,
                Err(zylos_approval::ApprovalError::Expired(_))
            ) {
                st.notify_custom(
                    "approvals.timeout",
                    "ApprovalTimeout",
                    serde_json::json!({ "approval_id": aid, "summary": summary }),
                )
                .await;
            }
        });
    }

    (
        axum::http::StatusCode::CREATED,
        Json(CreateResponse {
            id,
            status: "pending".to_string(),
            expires_at,
        }),
    )
        .into_response()
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

/// `GET /approvals/rules` — regras permanentes criadas por "Sempre permitir".
async fn list_rules(State(state): State<ApprovalsState>) -> impl IntoResponse {
    match state.store.list_all_rules().await {
        Ok(rules) => Json(serde_json::json!({ "rules": rules })).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to list approval rules");
            axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
struct RevokeBody {
    rule_id: String,
}

/// `POST /approvals/rules/revoke` — remove a regra; o próximo pedido volta a perguntar.
async fn revoke_rule(
    State(state): State<ApprovalsState>,
    Json(body): Json<RevokeBody>,
) -> impl IntoResponse {
    match state.store.delete_rule(&body.rule_id).await {
        Ok(()) => axum::http::StatusCode::NO_CONTENT.into_response(),
        Err(zylos_approval::ApprovalError::NotFound(_)) => {
            axum::http::StatusCode::NOT_FOUND.into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to revoke approval rule");
            axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
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

    match state
        .store
        .decide(&body.approval_id, decision, body.refuse_reason)
        .await
    {
        Ok(decided_at) => {
            let status = match decision {
                ApprovalDecision::Refuse => "refused",
                _ => "approved",
            };
            state
                .notify_custom(
                    "approvals.resolved",
                    "ApprovalResolved",
                    serde_json::json!({ "approval_id": body.approval_id, "status": status }),
                )
                .await;
            Json(
                serde_json::to_value(ResolveResponse {
                    id: body.approval_id,
                    status: status.to_string(),
                    decided_at,
                })
                .unwrap(),
            )
            .into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to resolve approval");
            axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

pub fn create_approvals_router(state: ApprovalsState) -> Router {
    Router::new()
        .route("/approvals", get(list_approvals).post(create_approval))
        // POST /approval/resolve (singular) é do roteador SSE e usa o store em memória e o enum
        // Approved/Denied; o cockpit decide aprovações do store SQLite (allow_once, refuse etc.),
        // por isso a resolução fica aqui, em /approvals/resolve (plural).
        .route("/approvals/resolve", post(resolve_approval))
        .route("/approvals/rules", get(list_rules))
        .route("/approvals/rules/revoke", post(revoke_rule))
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
                Request::post("/approvals/resolve")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    async fn resolve_with(decision: &str) -> StatusCode {
        let (state, store) = setup().await;
        let id = store
            .create(CreateApprovalRequest {
                bot_id: "test-bot".to_string(),
                tool_name: "send_email".to_string(),
                action_category: "send_message".to_string(),
                summary: "Send email".to_string(),
                arguments_json: "{}".to_string(),
                expires_in_seconds: Some(300),
            })
            .await
            .unwrap();
        let body = serde_json::json!({ "approval_id": id, "decision": decision });
        create_approvals_router(state)
            .oneshot(
                Request::post("/approvals/resolve")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap()
            .status()
    }

    #[tokio::test]
    async fn resolve_approval_refuse_and_invalid_decision() {
        assert_eq!(resolve_with("refuse").await, StatusCode::OK);
        assert_eq!(resolve_with("talvez").await, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn regra_sempre_permitir_aparece_na_lista_e_pode_ser_revogada() {
        let (state, _bus) = setup_with_bus().await;
        let id = state
            .store
            .create(CreateApprovalRequest {
                bot_id: "chief".into(),
                tool_name: "send_email".into(),
                action_category: "send_message".into(),
                summary: "Enviar resumo".into(),
                arguments_json: "{}".into(),
                expires_in_seconds: Some(300),
            })
            .await
            .unwrap();
        let body = serde_json::json!({ "approval_id": id, "decision": "allow_always" });
        assert_eq!(
            post_json(state.clone(), "/approvals/resolve", body).await,
            StatusCode::OK
        );

        let regras = state.store.list_all_rules().await.unwrap();
        assert_eq!(regras.len(), 1);
        assert_eq!(regras[0].decision, "allow_always");

        let revogar = serde_json::json!({ "rule_id": regras[0].id });
        assert_eq!(
            post_json(state.clone(), "/approvals/rules/revoke", revogar.clone()).await,
            StatusCode::NO_CONTENT
        );
        assert!(state.store.list_all_rules().await.unwrap().is_empty());
        assert_eq!(
            post_json(state, "/approvals/rules/revoke", revogar).await,
            StatusCode::NOT_FOUND
        );
    }

    // --- criação + eventos SSE ------------------------------------------------

    use futures_util::StreamExt;

    async fn setup_with_bus() -> (ApprovalsState, EventBus) {
        let store = ApprovalStore::connect("sqlite::memory:").await.unwrap();
        store.run_migrations().await.unwrap();
        let bus = EventBus::new(32, None);
        (
            ApprovalsState::new(Arc::new(store)).with_event_bus(bus.clone()),
            bus,
        )
    }

    async fn post_json(state: ApprovalsState, path: &str, body: serde_json::Value) -> StatusCode {
        create_approvals_router(state)
            .oneshot(
                Request::post(path)
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap()
            .status()
    }

    fn new_body(expires: Option<u64>) -> serde_json::Value {
        serde_json::json!({
            "bot_id": "backend",
            "tool_name": "deploy",
            "action_category": "deploy",
            "summary": "Publicar gateway 2.1.0",
            "expires_in_seconds": expires,
        })
    }

    async fn next_envelope<S, E: std::fmt::Debug>(
        rx: &mut S,
    ) -> zylos_live_comm::event_types::EventEnvelope
    where
        S: futures_util::Stream<Item = Result<zylos_live_comm::event_types::EventEnvelope, E>>
            + Unpin,
    {
        tokio::time::timeout(std::time::Duration::from_secs(5), rx.next())
            .await
            .expect("evento em até 5s")
            .expect("stream aberto")
            .expect("sem lag")
    }

    #[tokio::test]
    async fn criar_aprovacao_publica_approval_requested_e_aparece_na_lista() {
        let (state, bus) = setup_with_bus().await;
        let mut rx = bus.subscribe_legacy();

        assert_eq!(
            post_json(state.clone(), "/approvals", new_body(Some(300))).await,
            StatusCode::CREATED
        );

        match next_envelope(&mut rx).await.payload {
            ZylosEvent::ApprovalRequested(r) => {
                assert_eq!(r.action_description, "Publicar gateway 2.1.0");
                assert_eq!(r.requester_id, "backend");
                assert!(r.expires_at.is_some());
            }
            outro => panic!("esperava ApprovalRequested, veio {outro:?}"),
        }
        assert_eq!(state.store.list_pending().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn criar_rejeita_categoria_invalida_e_resumo_vazio() {
        let (state, _) = setup_with_bus().await;
        let mut ruim = new_body(None);
        ruim["action_category"] = "inexistente".into();
        assert_eq!(
            post_json(state.clone(), "/approvals", ruim).await,
            StatusCode::BAD_REQUEST
        );
        let mut vazio = new_body(None);
        vazio["summary"] = "  ".into();
        assert_eq!(
            post_json(state, "/approvals", vazio).await,
            StatusCode::BAD_REQUEST
        );
    }

    #[tokio::test]
    async fn resolver_publica_approval_resolved() {
        let (state, bus) = setup_with_bus().await;
        let id = state
            .store
            .create(CreateApprovalRequest {
                bot_id: "backend".into(),
                tool_name: "deploy".into(),
                action_category: "deploy".into(),
                summary: "Publicar".into(),
                arguments_json: "{}".into(),
                expires_in_seconds: Some(300),
            })
            .await
            .unwrap();
        let mut rx = bus.subscribe_legacy();
        let body = serde_json::json!({ "approval_id": id, "decision": "allow_once" });
        assert_eq!(
            post_json(state, "/approvals/resolve", body).await,
            StatusCode::OK
        );
        match next_envelope(&mut rx).await.payload {
            ZylosEvent::Custom(c) => {
                assert_eq!(c.name, "ApprovalResolved");
                assert_eq!(c.payload["status"], "approved");
            }
            outro => panic!("esperava ApprovalResolved, veio {outro:?}"),
        }
    }

    #[tokio::test]
    async fn aprovacao_sem_decisao_publica_approval_timeout() {
        let (state, bus) = setup_with_bus().await;
        let mut rx = bus.subscribe_legacy();
        assert_eq!(
            post_json(state, "/approvals", new_body(Some(1))).await,
            StatusCode::CREATED
        );
        // 1º evento: ApprovalRequested; 2º (após ~1,25 s): ApprovalTimeout.
        let _requested = next_envelope(&mut rx).await;
        match next_envelope(&mut rx).await.payload {
            ZylosEvent::Custom(c) => {
                assert_eq!(c.name, "ApprovalTimeout");
                assert_eq!(c.payload["summary"], "Publicar gateway 2.1.0");
            }
            outro => panic!("esperava ApprovalTimeout, veio {outro:?}"),
        }
    }
}
