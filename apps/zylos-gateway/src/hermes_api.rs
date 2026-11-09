//! Hermes Integration API — expõe a ponte Hermes (`HermesBridge`) via HTTP.
//!
//! Rotas (montadas sob `/api/v1`):
//! - `POST /hermes/events` — envelope Hermes completo (`HermesEnvelope`)
//! - `POST /hermes/{system}/{action}` — atalho do COMMUNICATION-CONTRACT: o corpo
//!   JSON vira o `payload`, `source = "hermes:{system}"`, `event_type = action`.
//! - `POST /hermes/bot/dispatch-task` — Kanban → BotSystem (forma simples ou DAG)
//! - `POST /hermes/bot/spawn-crew` — Crew → BotSystem (SwarmOrchestrator)
//!
//! Erros seguem RFC 9457 (Problem Details). Os eventos aceitos são publicados no
//! `EventBus` e chegam ao cockpit via SSE/WS (`/api/v1/events`).

use axum::{
    Json, Router,
    extract::{Path, Request, State},
    http::{StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::post,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;
use zylos_live_comm::hermes_bridge::{HermesBridge, HermesEnvelope};
use zylos_swarm_orchestrator::{SwarmAgent, Task as SwarmTask};

use crate::AppState;

/// Sistemas Zylos que aceitam chamadas vindas do Hermes.
const KNOWN_SYSTEMS: &[&str] = &[
    "bot", "kanban", "memory", "skills", "soar", "verification", "sandbox", "live-comm",
];

fn problem(
    status: StatusCode,
    slug: &str,
    title: &str,
    detail: String,
    instance: &str,
) -> Response {
    let body = json!({
        "type": format!("https://zylos.dev/errors/{slug}"),
        "title": title,
        "status": status.as_u16(),
        "detail": detail,
        "instance": instance,
    });
    (
        status,
        [(header::CONTENT_TYPE, "application/problem+json")],
        Json(body),
    )
        .into_response()
}

fn forward(state: &AppState, env: &HermesEnvelope, instance: &str) -> Response {
    match HermesBridge::new(state.event_bus.clone()).forward(env) {
        Ok(receivers) => Json(json!({
            "status": "accepted",
            "source": env.source,
            "event_type": env.event_type,
            "receivers": receivers,
        }))
        .into_response(),
        Err(e) => {
            let msg = e.to_string();
            // Broadcast sem ouvintes (nenhum cockpit conectado) não é falha do cliente.
            if msg.contains("no active receivers") || msg.contains("BroadcastSend") {
                Json(json!({
                    "status": "accepted",
                    "source": env.source,
                    "event_type": env.event_type,
                    "receivers": 0,
                }))
                .into_response()
            } else {
                problem(
                    StatusCode::BAD_REQUEST,
                    "invalid-event",
                    "Invalid Hermes Event",
                    msg,
                    instance,
                )
            }
        }
    }
}

async fn post_event(
    State(state): State<AppState>,
    body: Result<Json<HermesEnvelope>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let instance = "/api/v1/hermes/events";
    let Json(env) = match body {
        Ok(j) => j,
        Err(_) => {
            return problem(
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid-event",
                "Invalid Hermes Event",
                "corpo JSON inválido ou ausente".into(),
                instance,
            );
        }
    };
    forward(&state, &env, instance)
}

async fn post_system_action(
    State(state): State<AppState>,
    Path((system, action)): Path<(String, String)>,
    Json(payload): Json<Value>,
) -> Response {
    let instance = format!("/api/v1/hermes/{system}/{action}");
    if !KNOWN_SYSTEMS.contains(&system.as_str()) {
        return problem(
            StatusCode::NOT_FOUND,
            "unknown-system",
            "Unknown Zylos System",
            format!("system '{system}' not in {KNOWN_SYSTEMS:?}"),
            &instance,
        );
    }
    let env = HermesEnvelope {
        source: format!("hermes:{system}"),
        event_type: action,
        payload,
        timestamp: Some(chrono_now()),
    };
    forward(&state, &env, &instance)
}

fn chrono_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("unix:{secs}")
}

/// Comparação em tempo constante (evita timing attack no token).
fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Exige `Authorization: Bearer <token>`. Sem token configurado, nega tudo (fail-closed).
async fn require_token(
    State(token): State<Arc<Option<String>>>,
    req: Request,
    next: Next,
) -> Response {
    let instance = req.uri().path().to_string();
    let Some(expected) = token.as_ref() else {
        return problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "hermes-auth-not-configured",
            "Hermes Bridge Not Configured",
            "defina ZYLOS_HERMES_TOKEN para habilitar as rotas /hermes/*".into(),
            &instance,
        );
    };
    let provided = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");
    if !ct_eq(provided.as_bytes(), expected.as_bytes()) {
        return problem(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "Unauthorized",
            "token ausente ou inválido".into(),
            &instance,
        );
    }
    next.run(req).await
}

