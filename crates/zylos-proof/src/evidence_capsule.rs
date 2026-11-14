//! Evidence Capsule: unidade content-addressable de evidência com verificação em três fases,
//! composição transitiva de confiança e refutação estruturada.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

/// Tipo de evidência contida na cápsula.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EvidenceType {
    /// Artefato produzido (arquivo, binário, relatório)
    Artifact,
    /// Saída de ferramenta (test runner, linter, compilador)
    ToolOutput,
    /// Aprovação humana explícita
    HumanApproval,
    /// Inferência derivada de outras evidências
    Inference,
}

/// Grau de completude da evidência.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Completeness {
    /// Evidência parcial ou incompleta
    Partial,
    /// Evidência verificada e completa
    Verified,
    /// Evidência contradita por outra fonte
    Contradicted,
}

/// Nível de confiança da fonte da evidência.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum TrustLevel {
    /// Fonte não confiável ou desconhecida
    Untrusted = 0,
    /// Fonte interna do sistema
    Internal = 1,
    /// Fonte verificada por auditoria
    Verified = 2,
    /// Fonte externa independente
    External = 3,
}

/// Status de uma prova no ciclo de vida adversarial.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProofStatus {
    Draft,
    Verified,
    Contested,
    Refuted,
    Superseded,
}

/// Tipo de refutação aplicada a uma cápsula.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RebuttalType {
    FactualError,
    LogicalFlaw,
    OutdatedEvidence,
    ScopeViolation,
}

/// Resultado da verificação em três fases.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyResult {
    pub valid: bool,
    pub confidence: f64,
    pub missing_evidence: Vec<String>,
    pub contradictions: Vec<String>,
}

/// Cápsula de evidência content-addressable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceCapsule {
    pub finding: String,
    pub evidence_type: EvidenceType,
    pub source_id: String,
    pub digest: [u8; 32],
    pub completeness: Completeness,
    pub trust_boundary: TrustLevel,
    /// Ground associado (integração com o sistema existente)
    pub ground: super::Ground,
    /// Referências a outras cápsulas que esta refuta (bidirecional)
    #[serde(default)]
    pub refutes: Vec<[u8; 32]>,
    /// Referências a cápsulas que refutam esta (bidirecional)
    #[serde(default)]
    pub refuted_by: Vec<[u8; 32]>,
    /// Status atual da cápsula
    pub status: ProofStatus,
}

impl EvidenceCapsule {
    /// Cria nova cápsula com digest computado a partir da serialização canônica.
    pub fn new(
        finding: impl Into<String>,
        evidence_type: EvidenceType,
        source_id: impl Into<String>,
        completeness: Completeness,
        trust_boundary: TrustLevel,
        ground: super::Ground,
    ) -> Self {
        let finding = finding.into();
        let source_id = source_id.into();

        // Digest é computado sobre os campos canônicos (excluindo o próprio digest e links mutáveis)
        let digest = Self::compute_digest(
            &finding,
            evidence_type,
            &source_id,
            completeness,
            trust_boundary,
            ground,
        );

        Self {
            finding,
            evidence_type,
            source_id,
            digest,
            completeness,
            trust_boundary,
            ground,
            refutes: Vec::new(),
            refuted_by: Vec::new(),
            status: ProofStatus::Draft,
        }
    }

    /// Computa SHA-256 da representação canônica JSON dos campos imutáveis.
    pub fn compute_digest(
        finding: &str,
        evidence_type: EvidenceType,
        source_id: &str,
        completeness: Completeness,
        trust_boundary: TrustLevel,
        ground: super::Ground,
    ) -> [u8; 32] {
        // Serialização canônica: ordem fixa de campos, sem espaços extras
        let canonical = serde_json::json!({
            "finding": finding,
            "evidence_type": evidence_type,
            "source_id": source_id,
            "completeness": completeness,
            "trust_boundary": trust_boundary,
            "ground": ground,
        });
        let bytes = serde_json::to_vec(&canonical).expect("canonical serialization must not fail");
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let result = hasher.finalize();
        let mut digest = [0u8; 32];
        digest.copy_from_slice(&result);
        digest
    }

