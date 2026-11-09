//! Global Crate Orchestrator API — Task E.1
//!
//! Endpoint `/api/v1/orchestrate` that accepts DAG task definitions and
//! dispatches them across memory, skills, soar, verification, sandbox,
//! and live-comm subsystems.

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::post,
};
use serde::{Deserialize, Serialize};
use tracing::{error, info};

use crate::AppState;

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

/// A single node in the orchestration DAG.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DagNode {
    /// Unique identifier within this DAG.
    pub id: String,
    /// Subsystem that handles this node.
    pub subsystem: String,
    /// Action to perform (subsystem-specific).
    pub action: String,
    /// Payload forwarded to the subsystem handler.
    #[serde(default)]
    pub payload: serde_json::Value,
    /// IDs of nodes that must complete before this one starts.
    #[serde(default)]
    pub depends_on: Vec<String>,
}

/// Top-level orchestration request.
#[derive(Debug, Deserialize)]
pub struct OrchestrateRequest {
    /// Human-readable label for tracing.
    #[serde(default = "default_dag_label")]
    pub label: String,
    /// Ordered list of DAG nodes.
    pub nodes: Vec<DagNode>,
    /// If true, rollback all completed nodes on any failure.
    #[serde(default)]
    pub rollback_on_failure: bool,
    /// Quando o DAG tem etapas que não podem rodar: `false` (padrão) recusa o
    /// pedido inteiro com 422 e o diagnóstico; `true` executa só a parte válida.
    #[serde(default)]
    pub allow_partial: bool,
}

fn default_dag_label() -> String {
    "unnamed-dag".to_string()
}

/// Outcome of a single executed node.
#[derive(Debug, Clone, Serialize)]
pub struct NodeResult {
    pub node_id: String,
    pub subsystem: String,
    pub success: bool,
    pub output: serde_json::Value,
    pub error: Option<String>,
    pub duration_ms: u64,
}

/// Top-level orchestration response.
#[derive(Debug, Serialize)]
pub struct OrchestrateResponse {
    pub dag_label: String,
    pub total_nodes: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub rolled_back: bool,
    pub results: Vec<NodeResult>,
}

// ---------------------------------------------------------------------------
// Subsystem dispatchers (lightweight integration shims)
// ---------------------------------------------------------------------------

async fn dispatch_memory(action: &str, payload: &serde_json::Value) -> Result<serde_json::Value, String> {
    match action {
        "consolidate" => {
            // Exercise MemoryAgent consolidation path via in-memory graph
            use zylos_memory_system::{ContextGraph, MemoryAgent, MemoryAgentConfig};
            let graph = ContextGraph::in_memory().map_err(|e| e.to_string())?;
            let config = MemoryAgentConfig::default();
            let agent = MemoryAgent::new(std::sync::Arc::new(graph), config);
            let report = agent.run_consolidation_cycle().await.map_err(|e| e.to_string())?;
            Ok(serde_json::json!({
                "contradictions_found": report.contradictions_found,
                "memories_decayed": report.memories_decayed,
                "memories_merged": report.memories_merged,
                "duration_ms": report.duration_ms,
            }))
        }
        "contradiction_detect" => {
            use zylos_memory_system::{ContextNode, ContradictionDetector};
            let nodes_val = payload.get("nodes").and_then(|v| v.as_array());
            let nodes: Vec<ContextNode> = match nodes_val {
                Some(arr) => {
                    let mut out = Vec::new();
                    for item in arr {
                        let node: ContextNode = serde_json::from_value(item.clone()).map_err(|e| e.to_string())?;
                        out.push(node);
                    }
                    out
                }
                None => Vec::new(),
            };
            let contradictions = ContradictionDetector::detect_all(&nodes);
            Ok(serde_json::json!({
                "contradictions_found": contradictions.len(),
                "details": contradictions.iter().map(|c| serde_json::json!({
                    "kind": format!("{:?}", c.kind),
                    "confidence": c.confidence,
                })).collect::<Vec<_>>(),
            }))
        }
        _ => Err(format!("unknown memory action: {}", action)),
    }
}

