//! Agents API — agentes reais registrados no [`SwarmOrchestrator`].
//!
//! - `GET  /api/v1/agents` — lista os agentes do swarm com contagem de tarefas
//!   e último sinal de atividade (heartbeat ou atualização de tarefa).
//! - `POST /api/v1/agents` — registra (ou atualiza) um agente no swarm.
//!
//! Sem agentes registrados a lista vem vazia; nada é inventado.

use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::get, Json, Router};
use serde::{Deserialize, Serialize};
use zylos_swarm_orchestrator::{AgentTaskSummary, SwarmAgent, SwarmOrchestrator};

/// Agente como devolvido pela API.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentView {
    /// ID do agente no swarm.
    pub id: String,
    /// Nome legível.
    pub name: String,
    /// `running` (tem tarefa rodando ou carga), `idle` ou `archived` (inativo).
    pub status: String,
    /// Capacidades declaradas.
    pub capabilities: Vec<String>,
    /// Máximo de tarefas simultâneas.
    pub max_concurrent_tasks: usize,
    /// Carga atual registrada no swarm.
    pub current_load: usize,
    /// Total de tarefas atribuídas ao agente.
    pub task_count: usize,
    /// Tarefas em `running` agora.
    pub running_tasks: usize,
    /// Último heartbeat (RFC 3339).
    pub last_heartbeat: String,
    /// Mais recente entre heartbeat e atualização de tarefa (RFC 3339).
    pub last_active: String,
}

/// Resposta de `GET /agents`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentsResponse {
    /// Agentes registrados (pode ser vazio).
    pub agents: Vec<AgentView>,
}

/// Corpo de `POST /agents`.
#[derive(Debug, Clone, Deserialize)]
pub struct RegisterAgentRequest {
    /// ID opcional; se ausente é gerado (UUID v4).
    #[serde(default)]
    pub id: Option<String>,
    /// Nome legível (obrigatório).
    pub name: String,
    /// Capacidades do agente.
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// Máximo de tarefas simultâneas (padrão 1).
    #[serde(default)]
    pub max_concurrent_tasks: Option<usize>,
}

/// Monta a visão de um agente a partir do registro e do resumo de tarefas.
pub fn agent_view(agent: &SwarmAgent, summary: Option<&AgentTaskSummary>) -> AgentView {
    let running = summary.map(|s| s.running_tasks).unwrap_or(0);
    let status = if !agent.active {
        "archived"
    } else if running > 0 || agent.current_load > 0 {
        "running"
    } else {
        "idle"
    };
    let last_active = match summary.and_then(|s| s.last_task_update) {
        Some(t) if t > agent.last_heartbeat => t,
        _ => agent.last_heartbeat,
    };
    AgentView {
        id: agent.id.clone(),
        name: agent.name.clone(),
        status: status.to_string(),
        capabilities: agent.capabilities.clone(),
        max_concurrent_tasks: agent.max_concurrent_tasks,
        current_load: agent.current_load,
        task_count: summary.map(|s| s.total_tasks).unwrap_or(0),
        running_tasks: running,
        last_heartbeat: agent.last_heartbeat.to_rfc3339(),
        last_active: last_active.to_rfc3339(),
    }
}

fn erro(status: StatusCode, msg: impl Into<String>) -> axum::response::Response {
    (status, Json(serde_json::json!({ "error": msg.into() }))).into_response()
}

async fn list_agents(State(swarm): State<Arc<SwarmOrchestrator>>) -> axum::response::Response {
    let agents = match swarm.list_agents().await {
        Ok(a) => a,
        Err(e) => {
            tracing::error!(error = %e, "falha ao listar agentes do swarm");
            return erro(StatusCode::INTERNAL_SERVER_ERROR, "falha ao ler agentes");
        }
    };
    let sums = match swarm.agent_task_summaries().await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, "falha ao contar tarefas por agente");
            return erro(StatusCode::INTERNAL_SERVER_ERROR, "falha ao ler tarefas");
        }
    };
    let agents = agents
        .iter()
        .map(|a| agent_view(a, sums.get(&a.id)))
        .collect();
    Json(AgentsResponse { agents }).into_response()
}

async fn register_agent(
    State(swarm): State<Arc<SwarmOrchestrator>>,
    Json(req): Json<RegisterAgentRequest>,
) -> axum::response::Response {
    let name = req.name.trim();
    if name.is_empty() || name.chars().count() > 200 {
        return erro(
            StatusCode::BAD_REQUEST,
            "nome obrigatório (até 200 caracteres)",
        );
    }
    let id = match req.id.as_deref().map(str::trim) {
        Some("") => return erro(StatusCode::BAD_REQUEST, "id vazio"),
        Some(id) if id.chars().count() > 128 => {
            return erro(StatusCode::BAD_REQUEST, "id longo demais (até 128)")
        }
        Some(id) => id.to_string(),
        None => uuid::Uuid::new_v4().to_string(),
    };
    let mut agent = SwarmAgent::new(id, name);
    agent.capabilities = req.capabilities;
    agent.max_concurrent_tasks = req.max_concurrent_tasks.unwrap_or(1).max(1);
    if let Err(e) = swarm.register_agent(&agent).await {
        tracing::error!(error = %e, "falha ao registrar agente");
        return erro(
            StatusCode::INTERNAL_SERVER_ERROR,
            "falha ao registrar agente",
        );
    }
    (StatusCode::CREATED, Json(agent_view(&agent, None))).into_response()
}

/// Router de `/agents` (montado dentro de `/api/v1`).
pub fn create_agents_router(swarm: Arc<SwarmOrchestrator>) -> Router {
    Router::new()
        .route("/agents", get(list_agents).post(register_agent))
        .with_state(swarm)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;
    use zylos_swarm_orchestrator::Task;

    async fn swarm() -> Arc<SwarmOrchestrator> {
        Arc::new(SwarmOrchestrator::open(":memory:").await.unwrap())
    }

    async fn body_json(resp: axum::response::Response) -> serde_json::Value {
        let b = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        serde_json::from_slice(&b).unwrap()
    }

    #[tokio::test]
    async fn lista_vazia_sem_agentes() {
        let app = create_agents_router(swarm().await);
        let resp = app
            .oneshot(Request::get("/agents").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(body_json(resp).await["agents"], serde_json::json!([]));
    }

    #[tokio::test]
    async fn registra_e_lista_com_contagem_de_tarefas() {
        let s = swarm().await;
        let app = create_agents_router(s.clone());
        let resp = app
            .clone()
            .oneshot(
                Request::post("/agents")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"id":"rev","name":"Revisor","max_concurrent_tasks":3}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        assert_eq!(body_json(resp).await["status"], "idle");

        s.add_task(&Task::new("t", "d")).await.unwrap();
        s.initialize_ready_tasks().await.unwrap();
        s.schedule().await.unwrap();

        let resp = app
            .oneshot(Request::get("/agents").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let v = body_json(resp).await;
        let a = &v["agents"][0];
        assert_eq!(a["id"], "rev");
        assert_eq!(a["task_count"], 1);
        assert_eq!(a["running_tasks"], 1);
        assert_eq!(a["status"], "running");
    }

    #[tokio::test]
    async fn rejeita_nome_vazio() {
        let app = create_agents_router(swarm().await);
        let resp = app
            .oneshot(
                Request::post("/agents")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"name":"  "}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
}
