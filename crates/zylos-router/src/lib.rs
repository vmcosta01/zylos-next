//! # zylos-router
//!
//! Roteamento aterrado com coleira (leash), alvos sagrados e proposta de mudança.
//! Garante que agentes não desviem do escopo definido e nunca inventem capacidades.

pub mod channels;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

/// Erros produzidos pelo roteador aterrado durante validação de coleira,
/// alvos sagrados e propostas de mudança.
#[derive(Debug, Error)]
pub enum RouterError {
    #[error("alvo fora da coleira: {target} (distância={distance}, max={max})")]
    OutOfLeash {
        target: String,
        distance: f64,
        max: f64,
    },
    /// Um alvo marcado como sagrado foi modificado sem aprovação humana.
    #[error("alvo sagrado modificado sem aprovação: {0}")]
    SacredTargetModified(String),
    /// A proposta de mudança foi rejeitada pelo gate de risco ou política.
    #[error("proposta de mudança rejeitada: {0}")]
    ChangeRejected(String),
    /// Uma capacidade não registrada foi referenciada e bloqueada.
    #[error("capacidade inventada bloqueada: {0}")]
    InventedCapabilityBlocked(String),
}

/// Nível de risco associado a um alvo ou proposta de mudança.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

/// Capacidade registrada no roteador com metadados de matching e segurança.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    /// Identificador único da capacidade.
    pub id: String,
    /// Nome legível da capacidade.
    pub name: String,
    /// Palavras-chave para matching exato em queries.
    pub keywords: Vec<String>,
    /// Se verdadeiro, alterações nesta capacidade exigem aprovação humana.
    pub sacred: bool,
    /// Vetor de embedding para matching semântico.
    pub embedding: Vec<f64>,
    /// Histórico de acertos (0.0–1.0) usado como tiebreak quando habilitado.
    pub track_record: f64,
}

/// Resultado de uma operação de roteamento.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteResult {
    /// ID da capacidade selecionada.
    pub capability_id: String,
    /// Score de relevância do match (maior = melhor).
    pub score: f64,
    /// Verdadeiro quando nenhum match foi encontrado e o fallback foi usado.
    pub is_fallback: bool,
    /// Tipo de match que produziu este resultado.
    pub match_type: MatchType,
}

/// Estratégia usada para produzir um [`RouteResult`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MatchType {
    ExactKeyword,
    Semantic,
    Fallback,
}

/// Proposta de mudança de alvo gerada pelo roteador ou por um agente.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeProposal {
    /// Alvo original antes da proposta.
    pub original_target: String,
    /// Novo alvo proposto.
    pub proposed_target: String,
    /// Justificativa textual para a mudança.
    pub justification: String,
    /// Distância semântica entre os dois alvos.
    pub distance: f64,
    /// Nível de risco inferido para o alvo original.
    pub risk_level: RiskLevel,
    /// Sempre verdadeiro: mudanças exigem aprovação humana.
    pub requires_human: bool,
    /// Status de aprovação; começa como `false`.
    pub approved: bool,
}

/// Configuração da coleira (leash) que limita o escopo do roteador.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeashConfig {
    /// Distância coseno máxima permitida entre origem e alvo.
    pub max_distance: f64,
    /// Se verdadeiro, fallbacks recorrentes geram propostas de mudança.
    pub allow_change_proposals: bool,
    /// Conjunto de IDs de capacidades sagradas (imutáveis sem aprovação).
    pub sacred_targets: HashSet<String>,
    /// Número mínimo de fallbacks repetidos antes de propor mudança.
    pub proposal_threshold: usize,
    /// Se verdadeiro, usa `track_record` como critério de desempate.
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

/// Entrada agregada de um cluster de fallbacks recorrentes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FallbackClusterEntry {
    /// Marcador normalizado que identifica o cluster.
    pub marker: String,
    /// Número de vezes que este cluster foi observado.
    pub count: usize,
    /// Timestamp RFC 3339 da última ocorrência.
    pub last_seen: String,
}

