//! # SkillEvolutionGraph — Grafo de evolução de skills (T8.4.2)
//!
//! Modela relações entre skills como grafo direcionado com arestas tipadas:
//! - **Extends**: skill B estende skill A (herda precondições/outputs)
//! - **Replaces**: skill B substitui skill A (A torna-se deprecated)
//! - **Repairs**: skill B corrige falha conhecida em skill A
//! - **DependsOn**: skill B requer skill A como precondição
//!
//! ## Operações
//! - `merge(a, b)`: combina duas skills redundantes em uma nova
//! - `retire(skill_id)`: marca skill como deprecated sem deletar
//! - `add_validator(skill_id, validator)`: adiciona validator a skill existente
//!
//! ## Persistência
//! O grafo é serializável via serde para storage em SQLite ou arquivo JSON.

use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::contract::{SkillContract, ValidatorRef};

/// Tipo de relação entre duas skills no grafo de evolução.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvolutionEdge {
    /// Skill destino estende skill origem.
    Extends,
    /// Skill destino substitui skill origem.
    Replaces,
    /// Skill destino corrige skill origem.
    Repairs,
    /// Skill destino depende de skill origem.
    DependsOn,
}

impl std::fmt::Display for EvolutionEdge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Extends => write!(f, "extends"),
            Self::Replaces => write!(f, "replaces"),
            Self::Repairs => write!(f, "repairs"),
            Self::DependsOn => write!(f, "depends_on"),
        }
    }
}

/// Estado de uma skill no grafo de evolução.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillStatus {
    /// Skill ativa e utilizável.
    Active,
    /// Skill deprecated (substituída ou reparada).
    Deprecated { reason: String, replaced_by: Option<String> },
    /// Skill removida do grafo.
    Retired,
}

/// Nó do grafo de evolução representando uma skill.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillNode {
    /// ID da skill.
    pub skill_id: String,
    /// Nome da skill.
    pub name: String,
    /// Contrato formal da skill (opcional).
    pub contract: Option<SkillContract>,
    /// Estado atual da skill.
    pub status: SkillStatus,
    /// Versão da skill.
    pub version: String,
}

/// Resultado de uma operação de merge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeResult {
    /// ID da nova skill criada pelo merge.
    pub merged_skill_id: String,
    /// IDs das skills originais que foram combinadas.
    pub source_ids: Vec<String>,
    /// Se as skills originais foram marcadas como deprecated.
    pub sources_deprecated: bool,
}

/// Grafo de evolução de skills.
#[derive(Clone)]
pub struct SkillEvolutionGraph {
    graph: DiGraph<SkillNode, EvolutionEdge>,
    /// Index: skill_id → NodeIndex para lookup O(1).
    node_map: HashMap<String, NodeIndex>,
}

