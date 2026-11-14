//! # zylos-proof
//!
//! Proof engine determinística com 5 níveis de ground e chain of evidence.
//! Portado do CORTEX proof engine para Rust, com hashing SHA-256 de evidências
//! e verificação estrutural independente de LLM.

pub mod evidence_capsule;
pub mod honesty;
pub mod santa;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fmt;

/// Níveis de certeza (ground) — hierarquia estrita.
/// Um claim só é aceito se o ground for >= ao mínimo exigido pelo gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Ground {
    /// Dado bruto não verificado (ex: output de LLM sem confirmação)
    Assumed = 0,
    /// Observado diretamente (ex: arquivo existe, comando retornou 0)
    Observed = 1,
    /// Confirmado por fonte independente (ex: teste passou, hash bateu)
    Confirmed = 2,
    /// Verificado por múltiplas fontes independentes
    Verified = 3,
    /// Provado matematicamente ou por invariante estrutural
    Proven = 4,
}

impl fmt::Display for Ground {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ground::Assumed => write!(f, "assumed"),
            Ground::Observed => write!(f, "observed"),
            Ground::Confirmed => write!(f, "confirmed"),
            Ground::Verified => write!(f, "verified"),
            Ground::Proven => write!(f, "proven"),
        }
    }
}

impl Ground {
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Ground::Assumed),
            1 => Some(Ground::Observed),
            2 => Some(Ground::Confirmed),
            3 => Some(Ground::Verified),
            4 => Some(Ground::Proven),
            _ => None,
        }
    }
}

/// Uma peça de evidência na cadeia de prova.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    /// Descrição legível da evidência
    pub description: String,
    /// Nível de ground desta evidência
    pub ground: Ground,
    /// Hash SHA-256 do conteúdo da evidência (para integridade)
    pub content_hash: String,
    /// Fonte da evidência (ex: "test_runner", "file_check", "llm_judge")
    pub source: String,
    /// Timestamp ISO 8601
    pub timestamp: String,
}

impl Evidence {
    /// Cria nova evidência com hash computado automaticamente.
    pub fn new(
        description: impl Into<String>,
        ground: Ground,
        content: &[u8],
        source: impl Into<String>,
    ) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(content);
        let hash = format!("{:x}", hasher.finalize());

        Self {
            description: description.into(),
            ground,
            content_hash: hash,
            source: source.into(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Verifica se o hash da evidência ainda corresponde ao conteúdo original.
    pub fn verify_integrity(&self, content: &[u8]) -> bool {
        let mut hasher = Sha256::new();
        hasher.update(content);
        let expected = format!("{:x}", hasher.finalize());
        self.content_hash == expected
    }
}

/// Resultado de uma verificação de proof.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofResult {
    /// Se a prova passou
    pub passed: bool,
    /// Ground mínimo exigido
    pub required_ground: Ground,
    /// Ground máximo alcançado na cadeia
    pub achieved_ground: Ground,
    /// Cadeia de evidências
    pub evidence_chain: Vec<Evidence>,
    /// Hash composto de todas as evidências (para auditoria)
    pub chain_hash: String,
    /// Razão da falha (se aplicável)
    pub failure_reason: Option<String>,
}

impl ProofResult {
    /// Computa o hash composto da cadeia de evidências.
    pub fn compute_chain_hash(evidence_chain: &[Evidence]) -> String {
        let mut hasher = Sha256::new();
        for ev in evidence_chain {
            hasher.update(ev.content_hash.as_bytes());
            hasher.update(b"|");
        }
        format!("{:x}", hasher.finalize())
    }

    /// Retorna o ground máximo alcançado na cadeia.
    pub fn max_ground(evidence_chain: &[Evidence]) -> Ground {
        evidence_chain
            .iter()
            .map(|e| e.ground)
            .max()
            .unwrap_or(Ground::Assumed)
    }

    /// Retorna apenas as evidências que falharam (ground < required).
    pub fn weak_evidence(&self) -> Vec<&Evidence> {
        self.evidence_chain
            .iter()
            .filter(|e| e.ground < self.required_ground)
            .collect()
    }
}

/// Builder para construir provas de forma ergonômica.
pub struct ProofBuilder {
    required_ground: Ground,
    evidence_chain: Vec<Evidence>,
}

impl ProofBuilder {
    pub fn new(required_ground: Ground) -> Self {
        Self {
            required_ground,
            evidence_chain: Vec::new(),
        }
    }

    /// Adiciona evidência à cadeia.
    pub fn add_evidence(mut self, evidence: Evidence) -> Self {
        self.evidence_chain.push(evidence);
        self
    }

    /// Adiciona evidência observada (Ground::Observed).
    pub fn observe(
        mut self,
        description: impl Into<String>,
        content: &[u8],
        source: impl Into<String>,
    ) -> Self {
        self.evidence_chain.push(Evidence::new(
            description,
            Ground::Observed,
            content,
            source,
        ));
        self
    }

