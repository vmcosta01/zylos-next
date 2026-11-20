//! Spaces API: agrupam conversas, aprovações e quais agentes têm acesso (APP_FLOW v2.1 §2).
//!
//! Um Space tem um nome e uma lista de agentes com acesso. O Space padrão "Geral" é aberto:
//! todos os agentes entram nele e ele não pode ser apagado. O cockpit usa a lista de agentes para
//! filtrar o Inbox, as aprovações no chat e os bots do chat.
//!
//! ## Endpoints (sob `/api/v1`)
//! - `GET    /spaces`      lista os Spaces com seus agentes
//! - `POST   /spaces`      cria um Space `{ "name": "...", "description": "..." }`
//! - `PUT    /spaces/:id`  define `{ "open_access": bool, "agents": ["..."] }`
//! - `DELETE /spaces/:id`  apaga (o Space "Geral" responde 409)

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqliteConnectOptions, Row, SqlitePool};
use std::str::FromStr;

/// Id fixo do Space padrão.
pub const GENERAL_ID: &str = "geral";
const MAX_AGENTS: usize = 100;

#[derive(Clone)]
pub struct SpacesStore {
    pool: SqlitePool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Space {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Aberto: todos os agentes têm acesso (só o "Geral").
    pub open_access: bool,
    pub agents: Vec<String>,
    pub created_at: String,
}

#[derive(Debug)]
pub enum SpacesError {
    Invalid(&'static str),
    NameTaken,
    NotFound,
    Protected,
    Db(sqlx::Error),
}

impl From<sqlx::Error> for SpacesError {
    fn from(e: sqlx::Error) -> Self {
        SpacesError::Db(e)
    }
}

impl SpacesStore {
    /// Conecta (cria o arquivo se faltar) e migra; garante o Space "Geral".
    pub async fn connect(path: &str) -> Result<Self, sqlx::Error> {
        let opts = SqliteConnectOptions::from_str(path)?.create_if_missing(true);
        let pool = SqlitePool::connect_with(opts).await?;
        let store = Self { pool };
        store.migrate().await?;
        Ok(store)
    }

    async fn migrate(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS spaces (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                description TEXT NOT NULL DEFAULT '',
                open_access INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL
            )",
        )
        .execute(&self.pool)
        .await?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS space_agents (
                space_id TEXT NOT NULL REFERENCES spaces(id) ON DELETE CASCADE,
                agent TEXT NOT NULL,
                PRIMARY KEY (space_id, agent)
            )",
        )
        .execute(&self.pool)
        .await?;
        sqlx::query(
            "INSERT OR IGNORE INTO spaces (id, name, description, open_access, created_at)
             VALUES (?, 'Geral', 'Todos os agentes e conversas', 1, ?)",
        )
        .bind(GENERAL_ID)
        .bind(chrono::Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list(&self) -> Result<Vec<Space>, SpacesError> {
        let rows = sqlx::query(
            "SELECT id, name, description, open_access, created_at FROM spaces ORDER BY (id = ?) DESC, created_at ASC",
        )
        .bind(GENERAL_ID)
        .fetch_all(&self.pool)
        .await?;
        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            let id: String = r.get("id");
            let agents =
                sqlx::query("SELECT agent FROM space_agents WHERE space_id = ? ORDER BY agent")
                    .bind(&id)
                    .fetch_all(&self.pool)
                    .await?
                    .into_iter()
                    .map(|a| a.get::<String, _>("agent"))
                    .collect();
            out.push(Space {
                id,
                name: r.get("name"),
                description: r.get("description"),
                open_access: r.get::<i64, _>("open_access") != 0,
                agents,
                created_at: r.get("created_at"),
            });
        }
        Ok(out)
    }

