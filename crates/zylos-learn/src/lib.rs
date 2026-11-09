//! # zylos-learn
//!
//! Loop Aprender→Persistir→Aplicar com classificação de desfechos,
//! recorrência mecânica e witness anti-regressão.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Errors produced by the learning loop subsystem.
#[derive(Debug, Error)]
pub enum LearnError {
    /// Outcome payload could not be parsed or classified.
    #[error("desfecho inválido: {0}")]
    InvalidOutcome(String),
    /// One or more witness checks failed.
    #[error("witness check falhou: {0}")]
    WitnessFailed(String),
    /// Filesystem I/O error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Classificação de desfecho.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutcomeClass {
    /// Desfecho bom (pronto e limpo) → null (não inventa lição)
    Sucesso,
    /// Problema que exigiu escalonamento
    Escalacao,
    /// Falha comprovada por evidência
    FalhaProvada,
    /// Lacuna de capacidade detectada
    LacunaCapacidade,
}

/// Desfecho registrado.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Outcome {
    pub task_id: String,
    pub agent_id: String,
    pub classification: OutcomeClass,
    pub signature: String,
    pub detail: String,
    pub timestamp: String,
}

/// Entrada pendente em JSONL append-only.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingLesson {
    pub signature: String,
    pub classification: OutcomeClass,
    pub occurrences: usize,
    pub first_seen: String,
    pub last_seen: String,
    pub matured: bool,
}

/// Witness anti-regressão: marcador-em-arquivo + check determinístico.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Witness {
    pub marker_path: String,
    pub expected_content: String,
    pub check_type: WitnessCheckType,
}

/// Strategy used by a witness to verify anti-regression markers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WitnessCheckType {
    /// Marker file must exist on disk.
    FileExists,
    /// Marker file must contain the expected content substring.
    ContentMatch,
    /// Marker script must exit with code zero.
    ExitCodeZero,
}

/// Loop de aprendizado com recorrência mecânica.
pub struct LearningLoop {
    pending: HashMap<String, PendingLesson>,
    witnesses: Vec<Witness>,
    /// Limiar de recorrência para lição madura.
    pub recurrence_threshold: usize,
    jsonl_path: PathBuf,
}

impl LearningLoop {
    /// Create a new learning loop backed by the given JSONL file.
    pub fn new(jsonl_path: impl Into<PathBuf>) -> Self {
        Self {
            pending: HashMap::new(),
            witnesses: Vec::new(),
            recurrence_threshold: 3,
            jsonl_path: jsonl_path.into(),
        }
    }

    /// Assinatura normalizada para agrupamento por recorrência.
    /// Agrupa variações textuais do mesmo problema.
    pub fn normalize_signature(text: &str) -> String {
        let mut tokens: Vec<String> = text
            .to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty() && s.len() > 2)
            .map(|s| s.to_string())
            .collect();
        tokens.sort();
        tokens.dedup();
        let joined = tokens.join(":");
        // Hash determinístico da assinatura normalizada
        let mut hasher = Sha256::new();
        hasher.update(joined.as_bytes());
        format!("{:x}", hasher.finalize())[..16].to_string()
    }

    /// Registra desfecho com classificação.
    /// Desfecho bom (Sucesso) → retorna None (não inventa lição).
    /// FalhaProvada/Escalacao/LacunaCapacidade → retorna candidato.
    pub fn record_outcome(&mut self, outcome: &Outcome) -> Option<&PendingLesson> {
        if outcome.classification == OutcomeClass::Sucesso {
            return None;
        }

        let sig = if outcome.signature.is_empty() {
            Self::normalize_signature(&outcome.detail)
        } else {
            outcome.signature.clone()
        };

        let entry = self.pending.entry(sig.clone()).or_insert_with(|| PendingLesson {
            signature: sig.clone(),
            classification: outcome.classification,
            occurrences: 0,
            first_seen: outcome.timestamp.clone(),
            last_seen: outcome.timestamp.clone(),
            matured: false,
        });

        entry.occurrences += 1;
        entry.last_seen = outcome.timestamp.clone();

        // Gate de recorrência: >=3x → lição madura
        if entry.occurrences >= self.recurrence_threshold {
            entry.matured = true;
        }

        Some(entry)
    }

    /// Append-only JSONL: adiciona entrada pendente ao arquivo.
    pub fn append_pending(&self, lesson: &PendingLesson) -> Result<(), LearnError> {
        use std::io::Write;
        let json = serde_json::to_string(lesson)
            .map_err(|e| LearnError::InvalidOutcome(e.to_string()))?;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.jsonl_path)?;
        writeln!(file, "{}", json)?;
        Ok(())
    }

    /// Retorna todas as lições maduras (recorrência >= limiar).
    pub fn matured_lessons(&self) -> Vec<&PendingLesson> {
        self.pending.values().filter(|l| l.matured).collect()
    }

    /// Registra witness anti-regressão.
    pub fn add_witness(&mut self, witness: Witness) {
        self.witnesses.push(witness);
    }

    /// Check determinístico de witnesses (zero-exec, exit-code based).
    pub fn check_witnesses(&self, base_dir: &Path) -> Result<Vec<String>, LearnError> {
        let mut failures = Vec::new();
        for w in &self.witnesses {
            let full_path = base_dir.join(&w.marker_path);
            match w.check_type {
                WitnessCheckType::FileExists => {
                    if !full_path.exists() {
                        failures.push(format!("witness ausente: {}", w.marker_path));
                    }
                }
                WitnessCheckType::ContentMatch => {
                    if full_path.exists() {
                        let content = std::fs::read_to_string(&full_path)?;
                        if !content.contains(&w.expected_content) {
                            failures.push(format!("witness conteúdo diverge: {}", w.marker_path));
                        }
                    } else {
                        failures.push(format!("witness arquivo não existe: {}", w.marker_path));
                    }
                }
                WitnessCheckType::ExitCodeZero => {
                    // Placeholder: em produção, executaria comando e checaria exit code
                    if !full_path.exists() {
                        failures.push(format!("witness script ausente: {}", w.marker_path));
                    }
                }
            }
        }
        if failures.is_empty() { Ok(failures) } else { Err(LearnError::WitnessFailed(failures.join("; "))) }
    }

    /// Number of pending lessons currently tracked.
        pub fn pending_count(&self) -> usize { self.pending.len() }
    }