    /// Adiciona evidência confirmada (Ground::Confirmed).
    pub fn confirm(
        mut self,
        description: impl Into<String>,
        content: &[u8],
        source: impl Into<String>,
    ) -> Self {
        self.evidence_chain.push(Evidence::new(
            description,
            Ground::Confirmed,
            content,
            source,
        ));
        self
    }

    /// Adiciona evidência verificada (Ground::Verified).
    pub fn verify(
        mut self,
        description: impl Into<String>,
        content: &[u8],
        source: impl Into<String>,
    ) -> Self {
        self.evidence_chain.push(Evidence::new(
            description,
            Ground::Verified,
            content,
            source,
        ));
        self
    }

    /// Constrói o resultado da prova.
    pub fn build(self) -> ProofResult {
        let achieved = ProofResult::max_ground(&self.evidence_chain);
        let chain_hash = ProofResult::compute_chain_hash(&self.evidence_chain);
        let passed = achieved >= self.required_ground;

        let failure_reason = if !passed {
            Some(format!(
                "Ground insuficiente: requerido={}, alcançado={}",
                self.required_ground, achieved
            ))
        } else {
            None
        };

        ProofResult {
            passed,
            required_ground: self.required_ground,
            achieved_ground: achieved,
            evidence_chain: self.evidence_chain,
            chain_hash,
            failure_reason,
        }
    }
}

/// Gate de verificação — define o ground mínimo para aceitar um claim.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationGate {
    /// Nome do gate
    pub name: String,
    /// Ground mínimo necessário
    pub min_ground: Ground,
    /// Se evidências de LLM são aceitas como fonte primária
    pub accept_llm_as_primary: bool,
    /// Número mínimo de evidências independentes
    pub min_independent_sources: usize,
}

impl VerificationGate {
    pub fn new(name: impl Into<String>, min_ground: Ground) -> Self {
        Self {
            name: name.into(),
            min_ground,
            accept_llm_as_primary: false,
            min_independent_sources: 1,
        }
    }

    /// Configura se LLM é aceito como fonte primária.
    pub fn with_llm_primary(mut self, accept: bool) -> Self {
        self.accept_llm_as_primary = accept;
        self
    }

    /// Configura número mínimo de fontes independentes.
    pub fn with_min_sources(mut self, min: usize) -> Self {
        self.min_independent_sources = min;
        self
    }

    /// Avalia se uma prova passa neste gate.
    pub fn evaluate(&self, proof: &ProofResult) -> GateResult {
        let ground_ok = proof.achieved_ground >= self.min_ground;

        let sources: HashSet<&str> = proof
            .evidence_chain
            .iter()
            .map(|e| e.source.as_str())
            .collect();
        let sources_ok = sources.len() >= self.min_independent_sources;

        let llm_ok = if !self.accept_llm_as_primary {
            proof.evidence_chain.iter().any(|e| {
                e.ground >= self.min_ground
                    && e.source != "llm_judge"
                    && e.source != "llm_generator"
            })
        } else {
            true
        };

        let passed = ground_ok && sources_ok && llm_ok;

        GateResult {
            gate_name: self.name.clone(),
            passed,
            ground_ok,
            sources_ok,
            llm_ok,
            details: if !passed {
                let mut reasons = Vec::new();
                if !ground_ok {
                    reasons.push(format!(
                        "ground: {} < {}",
                        proof.achieved_ground, self.min_ground
                    ));
                }
                if !sources_ok {
                    reasons.push(format!(
                        "fontes: {} < {}",
                        sources.len(),
                        self.min_independent_sources
                    ));
                }
                if !llm_ok {
                    reasons.push("LLM não aceito como fonte primária".into());
                }
                Some(reasons.join("; "))
            } else {
                None
            },
        }
    }
}

