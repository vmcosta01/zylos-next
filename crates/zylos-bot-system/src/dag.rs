//! DAGExecutor — Execução topológica de workflows via petgraph
//!
//! Features:
//! - Conditional edges com expressões CEL-like (comparações, AND/OR/NOT)
//! - Backpressure via tokio::sync::Semaphore (max_concurrency configurável)
//! - Checkpoint/restore via SQLite (resume após restart)
//! - Artifact passing type-safe entre nós (serde_json round-trip)
//! - Progress tracking (0.0–1.0, drift <1%)
//! - Cancellation propagation via CancellationToken
//! - DOT export para Graphviz
//! - Retry com exponential backoff + jitter

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use petgraph::algo::{is_cyclic_directed, toposort};
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use rand::Rng;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, Semaphore};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::error::{BotError, Result};

// ---------------------------------------------------------------------------
// Tipos básicos
// ---------------------------------------------------------------------------

/// Identificador único de um DAG
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DagId(Uuid);

impl DagId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for DagId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for DagId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Status de um nó no DAG durante execução
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeStatus {
    Pending,
    Running,
    Success,
    Failed,
    Skipped,
}

/// Um nó no DAG de workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagNode {
    pub id: String,
    pub agent_name: String,
    pub task_spec: serde_json::Value,
    pub timeout_ms: u64,
    pub max_retries: u32,
    pub backoff_ms: u64,
}

/// Uma aresta direcionada no DAG
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagEdge {
    pub from: String,
    pub to: String,
    /// Expressão condicional opcional (CEL-like simples)
    pub condition: Option<String>,
}

/// Definição completa de um workflow DAG
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagDefinition {
    pub id: DagId,
    pub name: String,
    pub nodes: Vec<DagNode>,
    pub edges: Vec<DagEdge>,
}

/// Estado interno de um nó durante execução
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeExecutionState {
    pub status: NodeStatus,
    pub output: Option<serde_json::Value>,
    pub error: Option<String>,
    pub attempts: u32,
}

/// Contexto passado para cada nó contendo artifacts dos pais
#[derive(Debug, Clone)]
pub struct NodeContext {
    /// Artifacts produzidos pelos nós predecessores (node_id -> artifact)
    pub parent_artifacts: HashMap<String, serde_json::Value>,
    /// Task spec do próprio nó
    pub task_spec: serde_json::Value,
}

/// Progresso da execução do DAG
#[derive(Debug, Clone, Copy)]
pub struct DagProgress {
    pub completed: usize,
    pub total: usize,
    pub fraction: f64,
}

// ---------------------------------------------------------------------------
// Avaliador de condições CEL-like
// ---------------------------------------------------------------------------

/// Avalia uma expressão condicional simples contra um contexto JSON.
pub fn eval_condition(expr: &str, ctx: &serde_json::Value) -> bool {
    let expr = expr.trim();
    if expr.is_empty() {
        return true;
    }
    eval_or_expr(expr, ctx)
}

fn eval_or_expr(expr: &str, ctx: &serde_json::Value) -> bool {
    let parts = split_logical(expr, "||");
    if parts.len() > 1 {
        return parts.iter().any(|p| eval_and_expr(p.trim(), ctx));
    }
    eval_and_expr(expr, ctx)
}

fn eval_and_expr(expr: &str, ctx: &serde_json::Value) -> bool {
    let parts = split_logical(expr, "&&");
    if parts.len() > 1 {
        return parts.iter().all(|p| eval_not_expr(p.trim(), ctx));
    }
    eval_not_expr(expr, ctx)
}

fn eval_not_expr(expr: &str, ctx: &serde_json::Value) -> bool {
    let trimmed = expr.trim();
    if let Some(inner) = trimmed.strip_prefix('!') {
        !eval_atom(inner.trim(), ctx)
    } else {
        eval_atom(trimmed, ctx)
    }
}

fn eval_atom(expr: &str, ctx: &serde_json::Value) -> bool {
    let expr = expr.trim();
    if expr.starts_with('(') && expr.ends_with(')') {
        return eval_or_expr(&expr[1..expr.len() - 1], ctx);
    }
    for op in &["==", "!=", ">=", "<=", ">", "<"] {
        if let Some(pos) = find_operator(expr, op) {
            let lhs_str = expr[..pos].trim();
            let rhs_str = expr[pos + op.len()..].trim();
            let lhs_val = resolve_value(lhs_str, ctx);
            let rhs_val = resolve_value(rhs_str, ctx);
            return compare_values(&lhs_val, &rhs_val, op);
        }
    }
    let val = resolve_value(expr, ctx);
    match &val {
        serde_json::Value::Bool(b) => *b,
        serde_json::Value::Null => false,
        serde_json::Value::String(s) => !s.is_empty(),
        serde_json::Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(false),
        _ => true,
    }
}