    /// Verifica se o digest armazenado corresponde aos campos atuais.
    pub fn verify_digest(&self) -> bool {
        let expected = Self::compute_digest(
            &self.finding,
            self.evidence_type,
            &self.source_id,
            self.completeness,
            self.trust_boundary,
            self.ground,
        );
        self.digest == expected
    }

    /// Retorna o digest como string hex.
    pub fn digest_hex(&self) -> String {
        self.digest.iter().map(|b| format!("{:02x}", b)).collect()
    }
}

/// Verificação em três fases de um conjunto de cápsulas.
pub fn verify(capsules: &[EvidenceCapsule]) -> VerifyResult {
    let mut missing_evidence = Vec::new();
    let mut contradictions = Vec::new();
    let mut all_valid = true;

    // Fase 1: Integridade estrutural (schema + digests)
    for capsule in capsules {
        if !capsule.verify_digest() {
            all_valid = false;
            missing_evidence.push(format!("digest mismatch on capsule '{}'", capsule.finding));
        }
    }

    // Fase 2: Suficiência de evidência (mínimo por ground)
    // Requer pelo menos 1 cápsula por nível de ground presente, e >= 2 para Verified+
    let ground_counts = capsules
        .iter()
        .fold(std::collections::HashMap::new(), |mut acc, c| {
            *acc.entry(c.ground).or_insert(0usize) += 1;
            acc
        });

    for (ground, count) in &ground_counts {
        if *ground >= super::Ground::Verified && *count < 2 {
            all_valid = false;
            missing_evidence.push(format!(
                "insufficient evidence for ground {}: need >= 2, have {}",
                ground, count
            ));
        }
    }

    if capsules.is_empty() {
        all_valid = false;
        missing_evidence.push("no evidence capsules provided".into());
    }

    // Fase 3: Detecção de contradições
    // Cápsulas com mesmo finding mas completeness=Contradicted indicam conflito
    let mut findings_seen: std::collections::HashMap<&str, Vec<&EvidenceCapsule>> =
        std::collections::HashMap::new();
    for c in capsules {
        findings_seen.entry(c.finding.as_str()).or_default().push(c);
    }
    for (finding, group) in &findings_seen {
        let has_contradicted = group
            .iter()
            .any(|c| c.completeness == Completeness::Contradicted);
        let has_verified = group
            .iter()
            .any(|c| c.completeness == Completeness::Verified);
        if has_contradicted && has_verified {
            contradictions.push(format!("contradiction on finding '{}'", finding));
            all_valid = false;
        }
    }

    // Confiança baseada na proporção de cápsulas válidas e níveis de trust
    let confidence = if capsules.is_empty() {
        0.0
    } else {
        let trust_sum: f64 = capsules
            .iter()
            .map(|c| match c.trust_boundary {
                TrustLevel::Untrusted => 0.0,
                TrustLevel::Internal => 0.5,
                TrustLevel::Verified => 0.8,
                TrustLevel::External => 1.0,
            })
            .sum();
        let base = trust_sum / capsules.len() as f64;
        if contradictions.is_empty() {
            base
        } else {
            base * 0.5 // penaliza contradições
        }
    };

    VerifyResult {
        valid: all_valid,
        confidence,
        missing_evidence,
        contradictions,
    }
}