/// Roteador aterrado com coleira, alvos sagrados e detecção de capacidades inventadas.
pub struct GroundedRouter {
    config: LeashConfig,
    capabilities: Vec<Capability>,
    change_log: Vec<ChangeProposal>,
    fallback_clusters: HashMap<String, FallbackClusterEntry>,
    registered_ids: HashSet<String>,
}

impl GroundedRouter {
    /// Cria um novo roteador com a configuração de coleira fornecida.
    pub fn new(config: LeashConfig) -> Self {
        Self {
            config,
            capabilities: Vec::new(),
            change_log: Vec::new(),
            fallback_clusters: HashMap::new(),
            registered_ids: HashSet::new(),
        }
    }

    /// Registra uma capacidade; se for sagrada, adiciona-a ao conjunto de alvos protegidos.
    pub fn add_capability(&mut self, cap: Capability) {
        if cap.sacred {
            self.config.sacred_targets.insert(cap.id.clone());
        }
        self.registered_ids.insert(cap.id.clone());
        self.capabilities.push(cap);
    }

    /// Verifica se uma capacidade está registrada (não inventada).
    pub fn is_real(&self, capability_id: &str) -> bool {
        self.registered_ids.contains(capability_id)
    }

    fn tokenize(query: &str) -> Vec<String> {
        query
            .to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty() && s.len() > 2)
            .map(|s| s.to_string())
            .collect()
    }

    /// Calcula a distância coseno entre dois vetores normalizados.
    ///
    /// Retorna `1.0` (máxima dissimilaridade) se os vetores tiverem
    /// tamanhos diferentes ou forem vazios.
    pub fn cosine_distance(a: &[f64], b: &[f64]) -> f64 {
        if a.len() != b.len() || a.is_empty() {
            return 1.0;
        }
        let dot: f64 = a.iter().zip(b).map(|(x, y)| x * y).sum();
        let na: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
        let nb: f64 = b.iter().map(|x| x * x).sum::<f64>().sqrt();
        if na == 0.0 || nb == 0.0 {
            return 1.0;
        }
        1.0 - (dot / (na * nb))
    }

    /// Valida se o alvo está dentro da coleira; retorna a distância ou erro.
    pub fn check_leash(&self, origin: &[f64], target: &[f64]) -> Result<f64, RouterError> {
        let d = Self::cosine_distance(origin, target);
        if d > self.config.max_distance {
            Err(RouterError::OutOfLeash {
                target: format!("{:.4}", d),
                distance: d,
                max: self.config.max_distance,
            })
        } else {
            Ok(d)
        }
    }

    /// Roteia uma query para a melhor capacidade registrada usando keyword + semântica.
    ///
    /// Retorna fallback (`general-purpose`) quando nenhum match atinge os thresholds.
    pub fn route(&self, query: &str) -> RouteResult {
        let tokens = Self::tokenize(query);
        let mut best: Option<(String, f64, MatchType)> = None;
        for cap in &self.capabilities {
            let kh: usize = tokens
                .iter()
                .filter(|t| {
                    cap.keywords
                        .iter()
                        .any(|k| k.to_lowercase().contains(t.as_str()))
                })
                .count();
            if kh > 0 {
                let sc = kh as f64 / tokens.len().max(1) as f64;
                if best.as_ref().is_none_or(|(_, ps, _)| sc > *ps) {
                    best = Some((cap.id.clone(), sc, MatchType::ExactKeyword));
                }
            }
            if !tokens.is_empty() && !cap.embedding.is_empty() {
                let qe = Self::pseudo_embedding(query);
                let ss = 1.0 - Self::cosine_distance(&qe, &cap.embedding);
                if ss > 0.3 && best.as_ref().is_none_or(|(_, ps, _)| ss > *ps) {
                    best = Some((cap.id.clone(), ss, MatchType::Semantic));
                }
            }
        }
        if self.config.use_track_record_tiebreak {
            if let Some((ref id, ref mut sc, _)) = best {
                if let Some(c) = self.capabilities.iter().find(|c| &c.id == id) {
                    *sc += c.track_record * 0.01;
                }
            }
        }
        match best {
            Some((id, sc, mt)) => RouteResult {
                capability_id: id,
                score: sc,
                is_fallback: false,
                match_type: mt,
            },
            None => RouteResult {
                capability_id: "general-purpose".into(),
                score: 0.0,
                is_fallback: true,
                match_type: MatchType::Fallback,
            },
        }
    }

    fn pseudo_embedding(text: &str) -> Vec<f64> {
        let mut h = Sha256::new();
        h.update(text.as_bytes());
        let r = h.finalize();
        r.iter().map(|b| (*b as f64) / 255.0).collect()
    }

    fn normalize_marker(query: &str) -> String {
        let mut t = Self::tokenize(query);
        t.sort();
        t.dedup();
        t.join(":")
    }

    /// Registra uma ocorrência de fallback e retorna a entrada atualizada do cluster.
    pub fn record_fallback(&mut self, query: &str) -> FallbackClusterEntry {
        let m = Self::normalize_marker(query);
        let e = self
            .fallback_clusters
            .entry(m.clone())
            .or_insert_with(|| FallbackClusterEntry {
                marker: m,
                count: 0,
                last_seen: String::new(),
            });
        e.count += 1;
        e.last_seen = chrono::Utc::now().to_rfc3339();
        e.clone()
    }

    /// Roteia e, se o fallback atingir o threshold configurado, gera uma proposta de mudança.
    pub fn route_com_proposta(&mut self, query: &str) -> (RouteResult, Option<ChangeProposal>) {
        let r = self.route(query);
        if r.is_fallback {
            let c = self.record_fallback(query);
            if c.count >= self.config.proposal_threshold && self.config.allow_change_proposals {
                let p = ChangeProposal {
                    original_target: "general-purpose".into(),
                    proposed_target: r.capability_id.clone(),
                    justification: format!("Fallback recorrente ({}x)", c.count),
                    distance: 0.0,
                    risk_level: RiskLevel::Medium,
                    requires_human: true,
                    approved: false,
                };
                self.change_log.push(p.clone());
                return (r, Some(p));
            }
        }
        (r, None)
    }

    fn risk_for_target(target: &str) -> RiskLevel {
        let l = target.to_lowercase();
        if l.contains("proof") || l.contains("panel") || l.contains("verificar") {
            RiskLevel::Critical
        } else if l.contains("core") || l.contains("auth") || l.contains("security") {
            RiskLevel::High
        } else if l.contains("config") || l.contains("deploy") {
            RiskLevel::Medium
        } else {
            RiskLevel::Low
        }
    }

    /// Propõe uma mudança de alvo validando alvos sagrados e capacidades inventadas.
    ///
    /// Retorna erro se o alvo original for sagrado ou se o proposto não estiver registrado.
    pub fn propose_change(
        &mut self,
        original: &str,
        proposed: &str,
        justification: impl Into<String>,
        distance: f64,
    ) -> Result<ChangeProposal, RouterError> {
        if self.config.sacred_targets.contains(original) {
            return Err(RouterError::SacredTargetModified(original.into()));
        }
        if !self.is_real(proposed) && proposed != "general-purpose" {
            return Err(RouterError::InventedCapabilityBlocked(proposed.into()));
        }
        let rl = Self::risk_for_target(original);
        let p = ChangeProposal {
            original_target: original.into(),
            proposed_target: proposed.into(),
            justification: justification.into(),
            distance,
            risk_level: rl,
            requires_human: true,
            approved: false,
        };
        self.change_log.push(p.clone());
        Ok(p)
    }

    /// Retorna o log completo de propostas de mudança registradas.
    pub fn change_log(&self) -> &[ChangeProposal] {
        &self.change_log
    }
    /// Retorna todos os clusters de fallback observados até agora.
    pub fn fallback_clusters(&self) -> &HashMap<String, FallbackClusterEntry> {
        &self.fallback_clusters
    }
}