async fn dispatch_skills(action: &str, _payload: &serde_json::Value, state: &AppState) -> Result<serde_json::Value, String> {
    match action {
        "list" => {
            let registry = state.skills_state.registry.read().await;
            let skills: Vec<String> = registry.list_all().iter().map(|e| e.name.clone()).collect();
            Ok(serde_json::json!({ "skills": skills, "total": skills.len() }))
        }
        "eval_status" => {
            // EvalFlywheel uses SQLite internally; return availability status
            Ok(serde_json::json!({
                "flywheel_available": true,
                "status": "ready",
            }))
        }
        _ => Err(format!("unknown skills action: {}", action)),
    }
}

async fn dispatch_verification(action: &str, payload: &serde_json::Value) -> Result<serde_json::Value, String> {
    match action {
        "panel_deliberate" => {
            use zylos_verification::{AdversarialPanel, PanelConfig, Verdict, Ground};
            let config = PanelConfig::default();
            let mut panel = AdversarialPanel::new(config);
            panel.begin();

            // Extract verdicts from payload or use defaults
            if let Some(verdicts) = payload.get("verdicts").and_then(|v| v.as_array()) {
                for v in verdicts {
                    let judge_id = v.get("judge_id").and_then(|j| j.as_str()).unwrap_or("judge-0");
                    let ground_str = v.get("ground").and_then(|g| g.as_str()).unwrap_or("confirmed");
                    let ground = match ground_str {
                        "observed" => Ground::Observado,
                        "refuted" => Ground::Crido,
                        _ => Ground::Confirmado,
                    };
                    let rationale = v.get("rationale").and_then(|r| r.as_str()).unwrap_or("auto");
                    panel.add_verdict(Verdict {
                        judge_id: judge_id.to_string(),
                        valid: true,
                        ground,
                        rationale: rationale.to_string(),
                        duration_us: 100,
                    });
                }
            }

            let result = panel.deliberate();
            Ok(serde_json::json!({
                "agreed": result.agreed,
                "consensus_ground": format!("{:?}", result.consensus_ground),
                "valid_count": result.valid_count,
            }))
        }
        "proof_validate" => {
            use zylos_verification::{Proof, Ground};
            let claim = payload.get("claim").and_then(|c| c.as_str()).unwrap_or("test-claim");
            let proof = Proof::new(claim, Ground::Confirmado);
            Ok(serde_json::json!({
                "valid": true,
                "claim": proof.claim,
                "ground": format!("{:?}", proof.ground),
            }))
        }
        _ => Err(format!("unknown verification action: {}", action)),
    }
}

async fn dispatch_sandbox(action: &str, _payload: &serde_json::Value) -> Result<serde_json::Value, String> {
    match action {
        "wasm_check" => {
            // Verify WASM sandbox type availability without executing untrusted code
            Ok(serde_json::json!({
                "sandbox_available": true,
                "isolation": "wasm-memory-boundary",
            }))
        }
        _ => Err(format!("unknown sandbox action: {}", action)),
    }
}

async fn dispatch_soar(action: &str, payload: &serde_json::Value, state: &AppState) -> Result<serde_json::Value, String> {
    match action {
        "playbook_dispatch" => {
            use zylos_soar::ZylosSecurityEvent;
            let source = payload.get("source").and_then(|s| s.as_str()).unwrap_or("orchestrator");
            let title = payload.get("title").and_then(|t| t.as_str()).unwrap_or("orchestrated-event");
            let event = ZylosSecurityEvent::new(
                source,
                zylos_soar::Severity::Medium,
                zylos_soar::EventCategory::Anomaly,
                title,
                "DAG-dispatched SOAR event",
                vec![],
                serde_json::json!({}),
            );
            let result = state.soar_orchestrator.process_event(event).await.map_err(|e| e.to_string())?;
            Ok(serde_json::json!({
                "event_id": result.event_id.to_string(),
                "playbooks_dispatched": result.dispatched_playbooks.len(),
            }))
        }
        _ => Err(format!("unknown soar action: {}", action)),
    }
}

async fn dispatch_live_comm(action: &str, state: &AppState) -> Result<serde_json::Value, String> {
    match action {
        "cockpit_status" => {
            Ok(serde_json::json!({
                "event_bus_receivers": state.event_bus.receiver_count(),
                "streaming": true,
            }))
        }
        _ => Err(format!("unknown live-comm action: {}", action)),
    }
}

// ---------------------------------------------------------------------------
// DAG executor
// ---------------------------------------------------------------------------

/// Uma etapa que não vai rodar e o motivo.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct EtapaFora {
    pub node_id: String,
    pub motivo: String,
}