// ---------------------------------------------------------------------------
// dispatch-task — Kanban → BotSystem (COMMUNICATION-CONTRACT)
// ---------------------------------------------------------------------------

/// Nó individual aceito na forma completa (lista de nós).
#[derive(Debug, Deserialize, Clone)]
struct DispatchNodeInput {
    /// ID opcional do nó; se ausente, usa `<task_id>-<índice>`.
    #[serde(default)]
    id: Option<String>,
    subsystem: String,
    action: String,
    #[serde(default)]
    payload: Value,
    #[serde(default)]
    depends_on: Vec<String>,
}

/// Corpo aceito por `POST /api/v1/hermes/bot/dispatch-task`.
///
/// Aceita duas formas (untagged):
/// (a) `{ task_id, subsystem, action, payload? }` — nó único
/// (b) `{ task_id, label?, rollback_on_failure?, nodes: [...] }` — DAG completo
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum DispatchTaskRequest {
    // Batch deve vir ANTES de Single: como todos os campos de Single são
    // Option/default, qualquer JSON seria aceito por ela e o untagged nunca
    // chegaria a tentar Batch. Com Batch primeiro, o campo `nodes` (mesmo
    // com default) força a tentativa de desserializar como Batch; se o JSON
    // não tiver `nodes` como array, falha e o serde tenta Single em seguida.
    Batch {
        task_id: Option<String>,
        #[serde(default)]
        label: Option<String>,
        #[serde(default)]
        rollback_on_failure: bool,
        nodes: Vec<DispatchNodeInput>,
    },
    Single {
        task_id: Option<String>,
        subsystem: Option<String>,
        action: Option<String>,
        #[serde(default)]
        payload: Value,
    },
}

