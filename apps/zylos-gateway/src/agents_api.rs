//! Agents API — agentes reais registrados no [`SwarmOrchestrator`].
//!
//! - `GET  /api/v1/agents` — lista os agentes do swarm com contagem de tarefas
//!   e último sinal de atividade (heartbeat ou atualização de tarefa).
//! - `POST /api/v1/agents` — registra (ou atualiza) um agente no swarm.
//!
//! - `GET  /api/v1/swarm/topology` — a topologia do swarm: gateway, orquestrador e os agentes
//!   registrados, com as arestas de controle e as dependências entre tarefas de agentes.
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

/// Mais que isto de tarefas lidas para montar as arestas de dados.
const MAX_TAREFAS_TOPOLOGIA: usize = 1000;
/// Sem heartbeat há mais que isto, o agente aparece como degradado.
const HEARTBEAT_VELHO_S: i64 = 120;

/// Um nó da topologia, no formato que o painel do cockpit lê (`x`/`y` ficam a cargo do cockpit).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NoTopologia {
    pub id: String,
    pub label: String,
    pub node_type: &'static str,
    pub status: &'static str,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ArestaTopologia {
    pub source: String,
    pub target: String,
    pub edge_type: &'static str,
    pub weight: f64,
}

/// Estado do agente na topologia: desativado = offline; sem sinal recente = degradado.
fn status_do_agente(agent: &SwarmAgent, agora: chrono::DateTime<chrono::Utc>) -> &'static str {
    if !agent.active {
        "Offline"
    } else if (agora - agent.last_heartbeat).num_seconds() > HEARTBEAT_VELHO_S {
        "Degraded"
    } else {
        "Online"
    }
}

/// Monta nós e arestas a partir dos agentes e das tarefas. Aresta de controle: orquestrador ->
/// agente. Aresta de dados: agente A -> agente B quando uma tarefa de B depende de uma tarefa de
/// A (o peso é quantas dependências há entre os dois).
pub fn montar_topologia(
    agentes: &[SwarmAgent],
    tarefas: &[zylos_swarm_orchestrator::Task],
    agora: chrono::DateTime<chrono::Utc>,
) -> (Vec<NoTopologia>, Vec<ArestaTopologia>) {
    let mut nos = vec![
        NoTopologia {
            id: "gateway".into(),
            label: "Gateway".into(),
            node_type: "Gateway",
            status: "Online",
            x: 0.0,
            y: 0.0,
        },
        NoTopologia {
            id: "orchestrator".into(),
            label: "Orquestrador".into(),
            node_type: "Orchestrator",
            status: "Online",
            x: 0.0,
            y: 0.0,
        },
    ];
    let mut arestas = vec![ArestaTopologia {
        source: "gateway".into(),
        target: "orchestrator".into(),
        edge_type: "Control",
        weight: 1.0,
    }];
    for a in agentes {
        nos.push(NoTopologia {
            id: a.id.clone(),
            label: a.name.clone(),
            node_type: "Agent",
            status: status_do_agente(a, agora),
            x: 0.0,
            y: 0.0,
        });
        arestas.push(ArestaTopologia {
            source: "orchestrator".into(),
            target: a.id.clone(),
            edge_type: "Control",
            weight: 1.0,
        });
    }
    let dono: std::collections::HashMap<&str, &str> = tarefas
        .iter()
        .filter_map(|t| Some((t.id.as_str(), t.assigned_agent.as_deref()?)))
        .collect();
    let conhecido = |id: &str| agentes.iter().any(|a| a.id == id);
    let mut pesos: std::collections::BTreeMap<(String, String), f64> =
        std::collections::BTreeMap::new();
    for t in tarefas {
        let Some(destino) = t.assigned_agent.as_deref() else {
            continue;
        };
        for dep in &t.dependencies {
            if let Some(origem) = dono.get(dep.as_str()) {
                if origem != &destino && conhecido(origem) && conhecido(destino) {
                    *pesos
                        .entry((origem.to_string(), destino.to_string()))
                        .or_insert(0.0) += 1.0;
                }
            }
        }
    }
    for ((origem, destino), peso) in pesos {
        arestas.push(ArestaTopologia {
            source: origem,
            target: destino,
            edge_type: "Data",
            weight: peso,
        });
    }
    (nos, arestas)
}