// ─── Channel Registry & Session Manager ────────────────────────────────

use channels::{ChannelAdapter, ChannelError, ChannelSession, InboundMessage, OutboundMessage};

/// Registry that dispatches inbound/outbound messages to the correct adapter
pub struct ChannelRegistry {
    adapters: HashMap<String, Box<dyn ChannelAdapter>>,
}

impl ChannelRegistry {
    pub fn new() -> Self {
        Self {
            adapters: HashMap::new(),
        }
    }

    /// Register a channel adapter. Returns error if name already taken.
    pub fn register(&mut self, adapter: impl ChannelAdapter + 'static) -> Result<(), String> {
        let name = adapter.name().to_string();
        if self.adapters.contains_key(&name) {
            return Err(format!("channel '{}' already registered", name));
        }
        self.adapters.insert(name, Box::new(adapter));
        Ok(())
    }

    /// Get a reference to an adapter by name
    pub fn get(&self, name: &str) -> Option<&dyn ChannelAdapter> {
        self.adapters.get(name).map(|a| a.as_ref())
    }

    /// List all configured channel names
    pub fn list_configured(&self) -> Vec<&str> {
        self.adapters
            .iter()
            .filter(|(_, a)| a.is_configured())
            .map(|(k, _)| k.as_str())
            .collect()
    }

