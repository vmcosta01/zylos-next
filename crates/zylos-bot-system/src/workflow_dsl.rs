//! Workflow DSL — Parser e compilador de especificações YAML/TOML para DagDefinition
//!
//! Implementa a Task 2.1.3: DSL declarativa para workflows com validação
//! de schema, detecção de ciclos em parse-time, e rich error messages.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::dag::{DagDefinition, DagEdge, DagId, DagNode};
use crate::error::{BotError, Result};

// ---------------------------------------------------------------------------
// Schema types (YAML / TOML → intermediate representation)
// ---------------------------------------------------------------------------

/// Especificação completa de um workflow (formato serializável YAML/TOML)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowSpec {
    /// Nome do workflow
    pub name: String,
    /// Versão semântica (ex: "1.0.0")
    #[serde(default = "default_version")]
    pub version: String,
    /// Descrição opcional
    #[serde(default)]
    pub description: Option<String>,
    /// Nós do workflow
    pub nodes: Vec<NodeSpec>,
    /// Arestas direcionadas entre nós
    #[serde(default)]
    pub edges: Vec<EdgeSpec>,
    /// Configurações globais do workflow
    #[serde(default)]
    pub config: WorkflowConfig,
}

fn default_version() -> String {
    "1.0.0".to_string()
}

/// Especificação de um nó no workflow DSL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeSpec {
    /// Identificador único dentro do workflow
    pub id: String,
    /// Nome do agente executor
    pub agent: String,
    /// Especificação da tarefa (valor livre, repassado ao agente)
    #[serde(default)]
    pub task: serde_json::Value,
    /// Timeout em milissegundos (default: 30000)
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    /// Política de retry
    #[serde(default)]
    pub retry: RetrySpec,
    /// Se true, o executor deve verificar idempotência antes de re-executar
    #[serde(default)]
    pub verify: bool,
    /// Se true, o nó é seguro para re-execução sem efeitos colaterais
    #[serde(default)]
    pub idempotent: bool,
    /// Artefatos produzidos por este nó (nomes simbólicos)
    #[serde(default)]
    pub artifacts: Vec<String>,
}

fn default_timeout() -> u64 {
    30_000
}

/// Política de retry para um nó
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrySpec {
    /// Número máximo de tentativas adicionais (0 = sem retry)
    #[serde(default)]
    pub max_retries: u32,
    /// Backoff entre retries em ms
    #[serde(default = "default_backoff")]
    pub backoff_ms: u64,
}

impl Default for RetrySpec {
    fn default() -> Self {
        Self {
            max_retries: 0,
            backoff_ms: 100,
        }
    }
}

fn default_backoff() -> u64 {
    100
}

/// Especificação de uma aresta no workflow DSL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeSpec {
    /// ID do nó de origem
    pub from: String,
    /// ID do nó de destino
    pub to: String,
    /// Condição opcional (expressão simples avaliada em runtime)
    #[serde(default)]
    pub condition: Option<String>,
}

/// Configurações globais do workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowConfig {
    /// Concorrência máxima de nós em execução simultânea
    #[serde(default = "default_max_concurrency")]
    pub max_concurrency: usize,
    /// Se checkpoints intermediários devem ser persistidos
    #[serde(default)]
    pub checkpoint_enabled: bool,
    /// Intervalo de heartbeat em segundos
    #[serde(default = "default_heartbeat")]
    pub heartbeat_interval_s: u64,
}

impl Default for WorkflowConfig {
    fn default() -> Self {
        Self {
            max_concurrency: default_max_concurrency(),
            checkpoint_enabled: false,
            heartbeat_interval_s: default_heartbeat(),
        }
    }
}

fn default_max_concurrency() -> usize {
    4
}

fn default_heartbeat() -> u64 {
    30
}

// ---------------------------------------------------------------------------
// Validation errors with location info
// ---------------------------------------------------------------------------

/// Erro de validação do workflow DSL com contexto de localização
#[derive(Debug, Clone)]
pub struct ValidationError {
    /// Mensagem descritiva do erro
    pub message: String,
    /// Campo ou caminho onde o erro ocorreu
    pub path: String,
    /// Linha no documento fonte (se disponível)
    pub line: Option<usize>,
    /// Coluna no documento fonte (se disponível)
    pub column: Option<usize>,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)?;
        if !self.path.is_empty() {
            write!(f, " (at {})", self.path)?;
        }
        if let (Some(line), Some(col)) = (self.line, self.column) {
            write!(f, " [line {}, col {}]", line, col)?;
        } else if let Some(line) = self.line {
            write!(f, " [line {}]", line)?;
        }
        Ok(())
    }
}