impl Default for SkillEvolutionGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl SkillEvolutionGraph {
    /// Cria um grafo vazio.
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            node_map: HashMap::new(),
        }
    }

    /// Adiciona uma skill ao grafo.
    pub fn add_skill(&mut self, node: SkillNode) -> NodeIndex {
        if let Some(&idx) = self.node_map.get(&node.skill_id) {
            // Atualizar nó existente
            self.graph[idx] = node;
            idx
        } else {
            let idx = self.graph.add_node(node.clone());
            self.node_map.insert(node.skill_id, idx);
            idx
        }
    }

    /// Adiciona uma relação entre duas skills.
    pub fn add_relation(
        &mut self,
        from_skill_id: &str,
        to_skill_id: &str,
        edge: EvolutionEdge,
    ) -> Result<(), EvolutionError> {
        let from_idx = self
            .node_map
            .get(from_skill_id)
            .copied()
            .ok_or_else(|| EvolutionError::SkillNotFound(from_skill_id.to_string()))?;
        let to_idx = self
            .node_map
            .get(to_skill_id)
            .copied()
            .ok_or_else(|| EvolutionError::SkillNotFound(to_skill_id.to_string()))?;

        self.graph.add_edge(from_idx, to_idx, edge);
        Ok(())
    }

    /// Busca uma skill por ID.
    pub fn get_skill(&self, skill_id: &str) -> Option<&SkillNode> {
        self.node_map
            .get(skill_id)
            .map(|&idx| &self.graph[idx])
    }

    /// Retorna todas as skills ativas.
    pub fn active_skills(&self) -> Vec<&SkillNode> {
        self.graph
            .node_weights()
            .filter(|n| matches!(n.status, SkillStatus::Active))
            .collect()
    }

    /// Retorna todas as skills deprecated.
    pub fn deprecated_skills(&self) -> Vec<&SkillNode> {
        self.graph
            .node_weights()
            .filter(|n| matches!(n.status, SkillStatus::Deprecated { .. }))
            .collect()
    }

    /// Marca uma skill como retired (deprecated permanente).
    pub fn retire(&mut self, skill_id: &str) -> Result<(), EvolutionError> {
        let idx = self
            .node_map
            .get(skill_id)
            .copied()
            .ok_or_else(|| EvolutionError::SkillNotFound(skill_id.to_string()))?;

        self.graph[idx].status = SkillStatus::Retired;
        Ok(())
    }

    /// Marca uma skill como deprecated com razão e substituto opcional.
    pub fn deprecate(
        &mut self,
        skill_id: &str,
        reason: impl Into<String>,
        replaced_by: Option<String>,
    ) -> Result<(), EvolutionError> {
        let idx = self
            .node_map
            .get(skill_id)
            .copied()
            .ok_or_else(|| EvolutionError::SkillNotFound(skill_id.to_string()))?;

        self.graph[idx].status = SkillStatus::Deprecated {
            reason: reason.into(),
            replaced_by,
        };
        Ok(())
    }

    /// Adiciona um validator ao contrato de uma skill.
    pub fn add_validator(
        &mut self,
        skill_id: &str,
        validator: ValidatorRef,
    ) -> Result<(), EvolutionError> {
        let idx = self
            .node_map
            .get(skill_id)
            .copied()
            .ok_or_else(|| EvolutionError::SkillNotFound(skill_id.to_string()))?;

        let contract = self.graph[idx]
            .contract
            .get_or_insert_with(SkillContract::default);
        contract.validators.push(validator);

        Ok(())
    }

    /// Combina duas skills redundantes em uma nova skill.
    ///
    /// A nova skill herda contratos de ambas as fontes.
    /// As skills originais são marcadas como deprecated.
    pub fn merge(
        &mut self,
        skill_a_id: &str,
        skill_b_id: &str,
        merged_name: impl Into<String>,
    ) -> Result<MergeResult, EvolutionError> {
        let node_a = self
            .get_skill(skill_a_id)
            .ok_or_else(|| EvolutionError::SkillNotFound(skill_a_id.to_string()))?
            .clone();
        let node_b = self
            .get_skill(skill_b_id)
            .ok_or_else(|| EvolutionError::SkillNotFound(skill_b_id.to_string()))?
            .clone();

        // Criar contrato mergeado
        let mut merged_contract = SkillContract::default();
        if let Some(ref ca) = node_a.contract {
            merged_contract.preconditions.extend(ca.preconditions.iter().cloned());
            merged_contract.outputs.extend(ca.outputs.iter().cloned());
            merged_contract.assertions.extend(ca.assertions.iter().cloned());
            merged_contract.validators.extend(ca.validators.iter().cloned());
            merged_contract.fitness.extend(ca.fitness.iter().cloned());
        }
        if let Some(ref cb) = node_b.contract {
            merged_contract.preconditions.extend(cb.preconditions.iter().cloned());
            merged_contract.outputs.extend(cb.outputs.iter().cloned());
            merged_contract.assertions.extend(cb.assertions.iter().cloned());
            merged_contract.validators.extend(cb.validators.iter().cloned());
            merged_contract.fitness.extend(cb.fitness.iter().cloned());
        }

        let merged_id = format!("merged-{}-{}", skill_a_id, skill_b_id);
        let merged_node = SkillNode {
            skill_id: merged_id.clone(),
            name: merged_name.into(),
            contract: Some(merged_contract),
            status: SkillStatus::Active,
            version: "1.0".into(),
        };

        self.add_skill(merged_node);

        // Adicionar arestas Replaces
        self.add_relation(skill_a_id, &merged_id, EvolutionEdge::Replaces)?;
        self.add_relation(skill_b_id, &merged_id, EvolutionEdge::Replaces)?;

        // Deprecar originais
        self.deprecate(
            skill_a_id,
            format!("Merged into {}", merged_id),
            Some(merged_id.clone()),
        )?;
        self.deprecate(
            skill_b_id,
            format!("Merged into {}", merged_id),
            Some(merged_id.clone()),
        )?;

        Ok(MergeResult {
            merged_skill_id: merged_id,
            source_ids: vec![skill_a_id.to_string(), skill_b_id.to_string()],
            sources_deprecated: true,
        })
    }

    /// Retorna os sucessores diretos de uma skill (skills que ela aponta).
    pub fn successors(&self, skill_id: &str) -> Result<Vec<&SkillNode>, EvolutionError> {
        let idx = self
            .node_map
            .get(skill_id)
            .copied()
            .ok_or_else(|| EvolutionError::SkillNotFound(skill_id.to_string()))?;

        Ok(self
            .graph
            .neighbors_directed(idx, petgraph::Direction::Outgoing)
            .map(|n| &self.graph[n])
            .collect())
    }

    /// Retorna os predecessores diretos de uma skill (skills que apontam para ela).
    pub fn predecessors(&self, skill_id: &str) -> Result<Vec<&SkillNode>, EvolutionError> {
        let idx = self
            .node_map
            .get(skill_id)
            .copied()
            .ok_or_else(|| EvolutionError::SkillNotFound(skill_id.to_string()))?;

        Ok(self
            .graph
            .neighbors_directed(idx, petgraph::Direction::Incoming)
            .map(|n| &self.graph[n])
            .collect())
    }

    /// Retorna o número total de skills no grafo.
    pub fn skill_count(&self) -> usize {
        self.graph.node_count()
    }

    /// Retorna o número total de relações no grafo.
    pub fn relation_count(&self) -> usize {
        self.graph.edge_count()
    }

    /// Serializa o grafo para JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        let nodes: Vec<&SkillNode> = self.graph.node_weights().collect();
        let edges: Vec<(String, String, EvolutionEdge)> = self
            .graph
            .edge_references()
            .map(|e| {
                let from = &self.graph[e.source()].skill_id;
                let to = &self.graph[e.target()].skill_id;
                (from.clone(), to.clone(), *e.weight())
            })
            .collect();

        let data = serde_json::json!({
            "nodes": nodes,
            "edges": edges,
        });
        serde_json::to_string_pretty(&data)
    }

    /// Verifica se há ciclos no grafo (dependências circulares).
    pub fn has_cycles(&self) -> bool {
        petgraph::algo::is_cyclic_directed(&self.graph)
    }

    /// Retorna estatísticas do grafo.
    pub fn stats(&self) -> GraphStats {
        let active = self.active_skills().len();
        let deprecated = self.deprecated_skills().len();
        let retired = self
            .graph
            .node_weights()
            .filter(|n| matches!(n.status, SkillStatus::Retired))
            .count();

        GraphStats {
            total_skills: self.skill_count(),
            active_skills: active,
            deprecated_skills: deprecated,
            retired_skills: retired,
            total_relations: self.relation_count(),
            has_cycles: self.has_cycles(),
        }
    }
}