/// Compõe múltiplas cápsulas propagando confiança transitivamente.
/// - Se independentes (fontes distintas): trust = min(trusts)
/// - Se corroborativas (mesma fonte, achados compatíveis): trust = max(trusts)
/// - Detecta composição circular via rastreamento de digests visitados.
pub fn compose(capsules: &[EvidenceCapsule]) -> Result<EvidenceCapsule, ComposeError> {
    if capsules.is_empty() {
        return Err(ComposeError::Empty);
    }

    // Detecção de ciclo: se algum digest aparece mais de uma vez na entrada
    let mut seen = HashSet::new();
    for c in capsules {
        if !seen.insert(c.digest) {
            return Err(ComposeError::CircularComposition(c.digest_hex()));
        }
    }

    // Determinar se são independentes ou corroborativos
    let sources: HashSet<&str> = capsules.iter().map(|c| c.source_id.as_str()).collect();
    let independent = sources.len() > 1;

    let composed_trust = if independent {
        capsules
            .iter()
            .map(|c| c.trust_boundary)
            .min()
            .unwrap_or(TrustLevel::Untrusted)
    } else {
        capsules
            .iter()
            .map(|c| c.trust_boundary)
            .max()
            .unwrap_or(TrustLevel::Untrusted)
    };

    let composed_ground = if independent {
        capsules
            .iter()
            .map(|c| c.ground)
            .min()
            .unwrap_or(super::Ground::Assumed)
    } else {
        capsules
            .iter()
            .map(|c| c.ground)
            .max()
            .unwrap_or(super::Ground::Assumed)
    };

    let composed_completeness = if capsules
        .iter()
        .any(|c| c.completeness == Completeness::Contradicted)
    {
        Completeness::Contradicted
    } else if capsules
        .iter()
        .all(|c| c.completeness == Completeness::Verified)
    {
        Completeness::Verified
    } else {
        Completeness::Partial
    };

    let findings: Vec<&str> = capsules.iter().map(|c| c.finding.as_str()).collect();
    let composed_finding = format!("composed({})", findings.join(", "));

    let composed_source = if independent {
        format!(
            "compose_independent({})",
            sources.into_iter().collect::<Vec<_>>().join(",")
        )
    } else {
        format!("compose_corroborative({})", capsules[0].source_id)
    };

    Ok(EvidenceCapsule::new(
        composed_finding,
        EvidenceType::Inference,
        composed_source,
        composed_completeness,
        composed_trust,
        composed_ground,
    ))
}

/// Erros possíveis na composição.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComposeError {
    Empty,
    CircularComposition(String),
}

impl std::fmt::Display for ComposeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComposeError::Empty => write!(f, "cannot compose empty capsule set"),
            ComposeError::CircularComposition(digest) => {
                write!(
                    f,
                    "circular composition detected: duplicate digest {}",
                    digest
                )
            }
        }
    }
}

impl std::error::Error for ComposeError {}