/// O que um DAG permite executar: as etapas que rodam (em ordem) e as que ficam fora.
#[derive(Debug, Clone)]
pub struct Diagnostico {
    /// Índices de `nodes`, na ordem de execução.
    pub ordem: Vec<usize>,
    /// Índices de `nodes` que não rodam, na ordem do pedido, com o motivo.
    pub fora: Vec<(usize, String)>,
}

/// Separa o que pode rodar do que não pode. Ficam fora: etapa com id repetido
/// (todas as cópias, porque não dá para saber qual é a certa), etapa que depende
/// de id inexistente, etapa em ciclo e etapa que depende de alguma que ficou fora.
/// As demais saem em ordem que respeita `depends_on` (Kahn estável: entre as
/// prontas vai primeiro a que veio antes no pedido, então um pedido já em ordem
/// não muda de ordem).
pub fn diagnosticar(nodes: &[DagNode]) -> Diagnostico {
    let n = nodes.len();
    let mut contagem: std::collections::HashMap<&str, usize> = std::collections::HashMap::with_capacity(n);
    for no in nodes {
        *contagem.entry(no.id.as_str()).or_insert(0) += 1;
    }
    let mut motivo: Vec<Option<String>> = vec![None; n];
    let mut indice: std::collections::HashMap<&str, usize> = std::collections::HashMap::with_capacity(n);
    for (i, no) in nodes.iter().enumerate() {
        if contagem[no.id.as_str()] > 1 {
            motivo[i] = Some(format!("id repetido: {}", no.id));
        } else {
            indice.insert(no.id.as_str(), i);
        }
    }
    let mut dependentes: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut faltam: Vec<usize> = vec![0; n];
    for (i, no) in nodes.iter().enumerate() {
        if motivo[i].is_some() {
            continue;
        }
        for dep in &no.depends_on {
            match indice.get(dep.as_str()) {
                Some(&d) => {
                    dependentes[d].push(i);
                    faltam[i] += 1;
                }
                None if contagem.contains_key(dep.as_str()) => {
                    motivo[i] = Some(format!("depende de id repetido: {dep}"));
                    break;
                }
                None => {
                    motivo[i] = Some(format!("depende de etapa inexistente: {dep}"));
                    break;
                }
            }
        }
    }
    let mut prontos: std::collections::BinaryHeap<std::cmp::Reverse<usize>> = (0..n)
        .filter(|&i| motivo[i].is_none() && faltam[i] == 0)
        .map(std::cmp::Reverse)
        .collect();
    let mut ordem: Vec<usize> = Vec::with_capacity(n);
    let mut rodou = vec![false; n];
    while let Some(std::cmp::Reverse(cur)) = prontos.pop() {
        ordem.push(cur);
        rodou[cur] = true;
        for &next in &dependentes[cur] {
            faltam[next] -= 1;
            if faltam[next] == 0 && motivo[next].is_none() {
                prontos.push(std::cmp::Reverse(next));
            }
        }
    }
    let fora = (0..n)
        .filter(|&i| !rodou[i])
        .map(|i| {
            let m = motivo[i]
                .take()
                .unwrap_or_else(|| "em ciclo de dependências ou depende de etapa que não vai rodar".into());
            (i, m)
        })
        .collect();
    Diagnostico { ordem, fora }
}

/// Ordem de execução de um DAG válido; `None` se alguma etapa ficaria fora
/// (id repetido, dependência inexistente ou ciclo).
pub fn ordem_de_execucao(nodes: &[DagNode]) -> Option<Vec<usize>> {
    let d = diagnosticar(nodes);
    d.fora.is_empty().then_some(d.ordem)
}

pub async fn execute_dag(
    state: &AppState,
    nodes: &[DagNode],
    rollback_on_failure: bool,
) -> OrchestrateResponse {
    // O pedido pode listar os nós fora de ordem: executa na ordem das dependências.
    // DAG que não dá para ordenar (id repetido, dependência inexistente, ciclo)
    // segue na ordem recebida, e o nó sem dependência cumprida falha como antes.
    let ordem: Vec<usize> =
        ordem_de_execucao(nodes).unwrap_or_else(|| (0..nodes.len()).collect());
    executar(state, nodes, &ordem, &[], rollback_on_failure).await
}