    pub async fn create(&self, name: &str, description: &str) -> Result<Space, SpacesError> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 60 {
            return Err(SpacesError::Invalid(
                "o nome precisa ter de 1 a 60 caracteres",
            ));
        }
        if description.chars().count() > 300 {
            return Err(SpacesError::Invalid("a descrição passa de 300 caracteres"));
        }
        let id = uuid::Uuid::new_v4().to_string();
        let res = sqlx::query(
            "INSERT INTO spaces (id, name, description, open_access, created_at) VALUES (?, ?, ?, 0, ?)",
        )
        .bind(&id)
        .bind(name)
        .bind(description.trim())
        .bind(chrono::Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await;
        match res {
            Ok(_) => {}
            Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
                return Err(SpacesError::NameTaken)
            }
            Err(e) => return Err(e.into()),
        }
        self.get(&id).await
    }

    async fn get(&self, id: &str) -> Result<Space, SpacesError> {
        self.list()
            .await?
            .into_iter()
            .find(|s| s.id == id)
            .ok_or(SpacesError::NotFound)
    }

    /// Define o acesso. O "Geral" continua aberto (não aceita lista própria).
    pub async fn set_access(
        &self,
        id: &str,
        open_access: bool,
        agents: &[String],
    ) -> Result<Space, SpacesError> {
        if id == GENERAL_ID && !open_access {
            return Err(SpacesError::Protected);
        }
        if agents.len() > MAX_AGENTS {
            return Err(SpacesError::Invalid("agentes demais neste Space"));
        }
        let mut clean: Vec<String> = Vec::new();
        for a in agents {
            let a = a.trim();
            if a.is_empty() || a.chars().count() > 64 {
                return Err(SpacesError::Invalid("nome de agente inválido"));
            }
            if !clean.iter().any(|x| x == a) {
                clean.push(a.to_string());
            }
        }
        self.get(id).await?;
        let mut tx = self.pool.begin().await?;
        sqlx::query("UPDATE spaces SET open_access = ? WHERE id = ?")
            .bind(open_access as i64)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM space_agents WHERE space_id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        for a in &clean {
            sqlx::query("INSERT INTO space_agents (space_id, agent) VALUES (?, ?)")
                .bind(id)
                .bind(a)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        self.get(id).await
    }

    pub async fn delete(&self, id: &str) -> Result<(), SpacesError> {
        if id == GENERAL_ID {
            return Err(SpacesError::Protected);
        }
        sqlx::query("DELETE FROM space_agents WHERE space_id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        let res = sqlx::query("DELETE FROM spaces WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        if res.rows_affected() == 0 {
            return Err(SpacesError::NotFound);
        }
        Ok(())
    }
}

fn status_of(e: &SpacesError) -> StatusCode {
    match e {
        SpacesError::Invalid(_) => StatusCode::BAD_REQUEST,
        SpacesError::NameTaken | SpacesError::Protected => StatusCode::CONFLICT,
        SpacesError::NotFound => StatusCode::NOT_FOUND,
        SpacesError::Db(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn fail(e: SpacesError) -> axum::response::Response {
    let status = status_of(&e);
    let msg = match &e {
        SpacesError::Invalid(m) => m.to_string(),
        SpacesError::NameTaken => "já existe um Space com esse nome".to_string(),
        SpacesError::NotFound => "Space não encontrado".to_string(),
        SpacesError::Protected => "o Space Geral não pode ser alterado desse jeito".to_string(),
        SpacesError::Db(err) => {
            tracing::error!(error = %err, "spaces db error");
            "erro interno".to_string()
        }
    };
    (status, Json(serde_json::json!({ "error": msg }))).into_response()
}

#[derive(Debug, Deserialize)]
struct CreateBody {
    name: String,
    #[serde(default)]
    description: String,
}

#[derive(Debug, Deserialize)]
struct AccessBody {
    #[serde(default)]
    open_access: bool,
    #[serde(default)]
    agents: Vec<String>,
}

async fn list_spaces(State(s): State<SpacesStore>) -> axum::response::Response {
    match s.list().await {
        Ok(v) => Json(serde_json::json!({ "spaces": v })).into_response(),
        Err(e) => fail(e),
    }
}

async fn create_space(
    State(s): State<SpacesStore>,
    Json(b): Json<CreateBody>,
) -> axum::response::Response {
    match s.create(&b.name, &b.description).await {
        Ok(sp) => (StatusCode::CREATED, Json(sp)).into_response(),
        Err(e) => fail(e),
    }
}

async fn put_space(
    State(s): State<SpacesStore>,
    Path(id): Path<String>,
    Json(b): Json<AccessBody>,
) -> axum::response::Response {
    match s.set_access(&id, b.open_access, &b.agents).await {
        Ok(sp) => Json(sp).into_response(),
        Err(e) => fail(e),
    }
}

async fn delete_space(
    State(s): State<SpacesStore>,
    Path(id): Path<String>,
) -> axum::response::Response {
    match s.delete(&id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => fail(e),
    }
}

pub fn create_spaces_router(store: SpacesStore) -> Router {
    Router::new()
        .route("/spaces", get(list_spaces).post(create_space))
        .route("/spaces/:id", put(put_space).delete(delete_space))
        .with_state(store)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    async fn store() -> SpacesStore {
        SpacesStore::connect("sqlite::memory:").await.unwrap()
    }

    #[tokio::test]
    async fn geral_existe_e_e_aberto() {
        let s = store().await;
        let list = s.list().await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, GENERAL_ID);
        assert!(list[0].open_access);
    }

    #[tokio::test]
    async fn criar_define_agentes_e_apagar() {
        let s = store().await;
        let sp = s.create("  Jurídico ", "contratos").await.unwrap();
        assert_eq!(sp.name, "Jurídico");
        assert!(!sp.open_access && sp.agents.is_empty());

        let sp = s
            .set_access(
                &sp.id,
                false,
                &["chief".into(), "chief".into(), " dev ".into()],
            )
            .await
            .unwrap();
        assert_eq!(sp.agents, vec!["chief".to_string(), "dev".to_string()]);

        s.delete(&sp.id).await.unwrap();
        assert_eq!(s.list().await.unwrap().len(), 1);
        assert!(matches!(s.delete(&sp.id).await, Err(SpacesError::NotFound)));
    }

    #[tokio::test]
    async fn validacoes_e_protecao_do_geral() {
        let s = store().await;
        assert!(matches!(
            s.create("   ", "").await,
            Err(SpacesError::Invalid(_))
        ));
        s.create("Equipe", "").await.unwrap();
        assert!(matches!(
            s.create("Equipe", "").await,
            Err(SpacesError::NameTaken)
        ));
        assert!(matches!(
            s.create("Geral", "").await,
            Err(SpacesError::NameTaken)
        ));
        assert!(matches!(
            s.delete(GENERAL_ID).await,
            Err(SpacesError::Protected)
        ));
        assert!(matches!(
            s.set_access(GENERAL_ID, false, &[]).await,
            Err(SpacesError::Protected)
        ));
        assert!(matches!(
            s.set_access("nada", false, &["x".into()]).await,
            Err(SpacesError::NotFound)
        ));
        assert!(matches!(
            s.set_access(GENERAL_ID, true, &["".into()]).await,
            Err(SpacesError::Invalid(_))
        ));
    }

    #[tokio::test]
    async fn rotas_http_funcionam() {
        let app = create_spaces_router(store().await);
        let resp = app
            .clone()
            .oneshot(
                Request::post("/spaces")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"name":"Pessoal"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let id = v["id"].as_str().unwrap().to_string();

        let put = app
            .clone()
            .oneshot(
                Request::put(format!("/spaces/{id}"))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"agents":["chief"]}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(put.status(), StatusCode::OK);

        let list = app
            .clone()
            .oneshot(Request::get("/spaces").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let bytes = axum::body::to_bytes(list.into_body(), 1 << 20)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["spaces"].as_array().unwrap().len(), 2);
        assert_eq!(v["spaces"][1]["agents"][0], "chief");

        let del = app
            .oneshot(
                Request::delete("/spaces/geral")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(del.status(), StatusCode::CONFLICT);
    }
}