/// Estatísticas do grafo de evolução.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphStats {
    pub total_skills: usize,
    pub active_skills: usize,
    pub deprecated_skills: usize,
    pub retired_skills: usize,
    pub total_relations: usize,
    pub has_cycles: bool,
}

/// Erros do grafo de evolução.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EvolutionError {
    #[error("Skill não encontrada: {0}")]
    SkillNotFound(String),
    #[error("Operação inválida: {0}")]
    InvalidOperation(String),
    #[error("Erro de serialização: {0}")]
    SerializationError(String),
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::{OutputSchema, Predicate};

    fn make_node(id: &str, name: &str) -> SkillNode {
        SkillNode {
            skill_id: id.into(),
            name: name.into(),
            contract: None,
            status: SkillStatus::Active,
            version: "1.0".into(),
        }
    }

    fn make_node_with_contract(id: &str, name: &str) -> SkillNode {
        SkillNode {
            skill_id: id.into(),
            name: name.into(),
            contract: Some(
                SkillContract::new()
                    .with_precondition(Predicate::new("rust_installed", vec![]))
                    .with_output(OutputSchema::new("result", "object")),
            ),
            status: SkillStatus::Active,
            version: "1.0".into(),
        }
    }

    #[test]
    fn test_add_and_get_skill() {
        let mut graph = SkillEvolutionGraph::new();
        let node = make_node("skill-1", "Test Skill");
        graph.add_skill(node);

        let retrieved = graph.get_skill("skill-1");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "Test Skill");
        assert_eq!(graph.skill_count(), 1);
    }

    #[test]
    fn test_add_relation() {
        let mut graph = SkillEvolutionGraph::new();
        graph.add_skill(make_node("a", "Skill A"));
        graph.add_skill(make_node("b", "Skill B"));

        assert!(graph
            .add_relation("a", "b", EvolutionEdge::Extends)
            .is_ok());
        assert_eq!(graph.relation_count(), 1);
    }

    #[test]
    fn test_add_relation_missing_skill() {
        let mut graph = SkillEvolutionGraph::new();
        graph.add_skill(make_node("a", "Skill A"));

        let result = graph.add_relation("a", "nonexistent", EvolutionEdge::Extends);
        assert!(matches!(result, Err(EvolutionError::SkillNotFound(_))));
    }

    #[test]
    fn test_retire_skill() {
        let mut graph = SkillEvolutionGraph::new();
        graph.add_skill(make_node("s1", "To Retire"));

        graph.retire("s1").unwrap();
        let node = graph.get_skill("s1").unwrap();
        assert_eq!(node.status, SkillStatus::Retired);
    }

    #[test]
    fn test_deprecate_skill() {
        let mut graph = SkillEvolutionGraph::new();
        graph.add_skill(make_node("old", "Old Skill"));
        graph.add_skill(make_node("new", "New Skill"));

        graph
            .deprecate("old", "Replaced by new", Some("new".into()))
            .unwrap();

        let node = graph.get_skill("old").unwrap();
        match &node.status {
            SkillStatus::Deprecated {
                reason,
                replaced_by,
            } => {
                assert_eq!(reason, "Replaced by new");
                assert_eq!(replaced_by, &Some("new".into()));
            }
            _ => panic!("Expected Deprecated status"),
        }
    }

    #[test]
    fn test_add_validator() {
        let mut graph = SkillEvolutionGraph::new();
        graph.add_skill(make_node_with_contract("s1", "With Contract"));

        graph
            .add_validator("s1", ValidatorRef::new("check_output_size"))
            .unwrap();

        let node = graph.get_skill("s1").unwrap();
        assert_eq!(node.contract.as_ref().unwrap().validators.len(), 1);
    }

    #[test]
    fn test_add_validator_creates_contract_if_missing() {
        let mut graph = SkillEvolutionGraph::new();
        graph.add_skill(make_node("s1", "No Contract"));

        graph
            .add_validator("s1", ValidatorRef::new("basic_check"))
            .unwrap();

        let node = graph.get_skill("s1").unwrap();
        assert!(node.contract.is_some());
        assert_eq!(node.contract.as_ref().unwrap().validators.len(), 1);
    }

    #[test]
    fn test_merge_skills() {
        let mut graph = SkillEvolutionGraph::new();
        graph.add_skill(make_node_with_contract("a", "Skill A"));
        graph.add_skill(make_node_with_contract("b", "Skill B"));

        let result = graph.merge("a", "b", "Merged AB").unwrap();

        assert!(result.merged_skill_id.starts_with("merged-"));
        assert_eq!(result.source_ids.len(), 2);
        assert!(result.sources_deprecated);

        // Merged skill deve existir e estar ativa
        let merged = graph.get_skill(&result.merged_skill_id).unwrap();
        assert_eq!(merged.name, "Merged AB");
        assert_eq!(merged.status, SkillStatus::Active);
        // Deve ter contrato com elementos de ambos
        assert!(merged.contract.is_some());
        assert!(merged.contract.as_ref().unwrap().preconditions.len() >= 2);

        // Originais devem estar deprecated
        let a = graph.get_skill("a").unwrap();
        assert!(matches!(a.status, SkillStatus::Deprecated { .. }));
        let b = graph.get_skill("b").unwrap();
        assert!(matches!(b.status, SkillStatus::Deprecated { .. }));
    }

    #[test]
    fn test_successors_and_predecessors() {
        let mut graph = SkillEvolutionGraph::new();
        graph.add_skill(make_node("a", "A"));
        graph.add_skill(make_node("b", "B"));
        graph.add_skill(make_node("c", "C"));

        graph.add_relation("a", "b", EvolutionEdge::Extends).unwrap();
        graph.add_relation("a", "c", EvolutionEdge::DependsOn).unwrap();

        let succs = graph.successors("a").unwrap();
        assert_eq!(succs.len(), 2);

        let preds = graph.predecessors("b").unwrap();
        assert_eq!(preds.len(), 1);
        assert_eq!(preds[0].skill_id, "a");
    }

    #[test]
    fn test_active_and_deprecated_lists() {
        let mut graph = SkillEvolutionGraph::new();
        graph.add_skill(make_node("active1", "A1"));
        graph.add_skill(make_node("active2", "A2"));
        graph.add_skill(make_node("dep1", "D1"));
        graph.add_skill(make_node("ret1", "R1"));

        graph.deprecate("dep1", "old", None).unwrap();
        graph.retire("ret1").unwrap();

        assert_eq!(graph.active_skills().len(), 2);
        assert_eq!(graph.deprecated_skills().len(), 1);
    }

    #[test]
    fn test_no_cycles_in_simple_graph() {
        let mut graph = SkillEvolutionGraph::new();
        graph.add_skill(make_node("a", "A"));
        graph.add_skill(make_node("b", "B"));
        graph.add_skill(make_node("c", "C"));

        graph.add_relation("a", "b", EvolutionEdge::Extends).unwrap();
        graph.add_relation("b", "c", EvolutionEdge::DependsOn).unwrap();

        assert!(!graph.has_cycles());
    }

    #[test]
    fn test_cycle_detection() {
        let mut graph = SkillEvolutionGraph::new();
        graph.add_skill(make_node("a", "A"));
        graph.add_skill(make_node("b", "B"));

        graph.add_relation("a", "b", EvolutionEdge::DependsOn).unwrap();
        graph.add_relation("b", "a", EvolutionEdge::DependsOn).unwrap();

        assert!(graph.has_cycles());
    }

    #[test]
    fn test_stats() {
        let mut graph = SkillEvolutionGraph::new();
        graph.add_skill(make_node("a", "A"));
        graph.add_skill(make_node("b", "B"));
        graph.add_relation("a", "b", EvolutionEdge::Extends).unwrap();
        graph.deprecate("b", "old", None).unwrap();

        let stats = graph.stats();
        assert_eq!(stats.total_skills, 2);
        assert_eq!(stats.active_skills, 1);
        assert_eq!(stats.deprecated_skills, 1);
        assert_eq!(stats.retired_skills, 0);
        assert_eq!(stats.total_relations, 1);
        assert!(!stats.has_cycles);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let mut graph = SkillEvolutionGraph::new();
        graph.add_skill(make_node("x", "X"));
        graph.add_skill(make_node("y", "Y"));
        graph.add_relation("x", "y", EvolutionEdge::Replaces).unwrap();

        let json = graph.to_json().unwrap();
        assert!(json.contains("\"nodes\""));
        assert!(json.contains("\"edges\""));
        assert!(json.contains("replaces"));
    }

    #[test]
    fn test_evolution_edge_display() {
        assert_eq!(format!("{}", EvolutionEdge::Extends), "extends");
        assert_eq!(format!("{}", EvolutionEdge::Replaces), "replaces");
        assert_eq!(format!("{}", EvolutionEdge::Repairs), "repairs");
        assert_eq!(format!("{}", EvolutionEdge::DependsOn), "depends_on");
    }

    #[test]
    fn test_update_existing_skill() {
        let mut graph = SkillEvolutionGraph::new();
        graph.add_skill(make_node("s1", "Original"));
        assert_eq!(graph.skill_count(), 1);

        // Atualizar mesmo skill_id
        let mut updated = make_node("s1", "Updated Name");
        updated.version = "2.0".into();
        graph.add_skill(updated);

        assert_eq!(graph.skill_count(), 1); // Não duplicou
        assert_eq!(graph.get_skill("s1").unwrap().name, "Updated Name");
        assert_eq!(graph.get_skill("s1").unwrap().version, "2.0");
    }

    #[test]
    fn test_graph_stats_serialization() {
        let stats = GraphStats {
            total_skills: 10,
            active_skills: 7,
            deprecated_skills: 2,
            retired_skills: 1,
            total_relations: 5,
            has_cycles: false,
        };
        let json = serde_json::to_string(&stats).unwrap();
        let restored: GraphStats = serde_json::from_str(&json).unwrap();
        assert_eq!(stats, restored);
    }
}