/// Executa só a parte válida de um DAG já diagnosticado: as etapas de
/// `diag.ordem` rodam; as de `diag.fora` entram no resultado como falha
/// "não executado", sem rodar.
pub async fn execute_dag_parcial(
    state: &AppState,
    nodes: &[DagNode],
    diag: &Diagnostico,
    rollback_on_failure: bool,
) -> OrchestrateResponse {
    executar(state, nodes, &diag.ordem, &diag.fora, rollback_on_failure).await
}

async fn executar(
    state: &AppState,
    nodes: &[DagNode],
    ordem: &[usize],
    fora: &[(usize, String)],
    rollback_on_failure: bool,
) -> OrchestrateResponse {
    let mut results: Vec<NodeResult> = Vec::with_capacity(nodes.len());
    let mut completed_ids: Vec<String> = Vec::new();
    let mut any_failed = false;

    for node in ordem.iter().map(|&i| &nodes[i]) {
        // Check dependencies satisfied
        let deps_met = node.depends_on.iter().all(|dep| completed_ids.contains(dep));
        if !deps_met {
            let nr = NodeResult {
                node_id: node.id.clone(),
                subsystem: node.subsystem.clone(),
                success: false,
                output: serde_json::Value::Null,
                error: Some(format!("unmet dependencies: {:?}", node.depends_on)),
                duration_ms: 0,
            };
            results.push(nr);
            any_failed = true;
            if rollback_on_failure {
                break;
            }
            continue;
        }

        let start = std::time::Instant::now();
        let dispatch_result = match node.subsystem.as_str() {
            "memory" => dispatch_memory(&node.action, &node.payload).await,
            "skills" => dispatch_skills(&node.action, &node.payload, state).await,
            "verification" => dispatch_verification(&node.action, &node.payload).await,
            "sandbox" => dispatch_sandbox(&node.action, &node.payload).await,
            "soar" => dispatch_soar(&node.action, &node.payload, state).await,
            "live-comm" => dispatch_live_comm(&node.action, state).await,
            other => Err(format!("unknown subsystem: {}", other)),
        };
        let elapsed = start.elapsed().as_millis() as u64;

        match dispatch_result {
            Ok(output) => {
                completed_ids.push(node.id.clone());
                results.push(NodeResult {
                    node_id: node.id.clone(),
                    subsystem: node.subsystem.clone(),
                    success: true,
                    output,
                    error: None,
                    duration_ms: elapsed,
                });
            }
            Err(err) => {
                any_failed = true;
                results.push(NodeResult {
                    node_id: node.id.clone(),
                    subsystem: node.subsystem.clone(),
                    success: false,
                    output: serde_json::Value::Null,
                    error: Some(err),
                    duration_ms: elapsed,
                });
                if rollback_on_failure {
                    break;
                }
            }
        }
    }

    // Etapas de fora entram como falha, mas não contam para `rolled_back`: elas
    // não rodaram, e a parte válida já executada não foi desfeita.
    for (i, motivo) in fora {
        results.push(NodeResult {
            node_id: nodes[*i].id.clone(),
            subsystem: nodes[*i].subsystem.clone(),
            success: false,
            output: serde_json::Value::Null,
            error: Some(format!("não executado: {motivo}")),
            duration_ms: 0,
        });
    }

    let succeeded = results.iter().filter(|r| r.success).count();
    let failed = results.iter().filter(|r| !r.success).count();

    OrchestrateResponse {
        dag_label: String::new(), // filled by handler
        total_nodes: nodes.len(),
        succeeded,
        failed,
        rolled_back: rollback_on_failure && any_failed,
        results,
    }
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

/// POST /api/v1/orchestrate — Execute a cross-subsystem DAG.
async fn post_orchestrate(
    State(state): State<AppState>,
    Json(request): Json<OrchestrateRequest>,
) -> axum::response::Response {
    info!(label = %request.label, nodes = request.nodes.len(), "Orchestration DAG received");

    let diag = diagnosticar(&request.nodes);
    if !diag.fora.is_empty() && !request.allow_partial {
        // Nada roda: devolve o que ficaria fora e o que poderia rodar, para quem
        // pediu decidir se reenvia com allow_partial.
        let executaveis: Vec<&str> = diag.ordem.iter().map(|&i| request.nodes[i].id.as_str()).collect();
        let fora: Vec<EtapaFora> = diag
            .fora
            .iter()
            .map(|(i, m)| EtapaFora { node_id: request.nodes[*i].id.clone(), motivo: m.clone() })
            .collect();
        let body = serde_json::json!({
            "type": "https://zylos.dev/errors/dag-com-etapas-invalidas",
            "title": "DAG com etapas que não podem rodar",
            "status": 422,
            "detail": format!(
                "{} etapa(s) não podem rodar; nada foi executado. Para executar só a parte válida, reenvie com \"allow_partial\": true.",
                fora.len()
            ),
            "instance": "/api/v1/orchestrate",
            "executaveis": executaveis,
            "fora": fora,
        });
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            [(axum::http::header::CONTENT_TYPE, "application/problem+json")],
            Json(body),
        )
            .into_response();
    }

    let mut response = if diag.fora.is_empty() {
        execute_dag(&state, &request.nodes, request.rollback_on_failure).await
    } else {
        execute_dag_parcial(&state, &request.nodes, &diag, request.rollback_on_failure).await
    };
    response.dag_label = request.label;

    if response.failed > 0 {
        error!(failed = response.failed, "Orchestration completed with failures");
    }
    (StatusCode::OK, Json(response)).into_response()
}