async fn get_topology(State(swarm): State<Arc<SwarmOrchestrator>>) -> axum::response::Response {
    let agentes = match swarm.list_agents().await {
        Ok(a) => a,
        Err(e) => {
            tracing::error!(error = %e, "falha ao listar agentes para a topologia");
            return erro(StatusCode::INTERNAL_SERVER_ERROR, "falha ao ler agentes");
        }
    };
    let q = zylos_swarm_orchestrator::TaskQuery {
        limit: Some(MAX_TAREFAS_TOPOLOGIA),
        ..Default::default()
    };
    let tarefas = match swarm.query_tasks(&q).await {
        Ok(t) => t,
        Err(e) => {
            tracing::error!(error = %e, "falha ao ler tarefas para a topologia");
            return erro(StatusCode::INTERNAL_SERVER_ERROR, "falha ao ler tarefas");
        }
    };
    let (nodes, edges) = montar_topologia(&agentes, &tarefas, chrono::Utc::now());
    let online = nodes.iter().filter(|n| n.status == "Online").count();
    let avg_weight = if edges.is_empty() {
        0.0
    } else {
        edges.iter().map(|e| e.weight).sum::<f64>() / edges.len() as f64
    };
    Json(serde_json::json!({
        "stats": {
            "total_nodes": nodes.len(),
            "online_nodes": online,
            "total_edges": edges.len(),
            "avg_weight": avg_weight,
        },
        "nodes": nodes,
        "edges": edges,
    }))
    .into_response()
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
        .route("/swarm/topology", get(get_topology))
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
    async fn topologia_so_com_o_que_existe() {
        let sw = swarm().await;
        let app = create_agents_router(sw.clone());
        // Sem agentes: só gateway e orquestrador, uma aresta de controle.
        let r = app
            .clone()
            .oneshot(Request::get("/swarm/topology").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::OK);
        let v = body_json(r).await;
        assert_eq!(v["stats"]["total_nodes"], 2);
        assert_eq!(v["edges"].as_array().unwrap().len(), 1);

        // Dois agentes e uma tarefa de B que depende de uma tarefa de A.
        let mut a = SwarmAgent::new("a", "Agente A");
        a.capabilities = vec!["x".into()];
        let b = SwarmAgent::new("b", "Agente B");
        sw.register_agent(&a).await.unwrap();
        sw.register_agent(&b).await.unwrap();
        let mut t1 = Task::new("t1", "primeira");
        t1.assigned_agent = Some("a".into());
        sw.add_task(&t1).await.unwrap();
        let mut t2 = Task::new("t2", "segunda").with_dependencies(vec![t1.id.clone()]);
        t2.assigned_agent = Some("b".into());
        sw.add_task(&t2).await.unwrap();

        let r = app
            .oneshot(Request::get("/swarm/topology").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let v = body_json(r).await;
        let nos: Vec<_> = v["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n["id"].as_str().unwrap())
            .collect();
        assert_eq!(nos, ["gateway", "orchestrator", "a", "b"]);
        let tipos: Vec<_> = v["edges"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| {
                format!(
                    "{}>{}:{}",
                    e["source"].as_str().unwrap(),
                    e["target"].as_str().unwrap(),
                    e["edge_type"].as_str().unwrap()
                )
            })
            .collect();
        assert!(
            tipos.contains(&"orchestrator>a:Control".to_string()),
            "{tipos:?}"
        );
        assert!(
            tipos.contains(&"orchestrator>b:Control".to_string()),
            "{tipos:?}"
        );
        assert!(tipos.contains(&"a>b:Data".to_string()), "{tipos:?}");
        assert_eq!(v["stats"]["total_nodes"], 4);
        assert_eq!(v["stats"]["online_nodes"], 4);
        // x/y ficam a cargo do cockpit; nada de posição inventada aqui além do zero.
        assert_eq!(v["nodes"][2]["status"], "Online");
    }

    #[test]
    fn agente_sem_sinal_recente_ou_desativado_nao_aparece_online() {
        let agora = chrono::Utc::now();
        let mut a = SwarmAgent::new("a", "A");
        assert_eq!(status_do_agente(&a, agora), "Online");
        a.last_heartbeat = agora - chrono::Duration::seconds(HEARTBEAT_VELHO_S + 5);
        assert_eq!(status_do_agente(&a, agora), "Degraded");
        a.active = false;
        assert_eq!(status_do_agente(&a, agora), "Offline");
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
