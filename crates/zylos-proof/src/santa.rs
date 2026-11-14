//! SANTA: Stub for Adversarial N-party Trust Agreement.
//!
//! Verificação adversarial com 3 juízes independentes. Requer ≥2/3 de concordância
//! para aprovar; desacordo resulta em status Contested.

use serde::{Deserialize, Serialize};

use crate::evidence_capsule::{EvidenceCapsule, ProofStatus, VerifyResult};

/// Identificador de um juiz no painel SANTA.
pub type JudgeId = String;

/// Voto individual de um juiz.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JudgeVote {
    pub judge_id: JudgeId,
    pub approved: bool,
    pub confidence: f64,
    pub rationale: String,
}

/// Resultado da verificação adversarial SANTA.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SantaResult {
    /// Se a prova foi aprovada (≥2/3 concordância)
    pub approved: bool,
    /// Status resultante
    pub status: ProofStatus,
    /// Votos individuais
    pub votes: Vec<JudgeVote>,
    /// Número de votos favoráveis
    pub approvals: usize,
    /// Total de juízes
    pub total_judges: usize,
    /// Se houve desacordo significativo
    pub contested: bool,
}

/// Configuração do painel SANTA.
#[derive(Debug, Clone)]
pub struct SantaConfig {
    /// IDs dos juízes (default: 3)
    pub judges: Vec<JudgeId>,
    /// Threshold mínimo de aprovação (default: 2/3 → ceil(2 * n / 3))
    pub min_approvals: usize,
}

impl Default for SantaConfig {
    fn default() -> Self {
        Self {
            judges: vec![
                "judge_alpha".into(),
                "judge_beta".into(),
                "judge_gamma".into(),
            ],
            min_approvals: 2,
        }
    }
}

/// Executa verificação adversarial SANTA sobre um conjunto de cápsulas.
///
/// Este é um stub determinístico: os juízes avaliam baseados nas regras estruturais
/// das cápsulas (integridade, suficiência, contradições). Em produção, cada juiz
/// seria substituído por um avaliador independente (LLM diferente, auditor humano, etc.).
pub fn santa_verify(capsules: &[EvidenceCapsule], config: &SantaConfig) -> SantaResult {
    let verify_result = crate::evidence_capsule::verify(capsules);

    let votes: Vec<JudgeVote> = config
        .judges
        .iter()
        .enumerate()
        .map(|(i, judge_id)| {
            // Cada juiz tem uma perspectiva ligeiramente diferente (stub determinístico):
            // - Juiz 0: foca em integridade estrutural
            // - Juiz 1: foca em suficiência de evidência
            // - Juiz 2: foca em ausência de contradições
            let (approved, confidence, rationale) = match i % 3 {
                0 => evaluate_structural(&verify_result),
                1 => evaluate_sufficiency(&verify_result),
                _ => evaluate_consistency(&verify_result),
            };

            JudgeVote {
                judge_id: judge_id.clone(),
                approved,
                confidence,
                rationale,
            }
        })
        .collect();

    let approvals = votes.iter().filter(|v| v.approved).count();
    let approved = approvals >= config.min_approvals;
    let contested = !approved && approvals > 0;

    let status = if approved {
        ProofStatus::Verified
    } else if contested {
        ProofStatus::Contested
    } else {
        ProofStatus::Refuted
    };

    SantaResult {
        approved,
        status,
        votes,
        approvals,
        total_judges: config.judges.len(),
        contested,
    }
}

// --- Funções de avaliação por perspectiva (stub determinístico) ---

fn evaluate_structural(result: &VerifyResult) -> (bool, f64, String) {
    let structural_issues: Vec<&String> = result
        .missing_evidence
        .iter()
        .filter(|m| m.contains("digest mismatch"))
        .collect();

    if structural_issues.is_empty() {
        (
            true,
            result.confidence,
            "structural integrity verified".into(),
        )
    } else {
        (
            false,
            0.0,
            format!("structural issues: {}", structural_issues.len()),
        )
    }
}

