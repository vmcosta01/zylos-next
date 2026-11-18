//! Subagents API — tarefas filhas do swarm vistas como subagentes.
//!
//! - `GET /api/v1/subagents` — toda tarefa com `parent_id` no
//!   [`SwarmOrchestrator`], mais recente primeiro.
//!
//! Campos sem fonte real no swarm (tokens, modelo, passos) só são preenchidos
//! quando existem; senão vão como `null` e o cockpit mostra "—".

use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::get, Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use zylos_swarm_orchestrator::{SwarmOrchestrator, Task, TaskState};

/// Subagente como devolvido pela API.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubagentView {
    /// ID da tarefa filha.
    pub id: String,
    /// Nome da tarefa.
    pub name: String,
    /// Objetivo (descrição da tarefa).
    pub goal: String,
    /// `queued`, `running`, `waiting_approval`, `completed`, `failed`, `cancelled`.
    pub state: String,
    /// Tarefa-mãe.
    pub parent_id: String,
    /// Agente que executa (se atribuída).
    pub assigned_agent: Option<String>,
    /// Passos concluídos — só quando a tarefa tem filhas próprias.
    pub steps_completed: Option<usize>,
    /// Total de passos — só quando a tarefa tem filhas próprias.
    pub steps_total: Option<usize>,
    /// Tokens consumidos (o swarm não registra; `payload.tokens_used` se houver).
    pub tokens_used: Option<u64>,
    /// Modelo (`payload.model` se houver).
    pub model: Option<String>,
    /// Segundos desde `started_at` até `completed_at` (ou agora).
    pub duration_seconds: Option<u64>,
    /// Tentativas já feitas.
    pub retry_count: u32,
    /// Erro registrado na tarefa.
    pub error_message: Option<String>,
    /// Criação (RFC 3339).
    pub created_at: String,
}

/// Resposta de `GET /subagents`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentsResponse {
    /// Subagentes (pode ser vazio).
    pub subagents: Vec<SubagentView>,
}

/// Mapeia o estado da tarefa para o vocabulário do cockpit.
pub fn map_state(s: TaskState) -> &'static str {
    match s {
        TaskState::Pending | TaskState::Ready => "queued",
        TaskState::Running => "running",
        TaskState::Blocked => "waiting_approval",
        TaskState::Done => "completed",
        TaskState::Failed => "failed",
        TaskState::Cancelled => "cancelled",
    }
}

/// Converte uma tarefa filha em [`SubagentView`]; `children` são as filhas dela.
pub fn subagent_view(task: &Task, children: &[&Task], now: DateTime<Utc>) -> SubagentView {
    let (steps_completed, steps_total) = if children.is_empty() {
        (None, None)
    } else {
        let done = children
            .iter()
            .filter(|c| c.state == TaskState::Done)
            .count();
        (Some(done), Some(children.len()))
    };
    let duration_seconds = task.started_at.map(|start| {
        let end = task.completed_at.unwrap_or(now);
        (end - start).num_seconds().max(0) as u64
    });
    SubagentView {
        id: task.id.clone(),
        name: task.name.clone(),
        goal: task.description.clone(),
        state: map_state(task.state).to_string(),
        parent_id: task.parent_id.clone().unwrap_or_default(),
        assigned_agent: task.assigned_agent.clone(),
        steps_completed,
        steps_total,
        tokens_used: task.payload.get("tokens_used").and_then(|v| v.as_u64()),
        model: task
            .payload
            .get("model")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        duration_seconds,
        retry_count: task.retry_count,
        error_message: task.error.clone(),
        created_at: task.created_at.to_rfc3339(),
    }
}

async fn list_subagents(State(swarm): State<Arc<SwarmOrchestrator>>) -> axum::response::Response {
    let tasks = match swarm.list_child_tasks().await {
        Ok(t) => t,
        Err(e) => {
            tracing::error!(error = %e, "falha ao listar tarefas filhas");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "falha ao ler subagentes" })),
            )
                .into_response();
        }
    };
    let now = Utc::now();
    let subagents = tasks
        .iter()
        .map(|t| {
            let kids: Vec<&Task> = tasks
                .iter()
                .filter(|c| c.parent_id.as_deref() == Some(t.id.as_str()))
                .collect();
            subagent_view(t, &kids, now)
        })
        .collect();
    Json(SubagentsResponse { subagents }).into_response()
}

/// Router de `/subagents` (montado dentro de `/api/v1`).
pub fn create_subagents_router(swarm: Arc<SwarmOrchestrator>) -> Router {
    Router::new()
        .route("/subagents", get(list_subagents))
        .with_state(swarm)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    async fn get_json(app: Router) -> serde_json::Value {
        let resp = app
            .oneshot(Request::get("/subagents").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let b = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        serde_json::from_slice(&b).unwrap()
    }

    #[tokio::test]
    async fn vazio_sem_tarefas_filhas() {
        let s = Arc::new(SwarmOrchestrator::open(":memory:").await.unwrap());
        s.add_task(&Task::new("raiz", "sem filhas")).await.unwrap();
        let v = get_json(create_subagents_router(s)).await;
        assert_eq!(v["subagents"], serde_json::json!([]));
    }

    #[tokio::test]
    async fn lista_filhas_com_passos_e_campos_ausentes_nulos() {
        let s = Arc::new(SwarmOrchestrator::open(":memory:").await.unwrap());
        let raiz = Task::new("raiz", "r");
        s.add_task(&raiz).await.unwrap();
        let sub = Task::new("Pesquisa", "levantar docs")
            .with_parent(raiz.id.clone())
            .with_payload(serde_json::json!({"model": "m-x"}));
        s.add_task(&sub).await.unwrap();
        let passo = Task::new("passo", "p").with_parent(sub.id.clone());
        s.add_task(&passo).await.unwrap();

        let v = get_json(create_subagents_router(s)).await;
        let arr = v["subagents"].as_array().unwrap();
        assert_eq!(arr.len(), 2);
        let p = arr.iter().find(|x| x["id"] == sub.id.as_str()).unwrap();
        assert_eq!(p["state"], "queued");
        assert_eq!(p["goal"], "levantar docs");
        assert_eq!(p["steps_total"], 1);
        assert_eq!(p["steps_completed"], 0);
        assert_eq!(p["model"], "m-x");
        assert!(p["tokens_used"].is_null());
        assert!(p["duration_seconds"].is_null());
        let f = arr.iter().find(|x| x["id"] == passo.id.as_str()).unwrap();
        assert!(f["steps_total"].is_null());
    }

    #[test]
    fn mapeia_estados() {
        assert_eq!(map_state(TaskState::Blocked), "waiting_approval");
        assert_eq!(map_state(TaskState::Done), "completed");
        assert_eq!(map_state(TaskState::Ready), "queued");
    }
}