/// Cria uma refutação estruturada bidirecional entre duas cápsulas.
/// Retorna a cápsula de refutação e modifica ambas as entradas para manter links bidirecionais.
pub fn refute(
    target: &mut EvidenceCapsule,
    counter_evidence: EvidenceCapsule,
    rebuttal_type: RebuttalType,
) -> EvidenceCapsule {
    let rebuttal_finding = format!(
        "rebuttal({:?}): {}",
        rebuttal_type, counter_evidence.finding
    );

    let mut rebuttal_capsule = EvidenceCapsule::new(
        rebuttal_finding,
        EvidenceType::Inference,
        counter_evidence.source_id.clone(),
        Completeness::Verified,
        counter_evidence.trust_boundary,
        counter_evidence.ground,
    );

    // Link bidirecional
    rebuttal_capsule.refutes.push(target.digest);
    target.refuted_by.push(rebuttal_capsule.digest);
    target.status = ProofStatus::Refuted;
    rebuttal_capsule.status = ProofStatus::Verified;

    rebuttal_capsule
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Ground;

    fn sample_capsule(finding: &str, ground: Ground, trust: TrustLevel) -> EvidenceCapsule {
        EvidenceCapsule::new(
            finding,
            EvidenceType::ToolOutput,
            "test_runner",
            Completeness::Verified,
            trust,
            ground,
        )
    }

    #[test]
    fn test_capsule_digest_deterministic() {
        let c1 = sample_capsule("tests pass", Ground::Confirmed, TrustLevel::Internal);
        let c2 = sample_capsule("tests pass", Ground::Confirmed, TrustLevel::Internal);
        assert_eq!(c1.digest, c2.digest);
        assert!(c1.verify_digest());
    }

    #[test]
    fn test_capsule_digest_changes_with_content() {
        let c1 = sample_capsule("tests pass", Ground::Confirmed, TrustLevel::Internal);
        let c2 = sample_capsule("tests fail", Ground::Confirmed, TrustLevel::Internal);
        assert_ne!(c1.digest, c2.digest);
    }

    #[test]
    fn test_verify_all_phases_pass() {
        let capsules = vec![
            sample_capsule("check_a", Ground::Confirmed, TrustLevel::Internal),
            sample_capsule("check_b", Ground::Verified, TrustLevel::External),
            sample_capsule("check_c", Ground::Verified, TrustLevel::Verified),
        ];
        let result = verify(&capsules);
        assert!(result.valid);
        assert!(result.missing_evidence.is_empty());
        assert!(result.contradictions.is_empty());
        assert!(result.confidence > 0.0);
    }

    #[test]
    fn test_verify_detects_digest_tamper() {
        let mut capsules = vec![sample_capsule(
            "check",
            Ground::Confirmed,
            TrustLevel::Internal,
        )];
        capsules[0].finding = "tampered".into();
        let result = verify(&capsules);
        assert!(!result.valid);
        assert!(result
            .missing_evidence
            .iter()
            .any(|m| m.contains("digest mismatch")));
    }

    #[test]
    fn test_verify_detects_insufficient_evidence() {
        let capsules = vec![sample_capsule(
            "only_one",
            Ground::Verified,
            TrustLevel::Internal,
        )];
        let result = verify(&capsules);
        assert!(!result.valid);
        assert!(result
            .missing_evidence
            .iter()
            .any(|m| m.contains("insufficient")));
    }

    #[test]
    fn test_verify_detects_contradiction() {
        let mut c1 = sample_capsule("same_finding", Ground::Confirmed, TrustLevel::Internal);
        let mut c2 = sample_capsule("same_finding", Ground::Confirmed, TrustLevel::Internal);
        c1.completeness = Completeness::Verified;
        c2.completeness = Completeness::Contradicted;
        // Precisamos de digests diferentes para não serem filtradas como duplicatas
        c2.source_id = "other_source".into();
        c2.digest = EvidenceCapsule::compute_digest(
            &c2.finding,
            c2.evidence_type,
            &c2.source_id,
            c2.completeness,
            c2.trust_boundary,
            c2.ground,
        );
        let result = verify(&[c1, c2]);
        assert!(!result.valid);
        assert!(!result.contradictions.is_empty());
    }

    #[test]
    fn test_compose_independent_min_trust() {
        let c1 = sample_capsule("a", Ground::Confirmed, TrustLevel::External);
        let c2 = EvidenceCapsule::new(
            "b",
            EvidenceType::Artifact,
            "auditor",
            Completeness::Verified,
            TrustLevel::Internal,
            Ground::Verified,
        );
        let composed = compose(&[c1, c2]).unwrap();
        assert_eq!(composed.trust_boundary, TrustLevel::Internal); // min
        assert_eq!(composed.ground, Ground::Confirmed); // min
        assert!(composed.finding.contains("composed"));
    }

    #[test]
    fn test_compose_corroborative_max_trust() {
        let c1 = sample_capsule("a", Ground::Confirmed, TrustLevel::Internal);
        let c2 = EvidenceCapsule::new(
            "b",
            EvidenceType::ToolOutput,
            "test_runner", // mesma fonte → corroborativo
            Completeness::Verified,
            TrustLevel::External,
            Ground::Verified,
        );
        let composed = compose(&[c1, c2]).unwrap();
        assert_eq!(composed.trust_boundary, TrustLevel::External); // max
        assert_eq!(composed.ground, Ground::Verified); // max
    }

    #[test]
    fn test_compose_detects_circular() {
        let c = sample_capsule("a", Ground::Confirmed, TrustLevel::Internal);
        let result = compose(&[c.clone(), c]);
        assert!(matches!(result, Err(ComposeError::CircularComposition(_))));
    }

    #[test]
    fn test_compose_empty_error() {
        let result = compose(&[]);
        assert_eq!(result, Err(ComposeError::Empty));
    }

    #[test]
    fn test_refute_bidirectional_link() {
        let mut target = sample_capsule("original claim", Ground::Confirmed, TrustLevel::Internal);
        let counter = sample_capsule("counter evidence", Ground::Verified, TrustLevel::External);

        let rebuttal = refute(&mut target, counter, RebuttalType::FactualError);

        assert_eq!(target.status, ProofStatus::Refuted);
        assert_eq!(rebuttal.status, ProofStatus::Verified);
        assert!(target.refuted_by.contains(&rebuttal.digest));
        assert!(rebuttal.refutes.contains(&target.digest));
        assert!(rebuttal.finding.contains("FactualError"));
    }

    #[test]
    fn test_content_addressable_storage() {
        let c = sample_capsule("immutable fact", Ground::Proven, TrustLevel::External);
        let hex = c.digest_hex();
        assert_eq!(hex.len(), 64); // SHA-256 = 32 bytes = 64 hex chars
        assert!(c.verify_digest());
    }
}