/// Valida e normaliza o pedido de dispatch-task, retornando os DagNodes prontos
/// para o `execute_dag` e metadados derivados (label, rollback).
fn normalize_dispatch_request(
    req: DispatchTaskRequest,
) -> Result<(String, String, bool, Vec<crate::orchestrate_api::DagNode>), (StatusCode, &'static str, String)>
{
    match req {
        DispatchTaskRequest::Single {
            task_id,
            subsystem,
            action,
            payload,
        } => {
            let task_id = task_id.filter(|s| !s.trim().is_empty()).ok_or_else(|| {
                (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "invalid-dispatch-task",
                    "task_id é obrigatório".to_string(),
                )
            })?;
            let subsystem = subsystem.filter(|s| !s.trim().is_empty()).ok_or_else(|| {
                (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "invalid-dispatch-task",
                    "subsystem é obrigatório na forma simplificada".to_string(),
                )
            })?;
            let action = action.filter(|s| !s.trim().is_empty()).ok_or_else(|| {
                (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "invalid-dispatch-task",
                    "action é obrigatório na forma simplificada".to_string(),
                )
            })?;
            let node = crate::orchestrate_api::DagNode {
                id: task_id.clone(),
                subsystem,
                action,
                payload,
                depends_on: Vec::new(),
            };
            Ok((
                task_id.clone(),
                format!("hermes-task-{task_id}"),
                false,
                vec![node],
            ))
        }
        DispatchTaskRequest::Batch {
            task_id,
            label,
            rollback_on_failure,
            nodes,
        } => {
            let task_id = task_id.filter(|s| !s.trim().is_empty()).ok_or_else(|| {
                (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "invalid-dispatch-task",
                    "task_id é obrigatório".to_string(),
                )
            })?;
            if nodes.is_empty() {
                return Err((
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "invalid-dispatch-task",
                    "nodes não pode ser vazio".into(),
                ));
            }

            // Gera ids finais (custom ou <task_id>-<idx>) e valida duplicatas.
            let mut final_ids: Vec<String> = Vec::with_capacity(nodes.len());
            let mut seen_ids = std::collections::HashSet::new();
            for (idx, n) in nodes.iter().enumerate() {
                let id = match &n.id {
                    Some(custom) if !custom.trim().is_empty() => custom.trim().to_string(),
                    _ => format!("{task_id}-{idx}"),
                };
                if !seen_ids.insert(id.clone()) {
                    return Err((
                        StatusCode::UNPROCESSABLE_ENTITY,
                        "invalid-dispatch-task",
                        format!("id duplicado em nodes: {id}"),
                    ));
                }
                final_ids.push(id);
            }

            // Valida depends_on, detecta ciclos e calcula a ordem de execução (Kahn).
            // O execute_dag roda os nós na ordem em que os recebe, então eles saem
            // daqui já ordenados. Entre os nós prontos vai primeiro o que veio antes
            // no pedido: um pedido que já está em ordem não muda de ordem.
            let indice: std::collections::HashMap<&str, usize> = final_ids
                .iter()
                .enumerate()
                .map(|(i, id)| (id.as_str(), i))
                .collect();
            let mut dependentes: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
            let mut faltam: Vec<usize> = vec![0; nodes.len()];
            for (idx, n) in nodes.iter().enumerate() {
                for dep in &n.depends_on {
                    let Some(&d) = indice.get(dep.as_str()) else {
                        return Err((
                            StatusCode::UNPROCESSABLE_ENTITY,
                            "invalid-dispatch-task",
                            format!("depends_on cita id inexistente: {dep}"),
                        ));
                    };
                    dependentes[d].push(idx);
                    faltam[idx] += 1;
                }
            }

            let mut prontos: std::collections::BinaryHeap<std::cmp::Reverse<usize>> = (0..nodes
                .len())
                .filter(|&i| faltam[i] == 0)
                .map(std::cmp::Reverse)
                .collect();
            let mut ordem: Vec<usize> = Vec::with_capacity(nodes.len());
            while let Some(std::cmp::Reverse(cur)) = prontos.pop() {
                ordem.push(cur);
                for &next in &dependentes[cur] {
                    faltam[next] -= 1;
                    if faltam[next] == 0 {
                        prontos.push(std::cmp::Reverse(next));
                    }
                }
            }
            if ordem.len() != nodes.len() {
                return Err((
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "invalid-dispatch-task",
                    "ciclo detectado entre nodes".into(),
                ));
            }

            let mut por_indice: Vec<Option<_>> = nodes.into_iter().map(Some).collect();
            let dag_nodes: Vec<crate::orchestrate_api::DagNode> = ordem
                .into_iter()
                .map(|i| {
                    let n = por_indice[i]
                        .take()
                        .expect("cada nó entra uma única vez na ordem");
                    crate::orchestrate_api::DagNode {
                        id: final_ids[i].clone(),
                        subsystem: n.subsystem,
                        action: n.action,
                        payload: n.payload,
                        depends_on: n.depends_on,
                    }
                })
                .collect();

            let label = label
                .filter(|l| !l.trim().is_empty())
                .unwrap_or_else(|| format!("hermes-task-{task_id}"));

            Ok((task_id, label, rollback_on_failure, dag_nodes))
        }
    }
}

async fn post_dispatch_task(
    State(state): State<AppState>,
    body: Result<Json<DispatchTaskRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let instance = "/api/v1/hermes/bot/dispatch-task";
    let Json(req) = match body {
        Ok(j) => j,
        Err(_) => {
            return problem(
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid-dispatch-task",
                "Invalid Dispatch Task",
                "corpo JSON inválido ou ausente".into(),
                instance,
            );
        }
    };

    let (task_id, label, rollback_on_failure, dag_nodes) =
        match normalize_dispatch_request(req) {
            Ok(v) => v,
            Err((status, slug, detail)) => {
                return problem(status, slug, "Invalid Dispatch Task", detail, instance);
            }
        };

    let mut resp =
        crate::orchestrate_api::execute_dag(&state, &dag_nodes, rollback_on_failure).await;
    resp.dag_label = label.clone();

    // Publica evento no EventBus com resumo por nó (defeito #7).
    let nodes_summary: Vec<Value> = resp
        .results
        .iter()
        .map(|r| {
            json!({
                "node_id": r.node_id,
                "subsystem": r.subsystem,
                "success": r.success,
                "error": r.error,
            })
        })
        .collect();

    let envelope = HermesEnvelope {
        source: "hermes:bot".into(),
        event_type: "dispatch-task".into(),
        payload: json!({
            "task_id": task_id,
            "succeeded": resp.succeeded,
            "failed": resp.failed,
            "nodes": nodes_summary,
        }),
        timestamp: Some(chrono_now()),
    };
    let _ = HermesBridge::new(state.event_bus.clone()).forward(&envelope);

    (
        StatusCode::OK,
        Json(json!({
            "status": "executed",
            "task_id": task_id,
            "label": label,
            "succeeded": resp.succeeded,
            "failed": resp.failed,
            "results": resp.results,
        })),
    )
        .into_response()
}

// ---------------------------------------------------------------------------
// spawn-crew — Crew → BotSystem (COMMUNICATION-CONTRACT)
// ---------------------------------------------------------------------------

/// Corpo aceito por `POST /api/v1/hermes/bot/spawn-crew`.
#[derive(Debug, Deserialize)]
struct SpawnCrewRequest {
    crew_id: Option<String>,
    #[serde(default)]
    members: Vec<CrewMember>,
    #[serde(default)]
    tasks: Vec<CrewTask>,
}

#[derive(Debug, Deserialize)]
struct CrewMember {
    id: String,
    name: String,
    #[serde(default)]
    capabilities: Vec<String>,
    /// Concorrência máxima opcional; repassada ao `SwarmAgent.max_concurrent_tasks`.
    #[serde(default)]
    max_concurrent: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct CrewTask {
    name: String,
    /// Descrição opcional (padrão string vazia).
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    depends_on: Vec<String>,
    /// Alias legado: mantido para compatibilidade.
    #[serde(default)]
    required_capabilities: Vec<String>,
    /// Campo preferencial; se presente, sobrescreve `required_capabilities`.
    #[serde(default)]
    capabilities: Option<Vec<String>>,
    #[serde(default)]
    payload: Value,
}

async fn post_spawn_crew(
    State(state): State<AppState>,
    body: Result<Json<SpawnCrewRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let instance = "/api/v1/hermes/bot/spawn-crew";

    let Json(req) = match body {
        Ok(j) => j,
        Err(_) => {
            return problem(
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid-crew",
                "Invalid Crew",
                "corpo JSON inválido ou ausente".into(),
                instance,
            );
        }
    };

    let crew_id = match req.crew_id {
        Some(ref c) if !c.trim().is_empty() => c.trim().to_string(),
        _ => {
            return problem(
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid-crew",
                "Invalid Crew",
                "crew_id é obrigatório".into(),
                instance,
            );
        }
    };

    if req.members.is_empty() {
        return problem(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid-crew",
            "Invalid Crew",
            "members deve ter pelo menos 1 item".into(),
            instance,
        );
    }

    // IDs de membro únicos.
    {
        let mut seen = std::collections::HashSet::new();
        for m in &req.members {
            if !seen.insert(&m.id) {
                return problem(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "invalid-crew",
                    "Invalid Crew",
                    format!("member id duplicado: {}", m.id),
                    instance,
                );
            }
        }
    }

    // Nomes de task únicos + depends_on só pode citar nomes presentes + detecção de ciclo.
    let _task_names: Vec<String> = {
        let mut names = Vec::with_capacity(req.tasks.len());
        let mut seen_names = std::collections::HashSet::new();
        for t in &req.tasks {
            if !seen_names.insert(&t.name) {
                return problem(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "invalid-crew",
                    "Invalid Crew",
                    format!("task name duplicado: {}", t.name),
                    instance,
                );
            }
            names.push(t.name.clone());
        }
        for t in &req.tasks {
            for dep in &t.depends_on {
                if !seen_names.contains(dep) {
                    return problem(
                        StatusCode::UNPROCESSABLE_ENTITY,
                        "invalid-crew",
                        "Invalid Crew",
                        format!("depends_on cita task inexistente: {}", dep),
                        instance,
                    );
                }
            }
        }

        // Detecção de ciclo (Kahn) sobre nomes das tasks.
        let mut adj: std::collections::HashMap<&str, Vec<&str>> =
            std::collections::HashMap::new();
        let mut indeg: std::collections::HashMap<&str, usize> =
            std::collections::HashMap::new();
        for name in &names {
            adj.entry(name.as_str()).or_default();
            indeg.entry(name.as_str()).or_insert(0);
        }
        for t in &req.tasks {
            let target = t.name.as_str();
            for dep in &t.depends_on {
                adj.entry(dep.as_str()).or_default().push(target);
                *indeg.entry(target).or_insert(0) += 1;
            }
        }
        let mut queue: std::collections::VecDeque<&str> =
            std::collections::VecDeque::new();
        for (&name, &deg) in &indeg {
            if deg == 0 {
                queue.push_back(name);
            }
        }
        let mut visited = 0usize;
        while let Some(cur) = queue.pop_front() {
            visited += 1;
            if let Some(neigh) = adj.get(cur) {
                for &next in neigh {
                    if let Some(d) = indeg.get_mut(next) {
                        *d -= 1;
                        if *d == 0 {
                            queue.push_back(next);
                        }
                    }
                }
            }
        }
        if visited != names.len() {
            return problem(
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid-crew",
                "Invalid Crew",
                "ciclo detectado entre tasks".into(),
                instance,
            );
        }
        names
    };

    // Registra agentes com capacidade de isolamento da crew.
    let crew_cap = format!("crew:{crew_id}");
    let mut agent_ids: Vec<String> = Vec::with_capacity(req.members.len());
    for m in &req.members {
        let mut caps = m.capabilities.clone();
        if !caps.contains(&crew_cap) {
            caps.push(crew_cap.clone());
        }
        let mut agent = SwarmAgent::new(&m.id, &m.name);
        agent.capabilities = caps;
        if let Some(max) = m.max_concurrent {
            agent.max_concurrent_tasks = max.max(1);
        }
        if let Err(e) = state.swarm.register_agent(&agent).await {
            return problem(
                StatusCode::INTERNAL_SERVER_ERROR,
                "swarm-register-agent",
                "Swarm Register Agent Failed",
                e.to_string(),
                instance,
            );
        }
        agent_ids.push(m.id.clone());
    }

    // Ordena topologicamente as tasks para garantir que dependências sejam
    // registradas antes das tasks que delas dependem (defeito #1). A validação
    // de ciclo acima já garante que não há ciclos, então a ordenação sempre
    // produz uma sequência válida.
    let ordered_tasks: Vec<&CrewTask> = {
        let mut adj: std::collections::HashMap<&str, Vec<&str>> =
            std::collections::HashMap::new();
        let mut indeg: std::collections::HashMap<&str, usize> =
            std::collections::HashMap::new();
        let name_set: std::collections::HashSet<&str> =
            req.tasks.iter().map(|t| t.name.as_str()).collect();
        for t in &req.tasks {
            adj.entry(t.name.as_str()).or_default();
            indeg.entry(t.name.as_str()).or_insert(0);
        }
        for t in &req.tasks {
            for dep in &t.depends_on {
                if name_set.contains(dep.as_str()) {
                    adj.entry(dep.as_str()).or_default().push(t.name.as_str());
                    *indeg.entry(t.name.as_str()).or_insert(0) += 1;
                }
            }
        }
        let mut queue: std::collections::VecDeque<&str> =
            std::collections::VecDeque::new();
        for (&name, &deg) in &indeg {
            if deg == 0 {
                queue.push_back(name);
            }
        }
        let mut sorted_names: Vec<&str> = Vec::with_capacity(req.tasks.len());
        while let Some(cur) = queue.pop_front() {
            sorted_names.push(cur);
            if let Some(neigh) = adj.get(cur) {
                for &next in neigh {
                    if let Some(d) = indeg.get_mut(next) {
                        *d -= 1;
                        if *d == 0 {
                            queue.push_back(next);
                        }
                    }
                }
            }
        }
        // Mapeia nome → referência à task original na ordem topológica.
        let name_to_task: std::collections::HashMap<&str, &CrewTask> =
            req.tasks.iter().map(|t| (t.name.as_str(), t)).collect();
        sorted_names
            .into_iter()
            .filter_map(|n| name_to_task.get(n).copied())
            .collect()
    };

    // Mapeia nome → ID gerado pelo swarm para resolver depends_on.
    let mut name_to_id: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    for t in &ordered_tasks {
        let mut task = SwarmTask::new(&t.name, t.description.as_deref().unwrap_or(""));
        // capabilities tem precedência; fallback para required_capabilities.
        let mut req_caps = match &t.capabilities {
            Some(caps) => caps.clone(),
            None => t.required_capabilities.clone(),
        };
        if !req_caps.contains(&crew_cap) {
            req_caps.push(crew_cap.clone());
        }
        task.required_capabilities = req_caps;
        task.payload = t.payload.clone();

        // Resolve dependências por nome → ID já registrado (ordem topológica garantida acima).
        let dep_ids: Vec<String> = t
            .depends_on
            .iter()
            .filter_map(|n| name_to_id.get(n).cloned())
            .collect();
        task.dependencies = dep_ids;

        if let Err(e) = state.swarm.add_task(&task).await {
            return problem(
                StatusCode::INTERNAL_SERVER_ERROR,
                "swarm-add-task",
                "Swarm Add Task Failed",
                e.to_string(),
                instance,
            );
        }
        name_to_id.insert(t.name.clone(), task.id.clone());
    }

    // Inicializa tasks prontas e agenda.
    let _ = state.swarm.initialize_ready_tasks().await;
    let schedule = match state.swarm.schedule().await {
        Ok(s) => s,
        Err(e) => {
            return problem(
                StatusCode::INTERNAL_SERVER_ERROR,
                "swarm-schedule",
                "Swarm Schedule Failed",
                e.to_string(),
                instance,
            );
        }
    };

    // Filtra atribuições desta crew usando a capacidade injetada.
    // schedule.assignments é Vec<(TaskId, AgentId)>.
    let crew_task_ids: std::collections::HashSet<&str> = name_to_id.values().map(|s| s.as_str()).collect();
    let crew_assignments: Vec<Value> = schedule
        .assignments
        .iter()
        .filter(|(task_id, agent_id)| agent_ids.contains(agent_id) && crew_task_ids.contains(task_id.as_str()))
        .map(|(task_id, agent_id)| {
            json!({
                "task_id": task_id,
                "agent_id": agent_id,
            })
        })
        .collect();

    // Filtra blocked/unassignable para apenas tasks desta crew.
    let crew_blocked: Vec<String> = schedule
        .blocked_tasks
        .iter()
        .filter(|id| crew_task_ids.contains(id.as_str()))
        .cloned()
        .collect();
    let crew_unassignable: Vec<String> = schedule
        .unassignable_tasks
        .iter()
        .filter(|id| crew_task_ids.contains(id.as_str()))
        .cloned()
        .collect();

    // Publica evento no EventBus.
    let envelope = HermesEnvelope {
        source: "hermes:bot".into(),
        event_type: "spawn-crew".into(),
        payload: json!({
            "crew_id": crew_id,
            "agents_count": agent_ids.len(),
            "tasks_count": req.tasks.len(),
        }),
        timestamp: Some(chrono_now()),
    };
    let _ = HermesBridge::new(state.event_bus.clone()).forward(&envelope);

    let tasks_out: Vec<Value> = req
        .tasks
        .iter()
        .map(|t| {
            json!({
                "name": t.name,
                "id": name_to_id.get(&t.name).cloned().unwrap_or_default(),
            })
        })
        .collect();

    (
        StatusCode::OK,
        Json(json!({
            "status": "spawned",
            "crew_id": crew_id,
            "agents": agent_ids,
            "tasks": tasks_out,
            "schedule": {
                "assignments": crew_assignments,
                "blocked_tasks": crew_blocked,
                "unassignable_tasks": crew_unassignable,
                "errors": schedule.errors,
            },
        })),
    )
        .into_response()
}

/// Router das rotas Hermes, protegido por Bearer token (`ZYLOS_HERMES_TOKEN`).
///
/// As rotas explícitas (`/hermes/bot/dispatch-task` e `/hermes/bot/spawn-crew`)
/// têm precedência sobre o catch-all `/hermes/:system/:action`.
pub fn create_hermes_router(token: Option<String>) -> Router<AppState> {
    Router::new()
        .route("/hermes/events", post(post_event))
        .route("/hermes/bot/dispatch-task", post(post_dispatch_task))
        .route("/hermes/bot/spawn-crew", post(post_spawn_crew))
        .route("/hermes/:system/:action", post(post_system_action))
        .layer(middleware::from_fn_with_state(Arc::new(token), require_token))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_batch_with_nodes_chooses_batch_variant() {
        let json = r#"{
            "task_id": "kanban-42",
            "nodes": [
                {"subsystem":"sandbox","action":"wasm_check","payload":{},"depends_on":[]}
            ]
        }"#;
        let req: DispatchTaskRequest = serde_json::from_str(json)
            .expect("JSON válido com nodes deve desserializar como Batch");
        match req {
            DispatchTaskRequest::Batch { task_id, nodes, .. } => {
                assert_eq!(task_id.as_deref(), Some("kanban-42"));
                assert_eq!(nodes.len(), 1);
                assert_eq!(nodes[0].subsystem, "sandbox");
                assert_eq!(nodes[0].action, "wasm_check");
            }
            DispatchTaskRequest::Single { .. } => {
                panic!("JSON com campo 'nodes' foi desserializado como Single em vez de Batch");
            }
        }
    }

    #[test]
    fn deserialize_single_without_nodes_chooses_single_variant() {
        let json = r#"{"task_id":"t1","subsystem":"sandbox","action":"wasm_check"}"#;
        let req: DispatchTaskRequest = serde_json::from_str(json)
            .expect("JSON sem nodes deve desserializar como Single");
        match req {
            DispatchTaskRequest::Single { task_id, subsystem, action, .. } => {
                assert_eq!(task_id.as_deref(), Some("t1"));
                assert_eq!(subsystem.as_deref(), Some("sandbox"));
                assert_eq!(action.as_deref(), Some("wasm_check"));
            }
            DispatchTaskRequest::Batch { .. } => {
                panic!("JSON sem campo 'nodes' foi desserializado como Batch em vez de Single");
            }
        }
    }

    #[test]
    fn normalize_batch_with_custom_ids_preserves_ids() {
        let json = r#"{
            "task_id": "dag-custom",
            "nodes": [
                {"id":"a","subsystem":"sandbox","action":"wasm_check","payload":{},"depends_on":[]},
                {"id":"b","subsystem":"live-comm","action":"cockpit_status","payload":{},"depends_on":["a"]}
            ]
        }"#;
        let req: DispatchTaskRequest = serde_json::from_str(json).unwrap();
        let (task_id, _label, _rollback, dag_nodes) = normalize_dispatch_request(req).unwrap();
        assert_eq!(task_id, "dag-custom");
        assert_eq!(dag_nodes.len(), 2);
        assert_eq!(dag_nodes[0].id, "a");
        assert_eq!(dag_nodes[1].id, "b");
        assert_eq!(dag_nodes[1].depends_on, vec!["a"]);
    }

    #[test]
    fn normalize_batch_detects_cycle() {
        let json = r#"{
            "task_id": "dag-cycle",
            "nodes": [
                {"id":"a","subsystem":"sandbox","action":"x","depends_on":["b"]},
                {"id":"b","subsystem":"sandbox","action":"x","depends_on":["a"]}
            ]
        }"#;
        let req: DispatchTaskRequest = serde_json::from_str(json).unwrap();
        let result = normalize_dispatch_request(req);
        assert!(result.is_err());
        let (status, slug, detail) = result.unwrap_err();
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(slug, "invalid-dispatch-task");
        assert!(detail.contains("ciclo"));
    }
}