impl From<ValidationError> for BotError {
    fn from(e: ValidationError) -> Self {
        BotError::Internal(format!("Workflow validation: {}", e))
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Parseia uma string YAML como WorkflowSpec, valida e compila para DagDefinition
pub fn parse_workflow(yaml_str: &str) -> Result<DagDefinition> {
    let spec: WorkflowSpec = serde_yaml::from_str(yaml_str).map_err(|e| {
        let loc = e.location();
        let msg = format!(
            "YAML parse error: {}{}",
            e,
            loc.map(|l| format!(" [line {}, col {}]", l.line(), l.column()))
                .unwrap_or_default()
        );
        BotError::Internal(msg)
    })?;
    validate_workflow(&spec)?;
    Ok(compile_spec(spec))
}

/// Parseia uma string TOML como WorkflowSpec, valida e compila para DagDefinition
#[cfg(feature = "toml-format")]
pub fn parse_workflow_toml(toml_str: &str) -> Result<DagDefinition> {
    let spec: WorkflowSpec = toml::from_str(toml_str).map_err(|e| {
        let msg = format!("TOML parse error: {}", e);
        BotError::Internal(msg)
    })?;
    validate_workflow(&spec)?;
    Ok(compile_spec(spec))
}

/// Valida um WorkflowSpec já parseado
///
/// Verifica:
/// - IDs de nós únicos
/// - Referências de arestas apontam para nós existentes
/// - Ausência de ciclos (DFS-based)
/// - Condições não vazias quando presentes
pub fn validate_workflow(spec: &WorkflowSpec) -> Result<()> {
    let mut errors: Vec<ValidationError> = Vec::new();

    // 1. Nome não vazio
    if spec.name.trim().is_empty() {
        errors.push(ValidationError {
            message: "Workflow name must not be empty".into(),
            path: "name".into(),
            line: None,
            column: None,
        });
    }

    // 2. IDs únicos
    let mut seen_ids: HashSet<&str> = HashSet::new();
    for (i, node) in spec.nodes.iter().enumerate() {
        if node.id.trim().is_empty() {
            errors.push(ValidationError {
                message: format!("Node at index {} has empty id", i),
                path: format!("nodes[{}].id", i),
                line: None,
                column: None,
            });
        } else if !seen_ids.insert(node.id.as_str()) {
            errors.push(ValidationError {
                message: format!("Duplicate node id '{}'", node.id),
                path: format!("nodes[{}].id", i),
                line: None,
                column: None,
            });
        }
        if node.agent.trim().is_empty() {
            errors.push(ValidationError {
                message: format!("Node '{}' has empty agent name", node.id),
                path: format!("nodes[{}].agent", i),
                line: None,
                column: None,
            });
        }
    }

    // 3. Arestas referenciam nós existentes + condições válidas
    for (i, edge) in spec.edges.iter().enumerate() {
        if !seen_ids.contains(edge.from.as_str()) {
            errors.push(ValidationError {
                message: format!("Edge references unknown source node '{}'", edge.from),
                path: format!("edges[{}].from", i),
                line: None,
                column: None,
            });
        }
        if !seen_ids.contains(edge.to.as_str()) {
            errors.push(ValidationError {
                message: format!("Edge references unknown target node '{}'", edge.to),
                path: format!("edges[{}].to", i),
                line: None,
                column: None,
            });
        }
        if let Some(ref cond) = edge.condition {
            if cond.trim().is_empty() {
                errors.push(ValidationError {
                    message: "Edge condition must not be empty when present".into(),
                    path: format!("edges[{}].condition", i),
                    line: None,
                    column: None,
                });
            }
        }
    }

    // 4. Detecção de ciclos via DFS
    if errors.is_empty() {
        if let Some(cycle_path) = detect_cycle(spec) {
            errors.push(ValidationError {
                message: format!("Cycle detected: {}", cycle_path.join(" -> ")),
                path: "edges".into(),
                line: None,
                column: None,
            });
        }
    }

    // 5. Config sanity
    if spec.config.max_concurrency == 0 {
        errors.push(ValidationError {
            message: "max_concurrency must be > 0".into(),
            path: "config.max_concurrency".into(),
            line: None,
            column: None,
        });
    }

    if !errors.is_empty() {
        let combined: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
        return Err(BotError::Internal(format!(
            "Workflow validation failed ({} error(s)):\n  - {}",
            errors.len(),
            combined.join("\n  - ")
        )));
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Compilation: WorkflowSpec → DagDefinition
// ---------------------------------------------------------------------------

fn compile_spec(spec: WorkflowSpec) -> DagDefinition {
    let nodes = spec
        .nodes
        .into_iter()
        .map(|n| DagNode {
            id: n.id,
            agent_name: n.agent,
            task_spec: n.task,
            timeout_ms: n.timeout_ms,
            max_retries: n.retry.max_retries,
            backoff_ms: n.retry.backoff_ms,
        })
        .collect();

    let edges = spec
        .edges
        .into_iter()
        .map(|e| DagEdge {
            from: e.from,
            to: e.to,
            condition: e.condition,
        })
        .collect();

    DagDefinition {
        id: DagId::new(),
        name: spec.name,
        nodes,
        edges,
    }
}

// ---------------------------------------------------------------------------
// Cycle detection (DFS coloring)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Color {
    White,
    Gray,
    Black,
}

/// Retorna Some(path) se houver ciclo, descrevendo os nós envolvidos
fn detect_cycle(spec: &WorkflowSpec) -> Option<Vec<String>> {
    let adj: HashMap<&str, Vec<&str>> = spec
        .edges
        .iter()
        .fold(HashMap::new(), |mut map, e| {
            map.entry(e.from.as_str())
                .or_default()
                .push(e.to.as_str());
            map
        });

    let mut color: HashMap<&str, Color> = spec
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), Color::White))
        .collect();

