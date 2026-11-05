//! # zylos-router
//!
//! Roteamento aterrado com coleira (leash), alvos sagrados e proposta de mudança.
//! Garante que agentes não desviem do escopo definido e nunca inventem capacidades.

pub mod channels;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RouterError {
    #[error("alvo fora da coleira: {target} (distância={distance}, max={max})")]
    OutOfLeash { target: String, distance: f64, max: f64 },
    #[error("alvo sagrado modificado sem aprovação: {0}")]
    SacredTargetModified(String),
    #[error("proposta de mudança rejeitada: {0}")]
    ChangeRejected(String),
    #[error("capacidade inventada bloqueada: {0}")]
    InventedCapabilityBlocked(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLevel { Low, Medium, High, Critical }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    pub id: String,
    pub name: String,
    pub keywords: Vec<String>,
    pub sacred: bool,
    pub embedding: Vec<f64>,
    pub track_record: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteResult {
    pub capability_id: String,
    pub score: f64,
    pub is_fallback: bool,
    pub match_type: MatchType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MatchType { ExactKeyword, Semantic, Fallback }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeProposal {
    pub original_target: String,
    pub proposed_target: String,
    pub justification: String,
    pub distance: f64,
    pub risk_level: RiskLevel,
    pub requires_human: bool,
    pub approved: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeashConfig {
    pub max_distance: f64,
    pub allow_change_proposals: bool,
    pub sacred_targets: HashSet<String>,
    pub proposal_threshold: usize,
    pub use_track_record_tiebreak: bool,
}

impl Default for LeashConfig {
    fn default() -> Self {
        Self {
            max_distance: 0.8,
            allow_change_proposals: true,
            sacred_targets: HashSet::new(),
            proposal_threshold: 3,
            use_track_record_tiebreak: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FallbackClusterEntry {
    pub marker: String,
    pub count: usize,
    pub last_seen: String,
}

pub struct GroundedRouter {
    config: LeashConfig,
    capabilities: Vec<Capability>,
    change_log: Vec<ChangeProposal>,
    fallback_clusters: HashMap<String, FallbackClusterEntry>,
    registered_ids: HashSet<String>,
}

impl GroundedRouter {
    pub fn new(config: LeashConfig) -> Self {
        Self { config, capabilities: Vec::new(), change_log: Vec::new(),
               fallback_clusters: HashMap::new(), registered_ids: HashSet::new() }
    }

    pub fn add_capability(&mut self, cap: Capability) {
        if cap.sacred { self.config.sacred_targets.insert(cap.id.clone()); }
        self.registered_ids.insert(cap.id.clone());
        self.capabilities.push(cap);
    }

    pub fn is_real(&self, capability_id: &str) -> bool {
        self.registered_ids.contains(capability_id)
    }

    fn tokenize(query: &str) -> Vec<String> {
        query.to_lowercase().split(|c: char| !c.is_alphanumeric())
             .filter(|s| !s.is_empty() && s.len() > 2)
             .map(|s| s.to_string()).collect()
    }

    pub fn cosine_distance(a: &[f64], b: &[f64]) -> f64 {
        if a.len() != b.len() || a.is_empty() { return 1.0; }
        let dot: f64 = a.iter().zip(b).map(|(x,y)| x*y).sum();
        let na: f64 = a.iter().map(|x| x*x).sum::<f64>().sqrt();
        let nb: f64 = b.iter().map(|x| x*x).sum::<f64>().sqrt();
        if na == 0.0 || nb == 0.0 { return 1.0; }
        1.0 - (dot / (na * nb))
    }

    pub fn check_leash(&self, origin: &[f64], target: &[f64]) -> Result<f64, RouterError> {
        let d = Self::cosine_distance(origin, target);
        if d > self.config.max_distance {
            Err(RouterError::OutOfLeash { target: format!("{:.4}", d), distance: d, max: self.config.max_distance })
        } else { Ok(d) }
    }

    pub fn route(&self, query: &str) -> RouteResult {
        let tokens = Self::tokenize(query);
        let mut best: Option<(String, f64, MatchType)> = None;
        for cap in &self.capabilities {
            let kh: usize = tokens.iter().filter(|t| cap.keywords.iter().any(|k| k.to_lowercase().contains(t.as_str()))).count();
            if kh > 0 {
                let sc = kh as f64 / tokens.len().max(1) as f64;
                if best.as_ref().map_or(true, |(_,ps,_)| sc > *ps) {
                    best = Some((cap.id.clone(), sc, MatchType::ExactKeyword));
                }
            }
            if !tokens.is_empty() && !cap.embedding.is_empty() {
                let qe = Self::pseudo_embedding(query);
                let ss = 1.0 - Self::cosine_distance(&qe, &cap.embedding);
                if ss > 0.3 && best.as_ref().map_or(true, |(_,ps,_)| ss > *ps) {
                    best = Some((cap.id.clone(), ss, MatchType::Semantic));
                }
            }
        }
        if self.config.use_track_record_tiebreak {
            if let Some((ref id, ref mut sc, _)) = best {
                if let Some(c) = self.capabilities.iter().find(|c| &c.id == id) { *sc += c.track_record * 0.01; }
            }
        }
        match best {
            Some((id, sc, mt)) => RouteResult { capability_id: id, score: sc, is_fallback: false, match_type: mt },
            None => RouteResult { capability_id: "general-purpose".into(), score: 0.0, is_fallback: true, match_type: MatchType::Fallback },
        }
    }

    fn pseudo_embedding(text: &str) -> Vec<f64> {
        let mut h = Sha256::new(); h.update(text.as_bytes()); let r = h.finalize();
        r.iter().map(|b| (*b as f64) / 255.0).collect()
    }

    fn normalize_marker(query: &str) -> String {
        let mut t = Self::tokenize(query); t.sort(); t.dedup(); t.join(":")
    }

    pub fn record_fallback(&mut self, query: &str) -> FallbackClusterEntry {
        let m = Self::normalize_marker(query);
        let e = self.fallback_clusters.entry(m.clone()).or_insert_with(|| FallbackClusterEntry { marker: m, count: 0, last_seen: String::new() });
        e.count += 1; e.last_seen = chrono::Utc::now().to_rfc3339(); e.clone()
    }

    pub fn route_com_proposta(&mut self, query: &str) -> (RouteResult, Option<ChangeProposal>) {
        let r = self.route(query);
        if r.is_fallback {
            let c = self.record_fallback(query);
            if c.count >= self.config.proposal_threshold && self.config.allow_change_proposals {
                let p = ChangeProposal { original_target: "general-purpose".into(), proposed_target: r.capability_id.clone(),
                    justification: format!("Fallback recorrente ({}x)", c.count), distance: 0.0,
                    risk_level: RiskLevel::Medium, requires_human: true, approved: false };
                self.change_log.push(p.clone()); return (r, Some(p));
            }
        }
        (r, None)
    }

    fn risk_for_target(target: &str) -> RiskLevel {
        let l = target.to_lowercase();
        if l.contains("proof") || l.contains("panel") || l.contains("verificar") { RiskLevel::Critical }
        else if l.contains("core") || l.contains("auth") || l.contains("security") { RiskLevel::High }
        else if l.contains("config") || l.contains("deploy") { RiskLevel::Medium }
        else { RiskLevel::Low }
    }

    pub fn propose_change(&mut self, original: &str, proposed: &str, justification: impl Into<String>, distance: f64) -> Result<ChangeProposal, RouterError> {
        if self.config.sacred_targets.contains(original) { return Err(RouterError::SacredTargetModified(original.into())); }
        if !self.is_real(proposed) && proposed != "general-purpose" { return Err(RouterError::InventedCapabilityBlocked(proposed.into())); }
        let rl = Self::risk_for_target(original);
        let p = ChangeProposal { original_target: original.into(), proposed_target: proposed.into(),
            justification: justification.into(), distance, risk_level: rl, requires_human: true, approved: false };
        self.change_log.push(p.clone()); Ok(p)
    }

    pub fn change_log(&self) -> &[ChangeProposal] { &self.change_log }
    pub fn fallback_clusters(&self) -> &HashMap<String, FallbackClusterEntry> { &self.fallback_clusters }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_cap(id: &str, keywords: &[&str], sacred: bool) -> Capability {
        Capability { id: id.into(), name: id.into(),
            keywords: keywords.iter().map(|s| s.to_string()).collect(),
            sacred, embedding: vec![1.0, 0.0, 0.0], track_record: 0.8 }
    }

    #[test]
    fn test_route_exact_keyword_match() {
        let mut r = GroundedRouter::new(LeashConfig::default());
        r.add_capability(make_cap("code-gen", &["generate", "code", "write"], false));
        r.add_capability(make_cap("review", &["review", "check", "audit"], false));
        let res = r.route("generate code for module");
        assert_eq!(res.capability_id, "code-gen");
        assert!(!res.is_fallback);
        assert_eq!(res.match_type, MatchType::ExactKeyword);
        assert!(res.score > 0.0);
    }

    #[test]
    fn test_route_fallback_when_no_match() {
        let mut r = GroundedRouter::new(LeashConfig::default());
        r.add_capability(make_cap("code-gen", &["generate", "code"], false));
        let res = r.route("something completely unrelated xyz");
        assert_eq!(res.capability_id, "general-purpose");
        assert!(res.is_fallback);
        assert_eq!(res.score, 0.0);
        assert_eq!(res.match_type, MatchType::Fallback);
    }

    #[test]
    fn test_is_real_blocks_invented_capability() {
        let mut r = GroundedRouter::new(LeashConfig::default());
        r.add_capability(make_cap("real-cap", &["real"], false));
        assert!(r.is_real("real-cap"));
        assert!(!r.is_real("fake-cap"));
        assert!(!r.is_real("invented-thing"));
    }

    #[test]
    fn test_propose_change_requires_human_always() {
        let mut r = GroundedRouter::new(LeashConfig::default());
        r.add_capability(make_cap("old-api", &["old"], false));
        r.add_capability(make_cap("new-api", &["new"], false));
        let p = r.propose_change("old-api", "new-api", "better fit", 0.3).unwrap();
        assert!(p.requires_human);
        assert!(!p.approved);
    }

    #[test]
    fn test_propose_change_blocks_invented_capability() {
        let mut r = GroundedRouter::new(LeashConfig::default());
        r.add_capability(make_cap("real", &["real"], false));
        let res = r.propose_change("real", "nonexistent-fake", "try", 0.1);
        assert!(res.is_err());
        match res.unwrap_err() {
            RouterError::InventedCapabilityBlocked(id) => assert_eq!(id, "nonexistent-fake"),
            other => panic!("Expected InventedCapabilityBlocked, got {:?}", other),
        }
    }

    #[test]
    fn test_sacred_target_blocks_change_with_gate() {
        let mut config = LeashConfig::default();
        config.sacred_targets.insert("proof-verifier".into());
        let mut r = GroundedRouter::new(config);
        r.add_capability(make_cap("proof-verifier", &["proof"], true));
        let res = r.propose_change("proof-verifier", "new-proof", "refactor", 0.1);
        assert!(res.is_err());
        match res.unwrap_err() {
            RouterError::SacredTargetModified(id) => assert_eq!(id, "proof-verifier"),
            other => panic!("Expected SacredTargetModified, got {:?}", other),
        }
    }

    #[test]
    fn test_risk_level_by_target_type() {
        assert_eq!(GroundedRouter::risk_for_target("proof-checker"), RiskLevel::Critical);
        assert_eq!(GroundedRouter::risk_for_target("panel-verifier"), RiskLevel::Critical);
        assert_eq!(GroundedRouter::risk_for_target("core-auth"), RiskLevel::High);
        assert_eq!(GroundedRouter::risk_for_target("deploy-config"), RiskLevel::Medium);
        assert_eq!(GroundedRouter::risk_for_target("readme-update"), RiskLevel::Low);
    }

    #[test]
    fn test_route_com_proposta_below_threshold() {
        let mut r = GroundedRouter::new(LeashConfig { proposal_threshold: 3, ..Default::default() });
        r.add_capability(make_cap("code-gen", &["generate"], false));
        let (res, prop) = r.route_com_proposta("unrelated topic xyz");
        assert!(res.is_fallback);
        assert!(prop.is_none());
    }

    #[test]
    fn test_route_com_proposta_above_threshold() {
        let mut r = GroundedRouter::new(LeashConfig { proposal_threshold: 3, ..Default::default() });
        r.add_capability(make_cap("code-gen", &["generate"], false));
        for _ in 0..3 { r.route_com_proposta("unrelated topic xyz"); }
        let (res, prop) = r.route_com_proposta("unrelated topic xyz");
        assert!(res.is_fallback);
        assert!(prop.is_some());
        let p = prop.unwrap();
        assert!(p.requires_human);
        assert!(!p.approved);
    }

    #[test]
    fn test_fallback_dedup_persistent() {
        let mut r = GroundedRouter::new(LeashConfig::default());
        r.record_fallback("deploy error timeout");
        r.record_fallback("timeout deploy error");
        r.record_fallback("different issue here");
        let clusters = r.fallback_clusters();
        assert_eq!(clusters.len(), 2);
        let key1 = GroundedRouter::normalize_marker("deploy error timeout");
        assert_eq!(clusters[&key1].count, 2);
    }

    #[test]
    fn test_cosine_distance_identical() {
        let v = vec![1.0, 0.0, 0.0];
        assert!(GroundedRouter::cosine_distance(&v, &v) < 1e-10);
    }

    #[test]
    fn test_cosine_distance_orthogonal() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        assert!((GroundedRouter::cosine_distance(&a, &b) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_leash_allows_close_target() {
        let r = GroundedRouter::new(LeashConfig { max_distance: 0.5, ..Default::default() });
        assert!(r.check_leash(&[1.0, 0.0, 0.0], &[0.9, 0.1, 0.0]).is_ok());
    }

    #[test]
    fn test_leash_rejects_far_target() {
        let r = GroundedRouter::new(LeashConfig { max_distance: 0.1, ..Default::default() });
        assert!(r.check_leash(&[1.0, 0.0], &[0.0, 1.0]).is_err());
    }

    #[test]
    fn test_track_record_tiebreak() {
        let config = LeashConfig { use_track_record_tiebreak: true, ..Default::default() };
        let mut r = GroundedRouter::new(config);
        let mut ca = make_cap("cap-a", &["test"], false); ca.track_record = 0.9;
        let mut cb = make_cap("cap-b", &["test"], false); cb.track_record = 0.1;
        r.add_capability(ca); r.add_capability(cb);
        let res = r.route("test something");
        assert_eq!(res.capability_id, "cap-a");
    }
}