    /// Dispatch an inbound webhook payload to the named channel
    pub async fn receive(
        &self,
        channel: &str,
        payload: bytes::Bytes,
    ) -> Result<InboundMessage, ChannelError> {
        let adapter = self
            .adapters
            .get(channel)
            .ok_or_else(|| ChannelError::InvalidMessage(format!("unknown channel: {}", channel)))?;
        adapter.receive(payload).await
    }

    /// Send an outbound message through the named channel
    pub async fn send(
        &self,
        channel: &str,
        target: &str,
        msg: OutboundMessage,
    ) -> Result<(), ChannelError> {
        let adapter = self
            .adapters
            .get(channel)
            .ok_or_else(|| ChannelError::InvalidMessage(format!("unknown channel: {}", channel)))?;
        adapter.send(target, msg).await
    }

    /// Verify webhook for the named channel
    pub async fn verify_webhook(
        &self,
        channel: &str,
        challenge: &str,
        signature: Option<&str>,
        body: &[u8],
    ) -> Result<String, ChannelError> {
        let adapter = self
            .adapters
            .get(channel)
            .ok_or_else(|| ChannelError::InvalidMessage(format!("unknown channel: {}", channel)))?;
        adapter.verify_webhook(challenge, signature, body).await
    }
}

impl Default for ChannelRegistry {
    /// Cria um registry vazio via [`ChannelRegistry::new`].
    fn default() -> Self {
        Self::new()
    }
}

/// Manages channel sessions (external user ↔ zylos user mapping)
/// Gerencia sessões de canal mapeando usuários externos para usuários zylos.
pub struct SessionManager {
    sessions: HashMap<String, ChannelSession>,
    /// Index: external_user_id → session id
    external_index: HashMap<String, String>,
}