    let mut parent: HashMap<&str, &str> = HashMap::new();

    for node in &spec.nodes {
        if *color.get(node.id.as_str()).unwrap_or(&Color::Black) == Color::White {
            if let Some(cycle) = dfs_visit(node.id.as_str(), &adj, &mut color, &mut parent) {
                return Some(cycle);
            }
        }
    }
    None
}

fn dfs_visit<'a>(
    node: &'a str,
    adj: &HashMap<&str, Vec<&'a str>>,
    color: &mut HashMap<&'a str, Color>,
    parent: &mut HashMap<&'a str, &'a str>,
) -> Option<Vec<String>> {
    color.insert(node, Color::Gray);

    if let Some(neighbors) = adj.get(node) {
        for &next in neighbors {
            match color.get(next).copied().unwrap_or(Color::White) {
                Color::Gray => {
                    // Back edge → cycle found; reconstruct path
                    let mut path = vec![next.to_string()];
                    let mut cur = node;
                    while cur != next {
                        path.push(cur.to_string());
                        cur = *parent.get(cur).unwrap_or(&next);
                    }
                    path.push(next.to_string());
                    path.reverse();
                    return Some(path);
                }
                Color::White => {
                    parent.insert(next, node);
                    if let Some(cycle) = dfs_visit(next, adj, color, parent) {
                        return Some(cycle);
                    }
                }
                Color::Black => {}
            }
        }
    }

    color.insert(node, Color::Black);
    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dag::{DagExecutor, NodeStatus};
    use tokio_util::sync::CancellationToken;

    // -- Valid specs --

    const LINEAR_YAML: &str = r#"
name: linear-pipeline
version: "1.0.0"
description: Simple linear workflow
nodes:
  - id: fetch
    agent: data-fetcher
    task: { url: "https://api.example.com/data" }
    timeout_ms: 10000
    retry:
      max_retries: 2
      backoff_ms: 500
  - id: transform
    agent: transformer
    task: { operation: "normalize" }
  - id: store
    agent: db-writer
    task: { table: "results" }
    verify: true
    idempotent: true
    artifacts: ["report.json"]
edges:
  - from: fetch
    to: transform
  - from: transform
    to: store
config:
  max_concurrency: 2
  checkpoint_enabled: true
  heartbeat_interval_s: 15
"#;

    const PARALLEL_FANOUT_YAML: &str = r#"
name: fan-out-fan-in
nodes:
  - id: split
    agent: splitter
    task: { chunks: 3 }
  - id: worker-1
    agent: processor
    task: { chunk: 1 }
  - id: worker-2
    agent: processor
    task: { chunk: 2 }
  - id: worker-3
    agent: processor
    task: { chunk: 3 }
  - id: merge
    agent: merger
    task: { strategy: "concat" }
edges:
  - from: split
    to: worker-1
  - from: split
    to: worker-2
  - from: split
    to: worker-3
  - from: worker-1
    to: merge
  - from: worker-2
    to: merge
  - from: worker-3
    to: merge
"#;

    const CONDITIONAL_YAML: &str = r#"
name: conditional-branch
nodes:
  - id: check
    agent: validator
    task: { field: "status" }
  - id: success-path
    agent: handler
    task: { action: "approve" }
  - id: failure-path
    agent: handler
    task: { action: "reject" }
edges:
  - from: check
    to: success-path
    condition: "result.status == 'ok'"
  - from: check
    to: failure-path
    condition: "result.status != 'ok'"
"#;

    #[test]
    fn test_parse_linear_workflow() {
        let dag = parse_workflow(LINEAR_YAML).expect("should parse valid linear YAML");
        assert_eq!(dag.name, "linear-pipeline");
        assert_eq!(dag.nodes.len(), 3);
        assert_eq!(dag.edges.len(), 2);
        assert_eq!(dag.nodes[0].timeout_ms, 10000);
        assert_eq!(dag.nodes[0].max_retries, 2);
        assert_eq!(dag.nodes[0].backoff_ms, 500);
    }

    #[test]
    fn test_parse_parallel_fanout() {
        let dag = parse_workflow(PARALLEL_FANOUT_YAML).expect("should parse fan-out YAML");
        assert_eq!(dag.nodes.len(), 5);
        assert_eq!(dag.edges.len(), 6);
        // split tem 3 outgoing edges
        let split_outgoing: Vec<_> = dag
            .edges
            .iter()
            .filter(|e| e.from == "split")
            .collect();
        assert_eq!(split_outgoing.len(), 3);
    }

    #[test]
    fn test_parse_conditional_branching() {
        let dag = parse_workflow(CONDITIONAL_YAML).expect("should parse conditional YAML");
        assert_eq!(dag.edges.len(), 2);
        assert!(dag.edges[0].condition.is_some());
        assert!(dag.edges[1].condition.is_some());
        assert_eq!(
            dag.edges[0].condition.as_deref(),
            Some("result.status == 'ok'")
        );
    }

    // -- Invalid specs --

    #[test]
    fn test_reject_duplicate_node_ids() {
        let yaml = r#"
name: dup
nodes:
  - id: a
    agent: x
  - id: a
    agent: y
"#;
        let err = parse_workflow(yaml).unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("Duplicate node id"), "got: {}", msg);
    }

    #[test]
    fn test_reject_dangling_edge_source() {
        let yaml = r#"
name: dangling
nodes:
  - id: a
    agent: x
edges:
  - from: nonexistent
    to: a
"#;
        let err = parse_workflow(yaml).unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("unknown source"), "got: {}", msg);
    }

    #[test]
    fn test_reject_dangling_edge_target() {
        let yaml = r#"
name: dangling-target
nodes:
  - id: a
    agent: x
edges:
  - from: a
    to: ghost
"#;
        let err = parse_workflow(yaml).unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("unknown target"), "got: {}", msg);
    }

    #[test]
    fn test_reject_cycle() {
        let yaml = r#"
name: cyclic
nodes:
  - id: a
    agent: x
  - id: b
    agent: y
  - id: c
    agent: z
edges:
  - from: a
    to: b
  - from: b
    to: c
  - from: c
    to: a
"#;
        let err = parse_workflow(yaml).unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("Cycle detected"), "got: {}", msg);
    }

    #[test]
    fn test_reject_empty_condition() {
        let yaml = r#"
name: empty-cond
nodes:
  - id: a
    agent: x
  - id: b
    agent: y
edges:
  - from: a
    to: b
    condition: "   "
"#;
        let err = parse_workflow(yaml).unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("condition must not be empty"), "got: {}", msg);
    }

    #[test]
    fn test_reject_invalid_yaml_syntax() {
        let yaml = "name: broken\nnodes:\n  - id: [invalid";
        let err = parse_workflow(yaml).unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("YAML parse error"), "got: {}", msg);
    }

    #[test]
    fn test_reject_empty_name() {
        let yaml = r#"
name: ""
nodes:
  - id: a
    agent: x
"#;
        let err = parse_workflow(yaml).unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("name must not be empty"), "got: {}", msg);
    }

    #[test]
    fn test_reject_zero_concurrency() {
        let yaml = r#"
name: bad-config
nodes:
  - id: a
    agent: x
config:
  max_concurrency: 0
"#;
        let err = parse_workflow(yaml).unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("max_concurrency"), "got: {}", msg);
    }

    // -- Round-trip: YAML → DagDefinition → execute → verify --

    #[tokio::test]
    async fn test_roundtrip_linear_execution() {
        let dag = parse_workflow(LINEAR_YAML).expect("parse should succeed");

        // Verify graph builds without cycles
        let graph = DagExecutor::build_graph(&dag).expect("graph should be acyclic");
        assert_eq!(graph.node_count(), 3);
        assert_eq!(graph.edge_count(), 2);

        // Execute and verify all nodes succeed
        let executor = DagExecutor::new(CancellationToken::new());
        let results = executor
            .execute(&dag, |node_id, _spec, _token| async move {
                Ok(serde_json::json!({ "processed": node_id }))
            })
            .await
            .expect("execution should succeed");

        assert_eq!(results.get("fetch"), Some(&NodeStatus::Success));
        assert_eq!(results.get("transform"), Some(&NodeStatus::Success));
        assert_eq!(results.get("store"), Some(&NodeStatus::Success));
    }

    #[tokio::test]
    async fn test_roundtrip_fanout_execution() {
        let dag = parse_workflow(PARALLEL_FANOUT_YAML).expect("parse should succeed");

        let executor = DagExecutor::new(CancellationToken::new());
        let results = executor
            .execute(&dag, |node_id, _spec, _token| async move {
                Ok(serde_json::json!({ "done": node_id }))
            })
            .await
            .expect("execution should succeed");

        assert_eq!(results.len(), 5);
        for id in &["split", "worker-1", "worker-2", "worker-3", "merge"] {
            assert_eq!(results.get(*id), Some(&NodeStatus::Success));
        }
    }

    #[tokio::test]
    async fn test_roundtrip_conditional_with_failure_skips() {
        let dag = parse_workflow(CONDITIONAL_YAML).expect("parse should succeed");

        let executor = DagExecutor::new(CancellationToken::new());
        let results = executor
            .execute(&dag, |node_id, _spec, _token| async move {
                if node_id == "check" {
                    // Simular falha no check → dependências downstream são skipped
                    Err("validation failed".to_string())
                } else {
                    Ok(serde_json::json!({"handled": node_id}))
                }
            })
            .await
            .expect("execution should complete");

        assert_eq!(results.get("check"), Some(&NodeStatus::Failed));
        // Ambos paths são skipped pois dependem de check
        assert_eq!(results.get("success-path"), Some(&NodeStatus::Skipped));
        assert_eq!(results.get("failure-path"), Some(&NodeStatus::Skipped));
    }

    // -- Defaults --

    #[test]
    fn test_defaults_applied() {
        let yaml = r#"
name: minimal
nodes:
  - id: a
    agent: x
"#;
        let dag = parse_workflow(yaml).expect("minimal spec should parse");
        assert_eq!(dag.nodes[0].timeout_ms, 30_000);
        assert_eq!(dag.nodes[0].max_retries, 0);
        assert_eq!(dag.nodes[0].backoff_ms, 100);
    }

    // -- Serialization round-trip --

    #[test]
    fn test_spec_serialization_roundtrip() {
        let spec: WorkflowSpec =
            serde_yaml::from_str(LINEAR_YAML).expect("deserialize should work");
        let serialized = serde_yaml::to_string(&spec).expect("serialize should work");
        let reparsed: WorkflowSpec =
            serde_yaml::from_str(&serialized).expect("re-deserialize should work");
        assert_eq!(reparsed.name, spec.name);
        assert_eq!(reparsed.nodes.len(), spec.nodes.len());
        assert_eq!(reparsed.edges.len(), spec.edges.len());
    }
}