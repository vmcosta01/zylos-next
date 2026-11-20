//! Falhas de agentes: guarda o que o cockpit só via como toast e deixa a Triagem listar.
//!
//! Um gravador assina o barramento de eventos e guarda todo `AgentStateChanged` com
//! `new_state = "error"` (hoje, a falha do bot por chave, provedor ou rede). O motivo é o título
//! curto que o emissor já publica, nunca o detalhe (pode citar chave ou URL).
//!
//! ## Endpoints (sob `/api/v1`)
//! - `GET /failures?limit=N`  as falhas mais recentes primeiro (padrão 50, no máximo 200)

use std::str::FromStr;

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqliteConnectOptions, Row, SqlitePool};
use zylos_live_comm::{EventBus, ZylosEvent};

/// Quantas falhas ficam guardadas; as mais antigas saem.
const KEEP: i64 = 500;
const DEFAULT_LIMIT: i64 = 50;
const MAX_LIMIT: i64 = 200;
/// Tamanho máximo do motivo guardado.
const MAX_TITLE: usize = 200;

#[derive(Clone)]
pub struct FailuresStore {
    pool: SqlitePool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Failure {
    pub id: i64,
    pub agent_id: String,
    pub title: String,
    pub created_at: String,
}

impl FailuresStore {
    /// Conecta (cria o arquivo se faltar) e migra.
    pub async fn connect(path: &str) -> Result<Self, sqlx::Error> {
        let opts = SqliteConnectOptions::from_str(path)?.create_if_missing(true);
        let pool = SqlitePool::connect_with(opts).await?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS failures (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                agent_id TEXT NOT NULL,
                title TEXT NOT NULL,
                created_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await?;
        Ok(Self { pool })
    }

    /// Guarda uma falha e apaga as que passam do limite guardado.
    pub async fn record(&self, agent_id: &str, title: &str) -> Result<(), sqlx::Error> {
        let title: String = title.trim().chars().take(MAX_TITLE).collect();
        let title = if title.is_empty() {
            "Falha do agente".to_string()
        } else {
            title
        };
        sqlx::query("INSERT INTO failures (agent_id, title, created_at) VALUES (?, ?, ?)")
            .bind(agent_id.chars().take(64).collect::<String>())
            .bind(title)
            .bind(chrono::Utc::now().to_rfc3339())
            .execute(&self.pool)
            .await?;
        sqlx::query(
            "DELETE FROM failures WHERE id NOT IN (SELECT id FROM failures ORDER BY id DESC LIMIT ?)",
        )
        .bind(KEEP)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// As falhas mais recentes primeiro.
    pub async fn list(&self, limit: i64) -> Result<Vec<Failure>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT id, agent_id, title, created_at FROM failures ORDER BY id DESC LIMIT ?",
        )
        .bind(limit.clamp(1, MAX_LIMIT))
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| Failure {
                id: r.get("id"),
                agent_id: r.get("agent_id"),
                title: r.get("title"),
                created_at: r.get("created_at"),
            })
            .collect())
    }

    /// Assina o barramento e guarda cada falha de agente. Termina quando o barramento fecha.
    pub async fn spawn_recorder(
        &self,
        bus: &EventBus,
    ) -> Result<(), zylos_live_comm::LiveCommError> {
        let mut rx = bus
            .subscribe("failures-recorder", "agent.state", None)
            .await?;
        let store = self.clone();
        tokio::spawn(async move {
            use tokio::sync::broadcast::error::RecvError;
            loop {
                match rx.recv().await {
                    Ok(env) => {
                        if let ZylosEvent::AgentStateChanged(ev) = env.payload {
                            if ev.new_state == "error" {
                                let title = ev.reason.unwrap_or_default();
                                if let Err(e) = store.record(&ev.agent_id, &title).await {
                                    tracing::warn!(error = %e, "falha ao guardar a falha do agente");
                                }
                            }
                        }
                    }
                    Err(RecvError::Lagged(_)) => continue,
                    Err(RecvError::Closed) => break,
                }
            }
        });
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct ListQuery {
    limit: Option<i64>,
}

async fn list_failures(State(store): State<FailuresStore>, Query(q): Query<ListQuery>) -> Response {
    match store.list(q.limit.unwrap_or(DEFAULT_LIMIT)).await {
        Ok(failures) => Json(serde_json::json!({ "failures": failures })).into_response(),
        Err(e) => {
            tracing::warn!(error = %e, "falha ao ler as falhas");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "não foi possível ler as falhas" })),
            )
                .into_response()
        }
    }
}

/// Roteador com estado próprio (vira `Router<()>` para o `.merge`).
pub fn create_failures_router(store: FailuresStore) -> Router {
    Router::new()
        .route("/failures", get(list_failures))
        .with_state(store)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Arquivo temporário: com `:memory:` cada conexão do pool teria o seu próprio banco.
    async fn store() -> (tempfile::TempDir, FailuresStore) {
        let d = tempfile::tempdir().unwrap();
        let path = format!("sqlite:{}", d.path().join("failures.db").display());
        let s = FailuresStore::connect(&path).await.unwrap();
        (d, s)
    }

    #[tokio::test]
    async fn guarda_e_lista_a_mais_recente_primeiro() {
        let (_d, s) = store().await;
        s.record("vigia", "Chave de API faltando").await.unwrap();
        s.record("arquivista", "").await.unwrap();
        let l = s.list(10).await.unwrap();
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].agent_id, "arquivista");
        assert_eq!(l[0].title, "Falha do agente");
        assert_eq!(l[1].title, "Chave de API faltando");
        assert_eq!(s.list(1).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn mantem_so_as_mais_recentes() {
        let (_d, s) = store().await;
        for i in 0..(KEEP + 5) {
            s.record("a", &format!("f{i}")).await.unwrap();
        }
        let l = s.list(MAX_LIMIT).await.unwrap();
        assert_eq!(l.len() as i64, MAX_LIMIT);
        let tudo: i64 = sqlx::query("SELECT COUNT(*) AS n FROM failures")
            .fetch_one(&s.pool)
            .await
            .unwrap()
            .get("n");
        assert_eq!(tudo, KEEP);
        assert_eq!(l[0].title, format!("f{}", KEEP + 4));
    }
}