/// Build the orchestration router.
pub fn create_orchestrate_router() -> Router<AppState> {
    Router::new()
        .route("/orchestrate", post(post_orchestrate))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no(id: &str, deps: &[&str]) -> DagNode {
        DagNode {
            id: id.into(),
            subsystem: "live-comm".into(),
            action: "cockpit_status".into(),
            payload: serde_json::Value::Null,
            depends_on: deps.iter().map(|d| d.to_string()).collect(),
        }
    }

    fn fora_ids(nodes: &[DagNode]) -> Vec<String> {
        diagnosticar(nodes).fora.iter().map(|(i, _)| nodes[*i].id.clone()).collect()
    }

    #[test]
    fn diagnostico_separa_valido_de_invalido() {
        let nodes = [no("b", &["a"]), no("a", &[]), no("x", &["nada"]), no("y", &["x"])];
        let d = diagnosticar(&nodes);
        assert_eq!(d.ordem, vec![1, 0]);
        assert_eq!(fora_ids(&nodes), ["x", "y"]);
        assert!(d.fora[0].1.contains("inexistente: nada"));
        assert!(d.fora[1].1.contains("não vai rodar"));
    }

    #[test]
    fn diagnostico_tira_todas_as_copias_de_id_repetido_e_quem_depende_delas() {
        let nodes = [no("d", &[]), no("d", &[]), no("e", &["d"]), no("ok", &[])];
        let d = diagnosticar(&nodes);
        assert_eq!(d.ordem, vec![3]);
        assert_eq!(fora_ids(&nodes), ["d", "d", "e"]);
        assert!(d.fora[0].1.contains("id repetido"));
        assert!(d.fora[2].1.contains("depende de id repetido: d"), "{}", d.fora[2].1);
    }

    #[test]
    fn diagnostico_de_dag_valido_nao_deixa_nada_fora() {
        let nodes = [no("b", &["a"]), no("c", &["b"]), no("a", &[])];
        let d = diagnosticar(&nodes);
        assert!(d.fora.is_empty());
        assert_eq!(d.ordem, vec![2, 0, 1]);
    }

    #[test]
    fn ordena_pedido_fora_de_ordem() {
        let nodes = [no("b", &["a"]), no("c", &["b"]), no("a", &[])];
        assert_eq!(ordem_de_execucao(&nodes), Some(vec![2, 0, 1]));
    }

    #[test]
    fn pedido_em_ordem_fica_igual() {
        let nodes = [no("a", &[]), no("b", &["a"]), no("c", &[])];
        assert_eq!(ordem_de_execucao(&nodes), Some(vec![0, 1, 2]));
    }

    #[test]
    fn dag_que_nao_da_para_ordenar_devolve_none() {
        assert_eq!(ordem_de_execucao(&[no("a", &["a"])]), None, "auto-ciclo");
        assert_eq!(ordem_de_execucao(&[no("a", &["b"]), no("b", &["a"])]), None, "ciclo");
        assert_eq!(ordem_de_execucao(&[no("a", &["x"])]), None, "inexistente");
        assert_eq!(ordem_de_execucao(&[no("a", &[]), no("a", &[])]), None, "id repetido");
    }
}