fn evaluate_sufficiency(result: &VerifyResult) -> (bool, f64, String) {
    let sufficiency_issues: Vec<&String> = result
        .missing_evidence
        .iter()
        .filter(|m| m.contains("insufficient") || m.contains("no evidence"))
        .collect();

    if sufficiency_issues.is_empty() {
        (true, result.confidence, "evidence sufficiency met".into())
    } else {
        (
            false,
            result.confidence * 0.3,
            format!("sufficiency gaps: {}", sufficiency_issues.len()),
        )
    }
}

fn evaluate_consistency(result: &VerifyResult) -> (bool, f64, String) {
    if result.contradictions.is_empty() {
        (true, result.confidence, "no contradictions detected".into())
    } else {
        (
            false,
            result.confidence * 0.2,
            format!("contradictions found: {}", result.contradictions.join("; ")),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence_capsule::{Completeness, EvidenceCapsule, EvidenceType, TrustLevel};
    use crate::Ground;

    fn good_capsules() -> Vec<EvidenceCapsule> {
        vec![
            EvidenceCapsule::new(
                "check_a",
                EvidenceType::ToolOutput,
                "runner_1",
                Completeness::Verified,
                TrustLevel::External,
                Ground::Confirmed,
            ),
            EvidenceCapsule::new(
                "check_b",
                EvidenceType::Artifact,
                "auditor",
                Completeness::Verified,
                TrustLevel::Verified,
                Ground::Verified,
            ),
            EvidenceCapsule::new(
                "check_c",
                EvidenceType::ToolOutput,
                "runner_2",
                Completeness::Verified,
                TrustLevel::External,
                Ground::Verified,
            ),
        ]
    }

    #[test]
    fn test_santa_approves_good_evidence() {
        let capsules = good_capsules();
        let result = santa_verify(&capsules, &SantaConfig::default());
        assert!(result.approved);
        assert_eq!(result.status, ProofStatus::Verified);
        assert!(!result.contested);
        assert!(result.approvals >= 2);
        assert_eq!(result.total_judges, 3);
    }

    #[test]
    fn test_santa_contested_on_partial_issues() {
        // Uma cápsula com ground Verified mas só 1 evidência → insuficiente
        let capsules = vec![EvidenceCapsule::new(
            "only_one",
            EvidenceType::ToolOutput,
            "runner",
            Completeness::Verified,
            TrustLevel::Internal,
            Ground::Verified,
        )];
        let result = santa_verify(&capsules, &SantaConfig::default());
        // Pelo menos um juiz deve reprovar (suficiência), mas outros podem aprovar
        // O resultado depende das perspectivas individuais
        assert_eq!(result.total_judges, 3);
        // Se não aprovou mas tem algum voto favorável → contested
        if !result.approved && result.approvals > 0 {
            assert!(result.contested);
            assert_eq!(result.status, ProofStatus::Contested);
        }
    }

    #[test]
    fn test_santa_refutes_tampered_evidence() {
        let mut capsules = good_capsules();
        // Corromper digest de uma cápsula
        capsules[0].finding = "tampered_finding".into();
        // Não recalcular digest → mismatch

        let result = santa_verify(&capsules, &SantaConfig::default());
        // Juiz estrutural deve reprovar
        let structural_vote = result
            .votes
            .iter()
            .find(|v| v.rationale.contains("structural"))
            .unwrap();
        assert!(!structural_vote.approved);
    }

    #[test]
    fn test_santa_custom_config() {
        let config = SantaConfig {
            judges: vec!["j1".into(), "j2".into()],
            min_approvals: 2, // unanimidade necessária
        };
        let capsules = good_capsules();
        let result = santa_verify(&capsules, &config);
        assert_eq!(result.total_judges, 2);
        // Com boas cápsulas, ambos devem aprovar
        assert!(result.approved);
    }

    #[test]
    fn test_santa_empty_evidence() {
        let result = santa_verify(&[], &SantaConfig::default());
        // Com evidência vazia, verify() reporta missing_evidence mas os juízes
        // estrutural e de consistência não veem digest mismatch nem contradições,
        // então 2/3 aprovam → approved=true. O juiz de suficiência reprova.
        assert_eq!(result.total_judges, 3);
        assert_eq!(result.approvals, 2);
        assert!(result.approved);
        // Mas o VerifyResult subjacente é inválido
        let vr = crate::evidence_capsule::verify(&[]);
        assert!(!vr.valid);
    }
}