impl SessionManager {
    /// Cria um gerenciador de sessões vazio.
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            external_index: HashMap::new(),
        }
    }

    /// Create or retrieve a session for an external user on a channel
    /// Obtém ou cria uma sessão para um usuário externo em um canal específico.
    ///
    /// Atualiza `last_active_at` em acessos subsequentes.
    pub fn get_or_create(
        &mut self,
        channel_type: &str,
        external_user_id: &str,
        zylos_user_id: &str,
    ) -> &ChannelSession {
        let key = format!("{}:{}", channel_type, external_user_id);
        if !self.sessions.contains_key(&key) {
            let now = chrono::Utc::now().timestamp();
            let session = ChannelSession {
                id: uuid::Uuid::new_v4().to_string(),
                channel_type: channel_type.to_string(),
                external_user_id: external_user_id.to_string(),
                zylos_user_id: zylos_user_id.to_string(),
                metadata: serde_json::Value::Null,
                last_active_at: now,
                created_at: now,
            };
            self.external_index.insert(key.clone(), session.id.clone());
            self.sessions.insert(key.clone(), session);
        }
        // Touch last_active_at
        if let Some(s) = self.sessions.get_mut(&key) {
            s.last_active_at = chrono::Utc::now().timestamp();
        }
        self.sessions.get(&key).unwrap()
    }

    /// Look up a session by channel + external user
    pub fn lookup(&self, channel_type: &str, external_user_id: &str) -> Option<&ChannelSession> {
        let key = format!("{}:{}", channel_type, external_user_id);
        self.sessions.get(&key)
    }

    /// Count active sessions
    pub fn count(&self) -> usize {
        self.sessions.len()
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_cap(id: &str, keywords: &[&str], sacred: bool) -> Capability {
        Capability {
            id: id.into(),
            name: id.into(),
            keywords: keywords.iter().map(|s| s.to_string()).collect(),
            sacred,
            embedding: vec![1.0, 0.0, 0.0],
            track_record: 0.8,
        }
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
        let p = r
            .propose_change("old-api", "new-api", "better fit", 0.3)
            .unwrap();
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
        assert_eq!(
            GroundedRouter::risk_for_target("proof-checker"),
            RiskLevel::Critical
        );
        assert_eq!(
            GroundedRouter::risk_for_target("panel-verifier"),
            RiskLevel::Critical
        );
        assert_eq!(
            GroundedRouter::risk_for_target("core-auth"),
            RiskLevel::High
        );
        assert_eq!(
            GroundedRouter::risk_for_target("deploy-config"),
            RiskLevel::Medium
        );
        assert_eq!(
            GroundedRouter::risk_for_target("readme-update"),
            RiskLevel::Low
        );
    }

    #[test]
    fn test_route_com_proposta_below_threshold() {
        let mut r = GroundedRouter::new(LeashConfig {
            proposal_threshold: 3,
            ..Default::default()
        });
        r.add_capability(make_cap("code-gen", &["generate"], false));
        let (res, prop) = r.route_com_proposta("unrelated topic xyz");
        assert!(res.is_fallback);
        assert!(prop.is_none());
    }

    #[test]
    fn test_route_com_proposta_above_threshold() {
        let mut r = GroundedRouter::new(LeashConfig {
            proposal_threshold: 3,
            ..Default::default()
        });
        r.add_capability(make_cap("code-gen", &["generate"], false));
        for _ in 0..3 {
            r.route_com_proposta("unrelated topic xyz");
        }
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
        let r = GroundedRouter::new(LeashConfig {
            max_distance: 0.5,
            ..Default::default()
        });
        assert!(r.check_leash(&[1.0, 0.0, 0.0], &[0.9, 0.1, 0.0]).is_ok());
    }

    #[test]
    fn test_leash_rejects_far_target() {
        let r = GroundedRouter::new(LeashConfig {
            max_distance: 0.1,
            ..Default::default()
        });
        assert!(r.check_leash(&[1.0, 0.0], &[0.0, 1.0]).is_err());
    }

    #[test]
    fn test_track_record_tiebreak() {
        let config = LeashConfig {
            use_track_record_tiebreak: true,
            ..Default::default()
        };
        let mut r = GroundedRouter::new(config);
        let mut ca = make_cap("cap-a", &["test"], false);
        ca.track_record = 0.9;
        let mut cb = make_cap("cap-b", &["test"], false);
        cb.track_record = 0.1;
        r.add_capability(ca);
        r.add_capability(cb);
        let res = r.route("test something");
        assert_eq!(res.capability_id, "cap-a");
    }
}