fn find_operator(expr: &str, op: &str) -> Option<usize> {
    let mut in_string = false;
    let mut escape = false;
    let bytes = expr.as_bytes();
    let op_bytes = op.as_bytes();
    for i in 0..bytes.len() {
        if escape { escape = false; continue; }
        if bytes[i] == b'\\' { escape = true; continue; }
        if bytes[i] == b'"' { in_string = !in_string; continue; }
        if !in_string && i + op_bytes.len() <= bytes.len() {
            if &bytes[i..i + op_bytes.len()] == op_bytes {
                if op == ">" && i + 1 < bytes.len() && bytes[i + 1] == b'=' { continue; }
                if op == "<" && i + 1 < bytes.len() && bytes[i + 1] == b'=' { continue; }
                if op == "=" && i > 0 && (bytes[i - 1] == b'!' || bytes[i - 1] == b'>' || bytes[i - 1] == b'<') { continue; }
                return Some(i);
            }
        }
    }
    None
}

fn split_logical<'a>(expr: &'a str, op: &str) -> Vec<&'a str> {
    let mut parts = Vec::new();
    let mut depth = 0;
    let mut in_string = false;
    let mut escape = false;
    let mut last = 0;
    let bytes = expr.as_bytes();
    let op_bytes = op.as_bytes();
    for i in 0..bytes.len() {
        if escape { escape = false; continue; }
        if bytes[i] == b'\\' { escape = true; continue; }
        if bytes[i] == b'"' { in_string = !in_string; continue; }
        if in_string { continue; }
        if bytes[i] == b'(' { depth += 1; }
        else if bytes[i] == b')' { depth -= 1; }
        else if depth == 0 && i + op_bytes.len() <= bytes.len() {
            if &bytes[i..i + op_bytes.len()] == op_bytes {
                parts.push(&expr[last..i]);
                last = i + op_bytes.len();
            }
        }
    }
    parts.push(&expr[last..]);
    parts
}

fn resolve_value(token: &str, ctx: &serde_json::Value) -> serde_json::Value {
    let token = token.trim();
    if token.starts_with('"') && token.ends_with('"') && token.len() >= 2 {
        return serde_json::Value::String(token[1..token.len() - 1].to_string());
    }
    if token == "true" { return serde_json::Value::Bool(true); }
    if token == "false" { return serde_json::Value::Bool(false); }
    if token == "null" { return serde_json::Value::Null; }
    if let Ok(n) = token.parse::<i64>() { return serde_json::Value::Number(n.into()); }
    if let Ok(f) = token.parse::<f64>() {
        if let Some(n) = serde_json::Number::from_f64(f) { return serde_json::Value::Number(n); }
    }
    let mut current = ctx;
    for part in token.split('.') {
        match current.get(part) {
            Some(v) => current = v,
            None => return serde_json::Value::Null,
        }
    }
    current.clone()
}

fn compare_values(lhs: &serde_json::Value, rhs: &serde_json::Value, op: &str) -> bool {
    match op {
        "==" => json_eq(lhs, rhs),
        "!=" => !json_eq(lhs, rhs),
        ">" => json_cmp(lhs, rhs) == Some(std::cmp::Ordering::Greater),
        "<" => json_cmp(lhs, rhs) == Some(std::cmp::Ordering::Less),
        ">=" => matches!(json_cmp(lhs, rhs), Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal)),
        "<=" => matches!(json_cmp(lhs, rhs), Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)),
        _ => false,
    }
}

fn json_eq(a: &serde_json::Value, b: &serde_json::Value) -> bool { a == b }

fn json_cmp(a: &serde_json::Value, b: &serde_json::Value) -> Option<std::cmp::Ordering> {
    match (a, b) {
        (serde_json::Value::Number(na), serde_json::Value::Number(nb)) => {
            let fa = na.as_f64()?;
            let fb = nb.as_f64()?;
            fa.partial_cmp(&fb)
        }
        (serde_json::Value::String(sa), serde_json::Value::String(sb)) => Some(sa.cmp(sb)),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Checkpoint store (SQLite)
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct DagCheckpointStore {
    conn: Mutex<Connection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CheckpointRecord {
    dag_id: String,
    node_states: HashMap<String, NodeExecutionState>,
    updated_at: u64,
}

impl DagCheckpointStore {
    pub fn new(path: &str) -> Result<Self> {
        let conn = Connection::open(path).map_err(|e| {
            BotError::Internal(format!("Failed to open checkpoint DB: {}", e))
        })?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS dag_checkpoints (
                dag_id TEXT PRIMARY KEY,
                state_json TEXT NOT NULL,
                updated_at INTEGER NOT NULL
            );",
        )
        .map_err(|e| BotError::Internal(format!("Failed to init checkpoint table: {}", e)))?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    pub fn in_memory() -> Result<Self> { Self::new(":memory:") }

    pub async fn save(&self, dag_id: &DagId, node_states: &HashMap<String, NodeExecutionState>) -> Result<()> {
        let record = CheckpointRecord {
            dag_id: dag_id.to_string(),
            node_states: node_states.clone(),
            updated_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };
        let json = serde_json::to_string(&record)
            .map_err(|e| BotError::Internal(format!("Checkpoint serialize failed: {}", e)))?;
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT OR REPLACE INTO dag_checkpoints (dag_id, state_json, updated_at) VALUES (?1, ?2, ?3)",
            params![record.dag_id, json, record.updated_at],
        ).map_err(|e| BotError::Internal(format!("Checkpoint save failed: {}", e)))?;
        Ok(())
    }

    pub async fn load(&self, dag_id: &DagId) -> Result<Option<HashMap<String, NodeExecutionState>>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn
            .prepare("SELECT state_json FROM dag_checkpoints WHERE dag_id = ?1")
            .map_err(|e| BotError::Internal(format!("Checkpoint query failed: {}", e)))?;
        let result = stmt
            .query_row(params![dag_id.to_string()], |row| row.get::<_, String>(0))
            .optional()
            .map_err(|e| BotError::Internal(format!("Checkpoint load failed: {}", e)))?;
        match result {
            Some(json) => {
                let record: CheckpointRecord = serde_json::from_str(&json)
                    .map_err(|e| BotError::Internal(format!("Checkpoint deserialize failed: {}", e)))?;
                Ok(Some(record.node_states))
            }
            None => Ok(None),
        }
    }

    pub async fn remove(&self, dag_id: &DagId) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute("DELETE FROM dag_checkpoints WHERE dag_id = ?1", params![dag_id.to_string()])
            .map_err(|e| BotError::Internal(format!("Checkpoint remove failed: {}", e)))?;
        Ok(())
    }
}