/// Resultado da avaliação de um gate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateResult {
    pub gate_name: String,
    pub passed: bool,
    pub ground_ok: bool,
    pub sources_ok: bool,
    pub llm_ok: bool,
    pub details: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ground_ordering() {
        assert!(Ground::Proven > Ground::Verified);
        assert!(Ground::Verified > Ground::Confirmed);
        assert!(Ground::Confirmed > Ground::Observed);
        assert!(Ground::Observed > Ground::Assumed);
    }

    #[test]
    fn test_ground_roundtrip() {
        for v in 0..=4 {
            let g = Ground::from_u8(v).unwrap();
            assert_eq!(g.as_u8(), v);
        }
        assert!(Ground::from_u8(5).is_none());
    }

    #[test]
    fn test_evidence_hash_integrity() {
        let content = b"test output: all 42 assertions passed";
        let ev = Evidence::new("tests passed", Ground::Confirmed, content, "test_runner");
        assert!(ev.verify_integrity(content));
        assert!(!ev.verify_integrity(b"different content"));
    }

    #[test]
    fn test_proof_builder_pass() {
        let proof = ProofBuilder::new(Ground::Confirmed)
            .observe("file exists", b"/src/lib.rs exists", "file_check")
            .confirm("tests pass", b"42/42 passed", "test_runner")
            .build();

        assert!(proof.passed);
        assert_eq!(proof.achieved_ground, Ground::Confirmed);
        assert_eq!(proof.evidence_chain.len(), 2);
        assert!(proof.failure_reason.is_none());
    }

    #[test]
    fn test_proof_builder_fail_insufficient_ground() {
        let proof = ProofBuilder::new(Ground::Verified)
            .observe("file exists", b"/src/lib.rs exists", "file_check")
            .build();

        assert!(!proof.passed);
        assert_eq!(proof.achieved_ground, Ground::Observed);
        assert!(proof.failure_reason.is_some());
        assert!(proof.failure_reason.unwrap().contains("insuficiente"));
    }

    #[test]
    fn test_chain_hash_deterministic() {
        let proof1 = ProofBuilder::new(Ground::Observed)
            .observe("a", b"content_a", "src1")
            .observe("b", b"content_b", "src2")
            .build();

        let proof2 = ProofBuilder::new(Ground::Observed)
            .observe("a", b"content_a", "src1")
            .observe("b", b"content_b", "src2")
            .build();

        assert_eq!(proof1.chain_hash, proof2.chain_hash);
    }

    #[test]
    fn test_chain_hash_changes_with_content() {
        let proof1 = ProofBuilder::new(Ground::Observed)
            .observe("a", b"content_a", "src1")
            .build();

        let proof2 = ProofBuilder::new(Ground::Observed)
            .observe("a", b"content_b", "src1")
            .build();

        assert_ne!(proof1.chain_hash, proof2.chain_hash);
    }

    #[test]
    fn test_gate_pass() {
        let gate = VerificationGate::new("deploy_gate", Ground::Confirmed);
        let proof = ProofBuilder::new(Ground::Confirmed)
            .confirm("tests pass", b"ok", "test_runner")
            .build();

        let result = gate.evaluate(&proof);
        assert!(result.passed);
        assert!(result.ground_ok);
        assert!(result.sources_ok);
    }

    #[test]
    fn test_gate_fail_llm_primary() {
        let gate = VerificationGate::new("strict_gate", Ground::Confirmed);
        let proof = ProofBuilder::new(Ground::Confirmed)
            .add_evidence(Evidence::new(
                "llm says ok",
                Ground::Confirmed,
                b"approved",
                "llm_judge",
            ))
            .build();

        let result = gate.evaluate(&proof);
        assert!(!result.passed);
        assert!(!result.llm_ok);
        assert!(result.details.unwrap().contains("LLM"));
    }

    #[test]
    fn test_gate_accept_llm_when_configured() {
        let gate = VerificationGate::new("lenient_gate", Ground::Confirmed).with_llm_primary(true);

        let proof = ProofBuilder::new(Ground::Confirmed)
            .add_evidence(Evidence::new(
                "llm says ok",
                Ground::Confirmed,
                b"approved",
                "llm_judge",
            ))
            .build();

        let result = gate.evaluate(&proof);
        assert!(result.passed);
        assert!(result.llm_ok);
    }

    #[test]
    fn test_gate_min_independent_sources() {
        let gate = VerificationGate::new("multi_source", Ground::Observed).with_min_sources(2);

        let proof = ProofBuilder::new(Ground::Observed)
            .observe("check", b"data", "single_source")
            .build();

        let result = gate.evaluate(&proof);
        assert!(!result.passed);
        assert!(!result.sources_ok);
    }

    #[test]
    fn test_empty_chain() {
        let proof = ProofBuilder::new(Ground::Observed).build();
        assert!(!proof.passed);
        assert_eq!(proof.achieved_ground, Ground::Assumed);
    }

    #[test]
    fn test_ground_display() {
        assert_eq!(format!("{}", Ground::Assumed), "assumed");
        assert_eq!(format!("{}", Ground::Proven), "proven");
    }

    #[test]
    fn test_evidence_serialization() {
        let ev = Evidence::new("test", Ground::Confirmed, b"data", "src");
        let json = serde_json::to_string(&ev).unwrap();
        let deserialized: Evidence = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.ground, Ground::Confirmed);
        assert_eq!(deserialized.content_hash, ev.content_hash);
    }

    #[test]
    fn test_proof_result_serialization() {
        let proof = ProofBuilder::new(Ground::Confirmed)
            .confirm("ok", b"data", "src")
            .build();
        let json = serde_json::to_string(&proof).unwrap();
        let deserialized: ProofResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.passed, proof.passed);
        assert_eq!(deserialized.chain_hash, proof.chain_hash);
    }

    #[test]
    fn test_weak_evidence() {
        let proof = ProofBuilder::new(Ground::Confirmed)
            .observe("weak", b"data1", "src1")
            .confirm("strong", b"data2", "src2")
            .build();

        let weak = proof.weak_evidence();
        assert_eq!(weak.len(), 1);
        assert_eq!(weak[0].description, "weak");
    }

    #[test]
    fn test_verify_builder_method() {
        let proof = ProofBuilder::new(Ground::Verified)
            .verify("multi-source check", b"verified", "auditor")
            .build();

        assert!(proof.passed);
        assert_eq!(proof.achieved_ground, Ground::Verified);
    }
}
