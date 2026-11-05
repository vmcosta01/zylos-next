//! # zylos-resolver
//!
//! Resolver DAG com fronteira BFS, checkpoint/resume e paralelismo deterministico.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ResolverError {
    #[error("ciclo detectado no DAG: {0}")]
    CycleDetected(String),
    #[error("no nao encontrado: {0}")]
    NodeNotFound(String),
    #[error("checkpoint corrompido: {0}")]
    CheckpointCorrupted(String),
    #[error("prova falhou: {0}")]
    ProofFailed(String),
    #[error("limite atingido: {0}")]
    LimitReached(String),
    #[error("glue falhou entre {from} e {to}: {reason}")]
    GlueFailed { from: String, to: String, reason: String },
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeStatus { Pending, Running, Completed, Failed, Skipped }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagNode {
    pub id: String,
    pub description: String,
    pub dependencies: Vec<String>,
    pub status: NodeStatus,
    pub result_hash: Option<String>,
    pub depth: u32,
}

#[derive(Debug, Clone)]
pub struct BfsFrontier {
    queue: VecDeque<String>,
    visited: HashSet<String>,
}

impl BfsFrontier {
    pub fn new() -> Self { Self { queue: VecDeque::new(), visited: HashSet::new() } }
    pub fn push(&mut self, node_id: impl Into<String>) {
        let id = node_id.into();
        if self.visited.insert(id.clone()) { self.queue.push_back(id); }
    }
    pub fn next(&mut self) -> Option<String> { self.queue.pop_front() }
    pub fn is_empty(&self) -> bool { self.queue.is_empty() }
    pub fn len(&self) -> usize { self.queue.len() }
    pub fn snapshot(&self) -> Vec<String> { self.queue.iter().cloned().collect() }
    pub fn restore(snapshot: &[String]) -> Self {
        let mut f = Self::new();
        for id in snapshot { f.push(id.clone()); }
        f
    }
}
impl Default for BfsFrontier { fn default() -> Self { Self::new() } }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Corrida {
    pub desde: String,
    pub alvo: String,
    pub timestamp_inicio: String,
    pub timestamp_fim: Option<String>,
}
impl Corrida {
    pub fn nova(desde: impl Into<String>, alvo: impl Into<String>) -> Self {
        Self { desde: desde.into(), alvo: alvo.into(), timestamp_inicio: chrono::Utc::now().to_rfc3339(), timestamp_fim: None }
    }
    pub fn finalizar(&mut self) { self.timestamp_fim = Some(chrono::Utc::now().to_rfc3339()); }
    pub fn valida(&self) -> bool { !self.desde.is_empty() && !self.alvo.is_empty() && !self.timestamp_inicio.is_empty() }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolverCheckpoint {
    pub completed_nodes: HashSet<String>,
    pub failed_nodes: HashMap<String, String>,
    pub frontier_snapshot: Vec<String>,
    pub wave: u32,
    pub timestamp: String,
    pub chain_hash: String,
    pub corrida: Option<Corrida>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolverConfig {
    pub max_depth: u32,
    pub max_nodes: usize,
    pub max_parallel: Option<usize>,
    pub persist_path: Option<PathBuf>,
}
impl Default for ResolverConfig {
    fn default() -> Self { Self { max_depth: 3, max_nodes: 50, max_parallel: None, persist_path: None } }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveResult {
    pub completed: HashSet<String>,
    pub failed: HashMap<String, String>,
    pub truncated: bool,
    pub total_nodes_processed: usize,
    pub waves: u32,
    pub glue_proofs: Vec<GlueProof>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlueProof {
    pub from_node: String,
    pub to_node: String,
    pub claim_from: String,
    pub claim_to: String,
    pub hash: String,
    pub valid: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagGraph {
    pub nodes: HashMap<String, DagNode>,
    pub edges: Vec<(String, String)>,
}

impl DagGraph {
    pub fn new() -> Self { Self { nodes: HashMap::new(), edges: Vec::new() } }
    pub fn add_node(&mut self, node: DagNode) { self.nodes.insert(node.id.clone(), node); }
    pub fn add_edge(&mut self, from: impl Into<String>, to: impl Into<String>) {
        let f = from.into(); let t = to.into();
        self.edges.push((f.clone(), t.clone()));
        if let Some(node) = self.nodes.get_mut(&t) {
            if !node.dependencies.contains(&f) { node.dependencies.push(f); }
        }
    }
    pub fn has_cycle(&self) -> bool {
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();
        for node_id in self.nodes.keys() {
            if !visited.contains(node_id) && self.dfs_cycle(node_id, &mut visited, &mut rec_stack) { return true; }
        }
        false
    }
    fn dfs_cycle(&self, node: &str, visited: &mut HashSet<String>, rec_stack: &mut HashSet<String>) -> bool {
        visited.insert(node.to_string()); rec_stack.insert(node.to_string());
        if let Some(n) = self.nodes.get(node) {
            for dep in &n.dependencies {
                if !visited.contains(dep) { if self.dfs_cycle(dep, visited, rec_stack) { return true; } }
                else if rec_stack.contains(dep) { return true; }
            }
        }
        rec_stack.remove(node); false
    }
    pub fn ready_nodes(&self, completed: &HashSet<String>) -> Vec<String> {
        self.nodes.values().filter(|n| {
            !completed.contains(&n.id) && n.status == NodeStatus::Pending && n.dependencies.iter().all(|d| completed.contains(d))
        }).map(|n| n.id.clone()).collect()
    }
    pub fn compute_depths(&mut self) {
        let roots: Vec<String> = self.nodes.values().filter(|n| n.dependencies.is_empty()).map(|n| n.id.clone()).collect();
        let mut depths: HashMap<String, u32> = HashMap::new();
        let mut queue = VecDeque::new();
        for root in &roots { depths.insert(root.clone(), 0); queue.push_back(root.clone()); }
        while let Some(nid) = queue.pop_front() {
            let cd = *depths.get(&nid).unwrap_or(&0);
            for (from, to) in &self.edges {
                if from == &nid {
                    let child = cd + 1;
                    if !depths.contains_key(to) || depths[to] > child { depths.insert(to.clone(), child); queue.push_back(to.clone()); }
                }
            }
        }
        for (id, depth) in depths { if let Some(node) = self.nodes.get_mut(&id) { node.depth = depth; } }
    }
}
impl Default for DagGraph { fn default() -> Self { Self::new() } }

pub fn save_checkpoint(path: &Path, checkpoint: &ResolverCheckpoint) -> Result<(), ResolverError> {
    let tmp = path.with_extension("tmp");
    let json = serde_json::to_string_pretty(checkpoint).map_err(|e| ResolverError::CheckpointCorrupted(e.to_string()))?;
    std::fs::write(&tmp, json.as_bytes())?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}
pub fn load_checkpoint(path: &Path) -> Option<ResolverCheckpoint> {
    match std::fs::read_to_string(path) { Ok(c) => serde_json::from_str(&c).ok(), Err(_) => None }
}
pub fn clear_checkpoint(path: &Path) { let _ = std::fs::remove_file(path); }

pub fn glue(from_node: &DagNode, to_node: &DagNode, claim_from: &str, claim_to: &str) -> Result<GlueProof, ResolverError> {
    if from_node.status != NodeStatus::Completed {
        return Err(ResolverError::GlueFailed { from: from_node.id.clone(), to: to_node.id.clone(),
            reason: format!("no fonte '{}' nao completado (status={:?})", from_node.id, from_node.status) });
    }
    let mut h = Sha256::new();
    h.update(claim_from.as_bytes()); h.update(b"|"); h.update(claim_to.as_bytes()); h.update(b"|");
    h.update(from_node.result_hash.as_deref().unwrap_or("").as_bytes()); h.update(b"|");
    h.update(to_node.result_hash.as_deref().unwrap_or("").as_bytes());
    Ok(GlueProof { from_node: from_node.id.clone(), to_node: to_node.id.clone(),
        claim_from: claim_from.to_string(), claim_to: claim_to.to_string(),
        hash: format!("{:x}", h.finalize()), valid: true })
}

pub fn resolve<F>(dag: &mut DagGraph, config: &ResolverConfig, mut executor: F) -> Result<ResolveResult, ResolverError>
where F: FnMut(&str, &DagNode) -> Result<String, String> {
    if dag.has_cycle() { return Err(ResolverError::CycleDetected("DAG contem ciclo".into())); }
    dag.compute_depths();
    let mut completed: HashSet<String> = HashSet::new();
    let mut failed: HashMap<String, String> = HashMap::new();
    let mut frontier = BfsFrontier::new();
    let mut wave: u32 = 0;
    let mut corrida: Option<Corrida> = None;
    if let Some(ref pp) = config.persist_path {
        if let Some(cp) = load_checkpoint(pp) {
            if !cp.frontier_snapshot.is_empty() {
                completed = cp.completed_nodes; failed = cp.failed_nodes;
                frontier = BfsFrontier::restore(&cp.frontier_snapshot); wave = cp.wave; corrida = cp.corrida;
            }
        }
    }
    if frontier.is_empty() && completed.is_empty() {
        for nid in dag.ready_nodes(&completed) { frontier.push(nid); }
        corrida = Some(Corrida::nova("root", "target"));
    }
    let mut total_processed = 0usize;
    let mut truncated = false;
    let glue_proofs = Vec::new();
    while !frontier.is_empty() {
        wave += 1;
        let bs = config.max_parallel.unwrap_or(frontier.len());
        let mut batch = Vec::with_capacity(bs);
        while batch.len() < bs { if let Some(nid) = frontier.next() { batch.push(nid); } else { break; } }
        for nid in &batch {
            if total_processed >= config.max_nodes { truncated = true; break; }
            if let Some(node) = dag.nodes.get(nid) { if node.depth > config.max_depth { truncated = true; continue; } }
            if let Some(node) = dag.nodes.get_mut(nid) { node.status = NodeStatus::Running; }
            match executor(nid, dag.nodes.get(nid).unwrap()) {
                Ok(hash) => {
                    if let Some(node) = dag.nodes.get_mut(nid) { node.status = NodeStatus::Completed; node.result_hash = Some(hash); }
                    completed.insert(nid.clone()); total_processed += 1;
                    for rid in dag.ready_nodes(&completed) { frontier.push(rid); }
                }
                Err(reason) => {
                    if let Some(node) = dag.nodes.get_mut(nid) { node.status = NodeStatus::Failed; }
                    failed.insert(nid.clone(), reason); total_processed += 1;
                }
            }
        }
        if truncated { break; }
        if let Some(ref pp) = config.persist_path {
            let mut h = Sha256::new();
            for id in &completed { h.update(id.as_bytes()); }
            let cp = ResolverCheckpoint { completed_nodes: completed.clone(), failed_nodes: failed.clone(),
                frontier_snapshot: frontier.snapshot(), wave, timestamp: chrono::Utc::now().to_rfc3339(),
                chain_hash: format!("{:x}", h.finalize()), corrida: corrida.clone() };
            let _ = save_checkpoint(pp, &cp);
        }
    }
    if let Some(ref mut c) = corrida { c.finalizar(); }
    if !truncated && frontier.is_empty() { if let Some(ref pp) = config.persist_path { clear_checkpoint(pp); } }
    Ok(ResolveResult { completed, failed, truncated, total_nodes_processed: total_processed, waves: wave, glue_proofs })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn make_node(id: &str, deps: Vec<&str>) -> DagNode {
        DagNode { id: id.into(), description: format!("node {}", id), dependencies: deps.into_iter().map(Into::into).collect(),
            status: NodeStatus::Pending, result_hash: None, depth: 0 }
    }
    #[test] fn test_bfs_frontier_dedup() {
        let mut f = BfsFrontier::new(); f.push("a"); f.push("b"); f.push("a");
        assert_eq!(f.len(), 2); assert_eq!(f.next(), Some("a".into())); assert_eq!(f.next(), Some("b".into())); assert!(f.is_empty());
    }
    #[test] fn test_bfs_frontier_snapshot_restore() {
        let mut f = BfsFrontier::new(); f.push("x"); f.push("y"); f.push("z");
        let snap = f.snapshot(); assert_eq!(snap, vec!["x","y","z"]);
        let mut r = BfsFrontier::restore(&snap); assert_eq!(r.len(), 3); assert_eq!(r.next(), Some("x".into()));
    }
    #[test] fn test_dag_no_cycle() {
        let mut d = DagGraph::new(); d.add_node(make_node("a",vec![])); d.add_node(make_node("b",vec!["a"])); d.add_edge("a","b");
        assert!(!d.has_cycle());
    }
    #[test] fn test_dag_cycle_detected() {
        let mut d = DagGraph::new(); d.add_node(make_node("a",vec!["b"])); d.add_node(make_node("b",vec!["a"]));
        d.add_edge("b","a"); d.add_edge("a","b"); assert!(d.has_cycle());
    }
    #[test] fn test_ready_nodes() {
        let mut d = DagGraph::new(); d.add_node(make_node("a",vec![])); d.add_node(make_node("b",vec!["a"]));
        let c = HashSet::new();
        let ready_a = d.ready_nodes(&c); let r: HashSet<&str> = ready_a.iter().map(|s| s.as_str()).collect(); assert_eq!(r, HashSet::from(["a"]));
                let mut c2 = HashSet::new(); c2.insert("a".to_string());
                let ready_b = d.ready_nodes(&c2); let r2: HashSet<&str> = ready_b.iter().map(|s| s.as_str()).collect(); assert_eq!(r2, HashSet::from(["b"]));
    }
    #[test] fn test_resolve_empty_frontier() {
        let mut d = DagGraph::new(); let r = resolve(&mut d, &ResolverConfig::default(), |_,_| Ok("h".into())).unwrap();
        assert!(r.completed.is_empty()); assert_eq!(r.total_nodes_processed, 0);
    }
    #[test] fn test_resolve_simple_chain() {
        let mut d = DagGraph::new(); d.add_node(make_node("a",vec![])); d.add_node(make_node("b",vec!["a"])); d.add_node(make_node("c",vec!["b"]));
        d.add_edge("a","b"); d.add_edge("b","c");
        let r = resolve(&mut d, &ResolverConfig::default(), |id,_| Ok(format!("hash_{}",id))).unwrap();
        assert_eq!(r.completed.len(), 3); assert!(!r.truncated);
    }
    #[test] fn test_resolve_max_nodes_limit() {
        let mut d = DagGraph::new();
        let ids: Vec<String> = (0..10).map(|i| format!("n{}",i)).collect();
        for i in 0..10 { let deps: Vec<&str> = if i>0 { vec![ids[i-1].as_str()] } else { vec![] }; d.add_node(make_node(&ids[i],deps)); if i>0 { d.add_edge(&ids[i-1],&ids[i]); } }
        let r = resolve(&mut d, &ResolverConfig{max_nodes:5,..Default::default()}, |id,_| Ok(format!("h_{}",id))).unwrap();
                assert!(r.truncated); assert_eq!(r.completed.len(), 4);
    }
    #[test] fn test_resolve_max_depth_limit() {
        let mut d = DagGraph::new(); d.add_node(make_node("a",vec![])); d.add_node(make_node("b",vec!["a"]));
        d.add_node(make_node("c",vec!["b"])); d.add_node(make_node("d",vec!["c"]));
        d.add_edge("a","b"); d.add_edge("b","c"); d.add_edge("c","d");
        let r = resolve(&mut d, &ResolverConfig{max_depth:2,max_nodes:100,..Default::default()}, |id,_| Ok(format!("h_{}",id))).unwrap();
        assert!(r.truncated); assert!(r.completed.contains("a")); assert!(r.completed.contains("b")); assert!(r.completed.contains("c")); assert!(!r.completed.contains("d"));
    }
    #[test] fn test_resolve_with_failure() {
        let mut d = DagGraph::new(); d.add_node(make_node("a",vec![])); d.add_node(make_node("b",vec!["a"])); d.add_edge("a","b");
        let r = resolve(&mut d, &ResolverConfig::default(), |id,_| if id=="a" { Err("boom".into()) } else { Ok(format!("h_{}",id)) }).unwrap();
        assert!(r.failed.contains_key("a")); assert!(!r.completed.contains("b"));
    }
    #[test] fn test_checkpoint_save_load() {
        let dir = tempfile::tempdir().unwrap(); let p = dir.path().join("cp.json");
        let cp = ResolverCheckpoint { completed_nodes: ["a".into()].into_iter().collect(), failed_nodes: HashMap::new(),
            frontier_snapshot: vec!["b".into()], wave: 1, timestamp: "2026-11-03T00:00:00Z".into(), chain_hash: "abc".into(), corrida: None };
        save_checkpoint(&p, &cp).unwrap(); let l = load_checkpoint(&p).unwrap(); assert_eq!(l.chain_hash, "abc"); assert!(l.completed_nodes.contains("a"));
    }
    #[test] fn test_checkpoint_corrupted_returns_none() {
        let dir = tempfile::tempdir().unwrap(); let p = dir.path().join("bad.json");
        std::fs::write(&p, b"not valid json{{{").unwrap(); assert!(load_checkpoint(&p).is_none());
    }
    #[test] fn test_checkpoint_missing_returns_none() { assert!(load_checkpoint(&PathBuf::from("/nonexistent/cp.json")).is_none()); }
    #[test] fn test_clear_checkpoint() {
        let dir = tempfile::tempdir().unwrap(); let p = dir.path().join("cp.json");
        std::fs::write(&p, b"data").unwrap(); assert!(p.exists()); clear_checkpoint(&p); assert!(!p.exists());
    }
    #[test] fn test_glue_success() {
        let f = DagNode{id:"a".into(),description:"".into(),dependencies:vec![],status:NodeStatus::Completed,result_hash:Some("ha".into()),depth:0};
        let t = DagNode{id:"b".into(),description:"".into(),dependencies:vec!["a".into()],status:NodeStatus::Pending,result_hash:None,depth:1};
        let p = glue(&f,&t,"ca","cb").unwrap(); assert!(p.valid); assert!(!p.hash.is_empty());
    }
    #[test] fn test_glue_fails_incomplete_source() {
        let f = DagNode{id:"a".into(),description:"".into(),dependencies:vec![],status:NodeStatus::Pending,result_hash:None,depth:0};
        let t = DagNode{id:"b".into(),description:"".into(),dependencies:vec!["a".into()],status:NodeStatus::Pending,result_hash:None,depth:1};
        assert!(glue(&f,&t,"ca","cb").is_err());
    }
    #[test] fn test_resume_valid_checkpoint() {
        let dir = tempfile::tempdir().unwrap(); let p = dir.path().join("resume.json");
        let cp = ResolverCheckpoint { completed_nodes: ["a".into()].into_iter().collect(), failed_nodes: HashMap::new(),
            frontier_snapshot: vec!["b".into()], wave: 1, timestamp: "2026-11-03T00:00:00Z".into(), chain_hash: "prev".into(), corrida: None };
        save_checkpoint(&p, &cp).unwrap();
        let mut d = DagGraph::new(); d.add_node(make_node("a",vec![])); d.add_node(make_node("b",vec!["a"])); d.add_edge("a","b");
        let r = resolve(&mut d, &ResolverConfig{persist_path:Some(p.clone()),..Default::default()}, |id,_| Ok(format!("h_{}",id))).unwrap();
        assert!(r.completed.contains("a")); assert!(r.completed.contains("b")); assert!(!p.exists());
    }
    #[test] fn test_resume_empty_frontier_restarts() {
        let dir = tempfile::tempdir().unwrap(); let p = dir.path().join("empty.json");
        let cp = ResolverCheckpoint { completed_nodes: HashSet::new(), failed_nodes: HashMap::new(),
            frontier_snapshot: vec![], wave: 0, timestamp: "2026-11-03T00:00:00Z".into(), chain_hash: "empty".into(), corrida: None };
        save_checkpoint(&p, &cp).unwrap();
        let mut d = DagGraph::new(); d.add_node(make_node("a",vec![]));
        let r = resolve(&mut d, &ResolverConfig{persist_path:Some(p.clone()),..Default::default()}, |id,_| Ok(format!("h_{}",id))).unwrap();
        assert!(r.completed.contains("a"));
    }
    #[test] fn test_corrida_valida() { let c = Corrida::nova("i","f"); assert!(c.valida()); assert!(c.timestamp_fim.is_none()); }
    #[test] fn test_corrida_finalizar() { let mut c = Corrida::nova("a","b"); c.finalizar(); assert!(c.timestamp_fim.is_some()); }
    #[test] fn test_compute_depths() {
        let mut d = DagGraph::new(); d.add_node(make_node("r",vec![])); d.add_node(make_node("m",vec!["r"])); d.add_node(make_node("l",vec!["m"]));
        d.add_edge("r","m"); d.add_edge("m","l"); d.compute_depths();
        assert_eq!(d.nodes["r"].depth, 0); assert_eq!(d.nodes["m"].depth, 1); assert_eq!(d.nodes["l"].depth, 2);
    }
    #[test] fn test_checkpoint_serialization() {
        let cp = ResolverCheckpoint { completed_nodes: ["a".into()].into_iter().collect(), failed_nodes: HashMap::new(),
            frontier_snapshot: vec!["b".into()], wave: 2, timestamp: "2026-11-03T00:00:00Z".into(), chain_hash: "abc".into(), corrida: Some(Corrida::nova("x","y")) };
        let j = serde_json::to_string(&cp).unwrap(); let de: ResolverCheckpoint = serde_json::from_str(&j).unwrap();
        assert_eq!(de.chain_hash, "abc"); assert!(de.corrida.is_some());
    }
}