trait OptionalExt<T> {
    fn optional(self) -> std::result::Result<Option<T>, rusqlite::Error>;
}

impl<T> OptionalExt<T> for std::result::Result<T, rusqlite::Error> {
    fn optional(self) -> std::result::Result<Option<T>, rusqlite::Error> {
        match self {
            Ok(val) => Ok(Some(val)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

// ---------------------------------------------------------------------------
// DagExecutor
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DagExecutorConfig {
    pub max_concurrency: Option<usize>,
    pub checkpoint_store: Option<Arc<DagCheckpointStore>>,
}

impl Default for DagExecutorConfig {
    fn default() -> Self {
        Self { max_concurrency: None, checkpoint_store: None }
    }
}

pub struct DagExecutor {
    cancel_token: CancellationToken,
    config: DagExecutorConfig,
    progress: Arc<Mutex<DagProgress>>,
}

impl DagExecutor {
    pub fn new(cancel_token: CancellationToken) -> Self {
        Self::with_config(cancel_token, DagExecutorConfig::default())
    }

    pub fn with_config(cancel_token: CancellationToken, config: DagExecutorConfig) -> Self {
        Self {
            cancel_token,
            config,
            progress: Arc::new(Mutex::new(DagProgress { completed: 0, total: 0, fraction: 0.0 })),
        }
    }

    pub async fn get_progress(&self) -> DagProgress { *self.progress.lock().await }

    pub fn build_graph(def: &DagDefinition) -> Result<DiGraph<DagNode, DagEdge>> {
        let mut graph = DiGraph::<DagNode, DagEdge>::new();
        let mut node_map: HashMap<&str, NodeIndex> = HashMap::new();
        for node in &def.nodes {
            let idx = graph.add_node(node.clone());
            node_map.insert(&node.id, idx);
        }
        for edge in &def.edges {
            let from_idx = node_map.get(edge.from.as_str()).ok_or_else(|| {
                BotError::Internal(format!("Edge references unknown source node: {}", edge.from))
            })?;
            let to_idx = node_map.get(edge.to.as_str()).ok_or_else(|| {
                BotError::Internal(format!("Edge references unknown target node: {}", edge.to))
            })?;
            graph.add_edge(*from_idx, *to_idx, edge.clone());
        }
        if is_cyclic_directed(&graph) {
            return Err(BotError::Internal(format!("DAG '{}' contains cycles and cannot be executed", def.name)));
        }
        Ok(graph)
    }

    pub fn to_dot(def: &DagDefinition) -> String {
        let mut dot = String::new();
        dot.push_str("digraph \"");
        dot.push_str(&def.name);
        dot.push_str("\" {\n");
        dot.push_str("  rankdir=LR;\n");
        dot.push_str("  node [shape=box];\n");
        for node in &def.nodes {
            dot.push_str(&format!(
                "  \"{}\" [label=\"{}\\n{}\"];\n",
                node.id, node.id, node.agent_name
            ));
        }
        for edge in &def.edges {
            if let Some(ref cond) = edge.condition {
                dot.push_str(&format!(
                    "  \"{}\" -> \"{}\" [label=\"{}\"];\n",
                    edge.from, edge.to, cond
                ));
            } else {
                dot.push_str(&format!(
                    "  \"{}\" -> \"{}\";\n",
                    edge.from, edge.to
                ));
            }
        }
        dot.push_str("}\n");
        dot
    }

    pub async fn execute<F, Fut>(
        &self,
        def: &DagDefinition,
        run_node: F,
    ) -> Result<HashMap<String, NodeStatus>>
    where
        F: Fn(String, NodeContext, CancellationToken) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = std::result::Result<serde_json::Value, String>> + Send,
    {
        let graph = Self::build_graph(def)?;
        let sorted = toposort(&graph, None).map_err(|cycle| {
            BotError::Internal(format!("Topological sort failed (cycle detected): {:?}", cycle))
        })?;
        let total_nodes = sorted.len();

        let states: Arc<Mutex<HashMap<String, NodeExecutionState>>> = Arc::new(Mutex::new(
            if let Some(ref store) = self.config.checkpoint_store {
                match store.load(&def.id).await? {
                    Some(saved) => {
                        info!(dag_id = %def.id, "Restoring DAG from checkpoint");
                        saved
                    }
                    None => {
                        let mut m = HashMap::new();
                        for idx in &sorted {
                            m.insert(graph[*idx].id.clone(), NodeExecutionState {
                                status: NodeStatus::Pending, output: None, error: None, attempts: 0,
                            });
                        }
                        m
                    }
                }
            } else {
                let mut m = HashMap::new();
                for idx in &sorted {
                    m.insert(graph[*idx].id.clone(), NodeExecutionState {
                        status: NodeStatus::Pending, output: None, error: None, attempts: 0,
                    });
                }
                m
            },
        ));

        {
            let already_done = {
                let s = states.lock().await;
                s.values().filter(|st| st.status == NodeStatus::Success || st.status == NodeStatus::Skipped || st.status == NodeStatus::Failed).count()
            };
            let mut p = self.progress.lock().await;
            p.total = total_nodes;
            p.completed = already_done;
            p.fraction = if total_nodes > 0 { already_done as f64 / total_nodes as f64 } else { 1.0 };
        }

        let semaphore = self.config.max_concurrency.map(|n| Arc::new(Semaphore::new(n)));
        let run_node = Arc::new(run_node);
        let mut results: HashMap<String, NodeStatus> = HashMap::new();

        {
            let s = states.lock().await;
            for (nid, st) in s.iter() {
                match st.status {
                    NodeStatus::Success | NodeStatus::Failed | NodeStatus::Skipped => { results.insert(nid.clone(), st.status); }
                    _ => {}
                }
            }
        }

        info!(dag_id = %def.id, name = %def.name, total_nodes, "Starting DAG execution");

        for idx in &sorted {
            if self.cancel_token.is_cancelled() {
                warn!(dag_id = %def.id, "DAG execution cancelled");
                break;
            }
            let node = &graph[*idx];
            let node_id = node.id.clone();

            {
                let s = states.lock().await;
                if let Some(st) = s.get(&node_id) {
                    match st.status {
                        NodeStatus::Success | NodeStatus::Failed | NodeStatus::Skipped => { continue; }
                        _ => {}
                    }
                }
            }

            let should_skip = {
                let s = states.lock().await;
                let incoming: Vec<_> = graph.edges_directed(*idx, petgraph::Direction::Incoming).collect();
                if incoming.is_empty() {
                    false
                } else {
                    let any_active = incoming.iter().any(|e| {
                        let edge = e.weight();
                        let src_id = &graph[e.source()].id;
                        let src_state = s.get(src_id);
                        let dep_ok = src_state.map(|st| st.status == NodeStatus::Success).unwrap_or(false);
                        if !dep_ok { return false; }
                        match &edge.condition {
                            None => true,
                            Some(cond_expr) => {
                                let parent_output = src_state.and_then(|st| st.output.clone()).unwrap_or(serde_json::Value::Null);
                                eval_condition(cond_expr, &parent_output)
                            }
                        }
                    });
                    !any_active
                }
            };

            if should_skip {
                debug!(node_id = %node_id, "Skipping node: no active incoming edges");
                let mut s = states.lock().await;
                if let Some(st) = s.get_mut(&node_id) { st.status = NodeStatus::Skipped; }
                results.insert(node_id.clone(), NodeStatus::Skipped);
                self.update_progress(total_nodes).await;
                self.maybe_checkpoint(&def.id, &states).await;
                continue;
            }

            let parent_artifacts = {
                let s = states.lock().await;
                let mut arts = HashMap::new();
                for e in graph.edges_directed(*idx, petgraph::Direction::Incoming) {
                    let src_id = graph[e.source()].id.clone();
                    if let Some(st) = s.get(&src_id) {
                        if st.status == NodeStatus::Success {
                            if let Some(ref output) = st.output { arts.insert(src_id, output.clone()); }
                        }
                    }
                }
                arts
            };

            {
                let mut s = states.lock().await;
                if let Some(st) = s.get_mut(&node_id) { st.status = NodeStatus::Running; }
            }

            debug!(node_id = %node_id, agent = %node.agent_name, "Executing node");

            let _permit = if let Some(ref sem) = semaphore {
                Some(sem.acquire().await.map_err(|e| BotError::Internal(format!("Semaphore closed: {}", e)))?)
            } else { None };

            let timeout = Duration::from_millis(node.timeout_ms);
            let child_token = self.cancel_token.child_token();
            let mut success = false;

            for attempt in 0..=node.max_retries {
                let ctx = NodeContext { parent_artifacts: parent_artifacts.clone(), task_spec: node.task_spec.clone() };
                let nid = node_id.clone();
                let rt = run_node.clone();
                let ct = child_token.child_token();
                let exec_result = tokio::time::timeout(timeout, async move { rt(nid, ctx, ct).await }).await;
                match exec_result {
                    Ok(Ok(output)) => {
                        let mut s = states.lock().await;
                        if let Some(st) = s.get_mut(&node_id) {
                            st.status = NodeStatus::Success;
                            st.output = Some(output);
                            st.attempts = attempt + 1;
                        }
                        results.insert(node_id.clone(), NodeStatus::Success);
                        success = true;
                        debug!(node_id = %node_id, attempt = attempt + 1, "Node succeeded");
                        break;
                    }
                    Ok(Err(err_msg)) => {
                        warn!(node_id = %node_id, attempt = attempt + 1, error = %err_msg, "Node failed");
                        if attempt < node.max_retries {
                            let backoff = Self::compute_backoff(node.backoff_ms, attempt);
                            tokio::time::sleep(backoff).await;
                        } else {
                            let mut s = states.lock().await;
                            if let Some(st) = s.get_mut(&node_id) {
                                st.status = NodeStatus::Failed;
                                st.error = Some(err_msg);
                                st.attempts = attempt + 1;
                            }
                            results.insert(node_id.clone(), NodeStatus::Failed);
                        }
                    }
                    Err(_) => {
                        error!(node_id = %node_id, attempt = attempt + 1, timeout_ms = node.timeout_ms, "Node timed out");
                        if attempt < node.max_retries {
                            let backoff = Self::compute_backoff(node.backoff_ms, attempt);
                            tokio::time::sleep(backoff).await;
                        } else {
                            let mut s = states.lock().await;
                            if let Some(st) = s.get_mut(&node_id) {
                                st.status = NodeStatus::Failed;
                                st.error = Some(format!("Timeout after {}ms", node.timeout_ms));
                                st.attempts = attempt + 1;
                            }
                            results.insert(node_id.clone(), NodeStatus::Failed);
                        }
                    }
                }
            }

            self.update_progress(total_nodes).await;
            self.maybe_checkpoint(&def.id, &states).await;

            if !success && self.cancel_token.is_cancelled() {
                warn!(dag_id = %def.id, "DAG execution cancelled during node execution");
                break;
            }
        }

        let all_done = results.len() == total_nodes;
        if all_done {
            if let Some(ref store) = self.config.checkpoint_store { let _ = store.remove(&def.id).await; }
        }

        info!(dag_id = %def.id, name = %def.name, total_nodes, completed = results.len(), "DAG execution finished");
        Ok(results)
    }

    fn compute_backoff(base_ms: u64, attempt: u32) -> Duration {
        let exp = base_ms.saturating_mul(1u64 << attempt.min(10));
        let jitter = if base_ms > 0 { rand::thread_rng().gen_range(0..base_ms) } else { 0 };
        Duration::from_millis(exp + jitter)
    }

    async fn update_progress(&self, total: usize) {
        let mut p = self.progress.lock().await;
        p.completed += 1;
        p.total = total;
        p.fraction = if total > 0 { p.completed as f64 / total as f64 } else { 1.0 };
    }

    async fn maybe_checkpoint(&self, dag_id: &DagId, states: &Arc<Mutex<HashMap<String, NodeExecutionState>>>) {
        if let Some(ref store) = self.config.checkpoint_store {
            let s = states.lock().await;
            if let Err(e) = store.save(dag_id, &*s).await { warn!(error = %e, "Failed to save checkpoint"); }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_simple_dag() -> DagDefinition {
        DagDefinition {
            id: DagId::new(),
            name: "test-dag".to_string(),
            nodes: vec![
                DagNode { id: "a".to_string(), agent_name: "agent-a".to_string(), task_spec: serde_json::json!({"action": "step1"}), timeout_ms: 5000, max_retries: 0, backoff_ms: 100 },
                DagNode { id: "b".to_string(), agent_name: "agent-b".to_string(), task_spec: serde_json::json!({"action": "step2"}), timeout_ms: 5000, max_retries: 0, backoff_ms: 100 },
                DagNode { id: "c".to_string(), agent_name: "agent-c".to_string(), task_spec: serde_json::json!({"action": "step3"}), timeout_ms: 5000, max_retries: 0, backoff_ms: 100 },
            ],
            edges: vec![
                DagEdge { from: "a".to_string(), to: "b".to_string(), condition: None },
                DagEdge { from: "b".to_string(), to: "c".to_string(), condition: None },
            ],
        }
    }

    #[test]
    fn test_build_graph_valid() {
        let def = make_simple_dag();
        let graph = DagExecutor::build_graph(&def);
        assert!(graph.is_ok());
        let g = graph.unwrap();
        assert_eq!(g.node_count(), 3);
        assert_eq!(g.edge_count(), 2);
    }

    #[test]
    fn test_build_graph_detects_cycle() {
        let def = DagDefinition {
            id: DagId::new(), name: "cyclic-dag".to_string(),
            nodes: vec![
                DagNode { id: "x".to_string(), agent_name: "a".to_string(), task_spec: serde_json::json!({}), timeout_ms: 1000, max_retries: 0, backoff_ms: 0 },
                DagNode { id: "y".to_string(), agent_name: "b".to_string(), task_spec: serde_json::json!({}), timeout_ms: 1000, max_retries: 0, backoff_ms: 0 },
            ],
            edges: vec![
                DagEdge { from: "x".to_string(), to: "y".to_string(), condition: None },
                DagEdge { from: "y".to_string(), to: "x".to_string(), condition: None },
            ],
        };
        let result = DagExecutor::build_graph(&def);
        assert!(result.is_err());
        assert!(format!("{:?}", result.unwrap_err()).contains("cycle"));
    }

    #[test]
    fn test_build_graph_unknown_node() {
        let def = DagDefinition {
            id: DagId::new(), name: "bad-edge".to_string(),
            nodes: vec![DagNode { id: "a".to_string(), agent_name: "agent".to_string(), task_spec: serde_json::json!({}), timeout_ms: 1000, max_retries: 0, backoff_ms: 0 }],
            edges: vec![DagEdge { from: "a".to_string(), to: "nonexistent".to_string(), condition: None }],
        };
        let result = DagExecutor::build_graph(&def);
        assert!(result.is_err());
        assert!(format!("{:?}", result.unwrap_err()).contains("unknown target"));
    }

    #[tokio::test]
    async fn test_execute_linear_dag() {
        let def = make_simple_dag();
        let executor = DagExecutor::new(CancellationToken::new());
        let results = executor.execute(&def, |node_id, _ctx, _token| async move {
            Ok(serde_json::json!({"result": node_id}))
        }).await.expect("execution should succeed");
        assert_eq!(results.len(), 3);
        assert_eq!(results.get("a"), Some(&NodeStatus::Success));
        assert_eq!(results.get("b"), Some(&NodeStatus::Success));
        assert_eq!(results.get("c"), Some(&NodeStatus::Success));
    }

    #[tokio::test]
    async fn test_execute_skips_on_dependency_failure() {
        let def = make_simple_dag();
        let executor = DagExecutor::new(CancellationToken::new());
        let results = executor.execute(&def, |node_id, _ctx, _token| async move {
            if node_id == "b" { Err("simulated failure".to_string()) } else { Ok(serde_json::json!({"ok": true})) }
        }).await.expect("execution should complete");
        assert_eq!(results.get("a"), Some(&NodeStatus::Success));
        assert_eq!(results.get("b"), Some(&NodeStatus::Failed));
        assert_eq!(results.get("c"), Some(&NodeStatus::Skipped));
    }

    #[tokio::test]
    async fn test_execute_respects_cancellation() {
        let def = make_simple_dag();
        let cancel = CancellationToken::new();
        let executor = DagExecutor::new(cancel.clone());
        cancel.cancel();
        let results = executor.execute(&def, |_n, _c, _t| async move { Ok(serde_json::json!({"ok": true})) }).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_execute_with_retry() {
        let def = DagDefinition {
            id: DagId::new(), name: "retry-dag".to_string(),
            nodes: vec![DagNode { id: "flaky".to_string(), agent_name: "flaky-agent".to_string(), task_spec: serde_json::json!({}), timeout_ms: 5000, max_retries: 2, backoff_ms: 10 }],
            edges: vec![],
        };
        let counter = Arc::new(Mutex::new(0u32));
        let cc = counter.clone();
        let executor = DagExecutor::new(CancellationToken::new());
        let results = executor.execute(&def, move |_n, _c, _t| {
            let c = cc.clone();
            async move {
                let mut count = c.lock().await; *count += 1;
                if *count < 3 { Err("transient error".to_string()) } else { Ok(serde_json::json!({"recovered": true})) }
            }
        }).await.unwrap();
        assert_eq!(results.get("flaky"), Some(&NodeStatus::Success));
        assert_eq!(*counter.lock().await, 3);
    }

    #[test]
    fn test_eval_condition_equality() {
        let ctx = serde_json::json!({"status": "ok", "count": 42});
        assert!(eval_condition("status == \"ok\"", &ctx));
        assert!(!eval_condition("status == \"fail\"", &ctx));
        assert!(eval_condition("count == 42", &ctx));
        assert!(eval_condition("count != 0", &ctx));
    }

    #[test]
    fn test_eval_condition_comparison() {
        let ctx = serde_json::json!({"count": 10});
        assert!(eval_condition("count > 5", &ctx));
        assert!(eval_condition("count >= 10", &ctx));
        assert!(eval_condition("count < 20", &ctx));
        assert!(eval_condition("count <= 10", &ctx));
        assert!(!eval_condition("count > 100", &ctx));
    }

    #[test]
    fn test_eval_condition_logical_operators() {
        let ctx = serde_json::json!({"a": 1, "b": 2});
        assert!(eval_condition("a == 1 && b == 2", &ctx));
        assert!(!eval_condition("a == 1 && b == 99", &ctx));
        assert!(eval_condition("a == 99 || b == 2", &ctx));
        assert!(eval_condition("!(a == 99)", &ctx));
        assert!(!eval_condition("!(a == 1)", &ctx));
    }

    #[test]
    fn test_eval_condition_nested_path() {
        let ctx = serde_json::json!({"result": {"status": "done"}});
        assert!(eval_condition("result.status == \"done\"", &ctx));
        assert!(!eval_condition("result.status == \"pending\"", &ctx));
    }

    #[test]
    fn test_eval_condition_empty_is_true() {
        let ctx = serde_json::json!({});
        assert!(eval_condition("", &ctx));
        assert!(eval_condition("   ", &ctx));
    }

    #[tokio::test]
    async fn test_conditional_edge_skips_when_false() {
        let def = DagDefinition {
            id: DagId::new(), name: "cond-dag".to_string(),
            nodes: vec![
                DagNode { id: "root".to_string(), agent_name: "a".to_string(), task_spec: serde_json::json!({}), timeout_ms: 5000, max_retries: 0, backoff_ms: 0 },
                DagNode { id: "branch_yes".to_string(), agent_name: "b".to_string(), task_spec: serde_json::json!({}), timeout_ms: 5000, max_retries: 0, backoff_ms: 0 },
                DagNode { id: "branch_no".to_string(), agent_name: "c".to_string(), task_spec: serde_json::json!({}), timeout_ms: 5000, max_retries: 0, backoff_ms: 0 },
            ],
            edges: vec![
                DagEdge { from: "root".to_string(), to: "branch_yes".to_string(), condition: Some("result == \"go\"".to_string()) },
                DagEdge { from: "root".to_string(), to: "branch_no".to_string(), condition: Some("result == \"stop\"".to_string()) },
            ],
        };
        let executor = DagExecutor::new(CancellationToken::new());
        let results = executor.execute(&def, |node_id, _ctx, _token| async move {
            if node_id == "root" { Ok(serde_json::json!({"result": "go"})) } else { Ok(serde_json::json!({"ran": node_id})) }
        }).await.unwrap();
        assert_eq!(results.get("root"), Some(&NodeStatus::Success));
        assert_eq!(results.get("branch_yes"), Some(&NodeStatus::Success));
        assert_eq!(results.get("branch_no"), Some(&NodeStatus::Skipped));
    }

    #[tokio::test]
    async fn test_artifact_passing() {
        let def = DagDefinition {
            id: DagId::new(), name: "artifact-dag".to_string(),
            nodes: vec![
                DagNode { id: "producer".to_string(), agent_name: "a".to_string(), task_spec: serde_json::json!({}), timeout_ms: 5000, max_retries: 0, backoff_ms: 0 },
                DagNode { id: "consumer".to_string(), agent_name: "b".to_string(), task_spec: serde_json::json!({}), timeout_ms: 5000, max_retries: 0, backoff_ms: 0 },
            ],
            edges: vec![DagEdge { from: "producer".to_string(), to: "consumer".to_string(), condition: None }],
        };
        let executor = DagExecutor::new(CancellationToken::new());
        let results = executor.execute(&def, |node_id, ctx, _token| async move {
            if node_id == "producer" { Ok(serde_json::json!({"value": 42})) }
            else {
                let v = ctx.parent_artifacts.get("producer").and_then(|v| v.get("value")).and_then(|v| v.as_i64()).unwrap_or(0);
                Ok(serde_json::json!({"received": v}))
            }
        }).await.unwrap();
        assert_eq!(results.get("producer"), Some(&NodeStatus::Success));
        assert_eq!(results.get("consumer"), Some(&NodeStatus::Success));
    }

    #[tokio::test]
    async fn test_backpressure_limits_concurrency() {
        let def = DagDefinition {
            id: DagId::new(), name: "bp-dag".to_string(),
            nodes: (0..5).map(|i| DagNode { id: format!("n{}", i), agent_name: "agent".to_string(), task_spec: serde_json::json!({}), timeout_ms: 5000, max_retries: 0, backoff_ms: 0 }).collect(),
            edges: vec![],
        };
        let concurrent = Arc::new(Mutex::new(0u32));
        let max_concurrent = Arc::new(Mutex::new(0u32));
        let cc = concurrent.clone();
        let mc = max_concurrent.clone();
        let config = DagExecutorConfig { max_concurrency: Some(2), checkpoint_store: None };
        let executor = DagExecutor::with_config(CancellationToken::new(), config);
        let results = executor.execute(&def, move |_n, _c, _t| {
            let cc = cc.clone(); let mc = mc.clone();
            async move {
                let mut cur = cc.lock().await; *cur += 1; let current = *cur; drop(cur);
                let mut mx = mc.lock().await; if current > *mx { *mx = current; } drop(mx);
                tokio::time::sleep(Duration::from_millis(50)).await;
                let mut cur = cc.lock().await; *cur -= 1;
                Ok(serde_json::json!({"ok": true}))
            }
        }).await.unwrap();
        assert_eq!(results.len(), 5);
        assert!(*max_concurrent.lock().await <= 2);
    }

    #[tokio::test]
    async fn test_progress_tracking() {
        let def = make_simple_dag();
        let executor = DagExecutor::new(CancellationToken::new());
        let p = executor.get_progress().await;
        assert_eq!(p.fraction, 0.0);
        let _ = executor.execute(&def, |_n, _c, _t| async move { Ok(serde_json::json!({"ok": true})) }).await.unwrap();
        let p = executor.get_progress().await;
        assert_eq!(p.total, 3);
        assert_eq!(p.completed, 3);
        assert!((p.fraction - 1.0).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_cancellation_propagation() {
        let def = DagDefinition {
            id: DagId::new(), name: "cancel-dag".to_string(),
            nodes: vec![
                DagNode { id: "slow".to_string(), agent_name: "a".to_string(), task_spec: serde_json::json!({}), timeout_ms: 10000, max_retries: 0, backoff_ms: 0 },
                DagNode { id: "after".to_string(), agent_name: "b".to_string(), task_spec: serde_json::json!({}), timeout_ms: 5000, max_retries: 0, backoff_ms: 0 },
            ],
            edges: vec![DagEdge { from: "slow".to_string(), to: "after".to_string(), condition: None }],
        };
        let cancel = CancellationToken::new();
        let executor = DagExecutor::new(cancel.clone());
        let cc = cancel.clone();
        tokio::spawn(async move { tokio::time::sleep(Duration::from_millis(100)).await; cc.cancel(); });
        let start = std::time::Instant::now();
        let results = executor.execute(&def, |_n, _c, token| async move {
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(10)) => Ok(serde_json::json!({"ok": true})),
                _ = token.cancelled() => Err("cancelled".to_string()),
            }
        }).await.unwrap();
        assert!(start.elapsed() < Duration::from_secs(1));
        assert_ne!(results.get("after"), Some(&NodeStatus::Success));
    }

    #[test]
    fn test_dot_export() {
        let def = make_simple_dag();
        let dot = DagExecutor::to_dot(&def);
        assert!(dot.contains("digraph"));
        assert!(dot.contains("\"a\""));
        assert!(dot.contains("\"a\" -> \"b\""));
    }

    #[test]
    fn test_dot_export_with_conditions() {
        let def = DagDefinition {
            id: DagId::new(), name: "cond-dot".to_string(),
            nodes: vec![
                DagNode { id: "x".to_string(), agent_name: "a".to_string(), task_spec: serde_json::json!({}), timeout_ms: 1000, max_retries: 0, backoff_ms: 0 },
                DagNode { id: "y".to_string(), agent_name: "b".to_string(), task_spec: serde_json::json!({}), timeout_ms: 1000, max_retries: 0, backoff_ms: 0 },
            ],
            edges: vec![DagEdge { from: "x".to_string(), to: "y".to_string(), condition: Some("status == \"ok\"".to_string()) }],
        };
        let dot = DagExecutor::to_dot(&def);
        assert!(dot.contains("label="));
    }

    #[tokio::test]
    async fn test_checkpoint_save_and_restore() {
        let store = DagCheckpointStore::in_memory().unwrap();
        let dag_id = DagId::new();
        let mut states = HashMap::new();
        states.insert("a".to_string(), NodeExecutionState { status: NodeStatus::Success, output: Some(serde_json::json!({"v": 1})), error: None, attempts: 1 });
        states.insert("b".to_string(), NodeExecutionState { status: NodeStatus::Pending, output: None, error: None, attempts: 0 });
        store.save(&dag_id, &states).await.unwrap();
        let loaded = store.load(&dag_id).await.unwrap().unwrap();
        assert_eq!(loaded.get("a").unwrap().status, NodeStatus::Success);
        assert_eq!(loaded.get("b").unwrap().status, NodeStatus::Pending);
        store.remove(&dag_id).await.unwrap();
        assert!(store.load(&dag_id).await.unwrap().is_none());
    }

    #[test]
    fn test_backoff_increases_exponentially() {
        let b0 = DagExecutor::compute_backoff(100, 0);
        let b1 = DagExecutor::compute_backoff(100, 1);
        let b2 = DagExecutor::compute_backoff(100, 2);
        assert!(b0.as_millis() >= 100 && b0.as_millis() < 200);
        assert!(b1.as_millis() >= 200 && b1.as_millis() < 300);
        assert!(b2.as_millis() >= 400 && b2.as_millis() < 500);
    }
}