#[cfg(test)]
mod tests {
    use super::*;

    fn make_outcome(classification: OutcomeClass, detail: &str) -> Outcome {
        Outcome {
            task_id: "t1".into(),
            agent_id: "agent-a".into(),
            classification,
            signature: String::new(),
            detail: detail.into(),
            timestamp: "2026-11-03T00:00:00Z".into(),
        }
    }

    #[test]
    fn test_sucesso_returns_null() {
        let mut ll = LearningLoop::new("/tmp/test.jsonl");
        let outcome = make_outcome(OutcomeClass::Sucesso, "tudo ok");
        assert!(ll.record_outcome(&outcome).is_none());
        assert_eq!(ll.pending_count(), 0);
    }

    #[test]
    fn test_falha_provada_returns_candidate() {
        let mut ll = LearningLoop::new("/tmp/test.jsonl");
        let outcome = make_outcome(OutcomeClass::FalhaProvada, "timeout no deploy");
        let result = ll.record_outcome(&outcome);
        assert!(result.is_some());
        let lesson = result.unwrap();
        assert_eq!(lesson.occurrences, 1);
        assert!(!lesson.matured);
    }

    #[test]
    fn test_recurrence_gate_3x() {
        let mut ll = LearningLoop::new("/tmp/test.jsonl");
        for _i in 0..3 {
            let outcome = make_outcome(OutcomeClass::Escalacao, "timeout deploy error");
            ll.record_outcome(&outcome);
        }
        let matured = ll.matured_lessons();
        assert_eq!(matured.len(), 1);
        assert!(matured[0].matured);
        assert_eq!(matured[0].occurrences, 3);
    }

    #[test]
    fn test_normalized_signature_groups_variations() {
        let s1 = LearningLoop::normalize_signature("deploy timeout error");
        let s2 = LearningLoop::normalize_signature("timeout error deploy");
        let s3 = LearningLoop::normalize_signature("error timeout deploy");
        assert_eq!(s1, s2);
        assert_eq!(s2, s3);
    }

    #[test]
    fn test_normalized_signature_differentiates_distinct() {
        let s1 = LearningLoop::normalize_signature("deploy timeout error");
        let s2 = LearningLoop::normalize_signature("build compilation failure");
        assert_ne!(s1, s2);
    }

    #[test]
    fn test_witness_file_exists_check() {
        let mut ll = LearningLoop::new("/tmp/test.jsonl");
        ll.add_witness(Witness {
            marker_path: "test_marker.txt".into(),
            expected_content: String::new(),
            check_type: WitnessCheckType::FileExists,
        });
        // Check against non-existent dir should fail
        let result = ll.check_witnesses(Path::new("/tmp/nonexistent_dir_12345"));
        assert!(result.is_err());
    }

    #[test]
    fn test_lacuna_capacidade_classification() {
        let mut ll = LearningLoop::new("/tmp/test.jsonl");
        let outcome = make_outcome(OutcomeClass::LacunaCapacidade, "no tool for X");
        let result = ll.record_outcome(&outcome);
        assert!(result.is_some());
        assert_eq!(result.unwrap().classification, OutcomeClass::LacunaCapacidade);
    }

    #[test]
    fn test_pending_lesson_append_jsonl() {
        let dir = std::env::temp_dir();
        let path = dir.join("zylos_learn_test.jsonl");
        let ll = LearningLoop::new(&path);
        let lesson = PendingLesson {
            signature: "abc123".into(),
            classification: OutcomeClass::FalhaProvada,
            occurrences: 2,
            first_seen: "2026-11-03T00:00:00Z".into(),
            last_seen: "2026-11-03T01:00:00Z".into(),
            matured: false,
        };
        assert!(ll.append_pending(&lesson).is_ok());
        assert!(path.exists());
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("abc123"));
        // Cleanup
        let _ = std::fs::remove_file(&path);
    }
}
