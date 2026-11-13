//! # SkillMiner — Mineração automática de skills a partir de logs (T8.4.3)
//!
//! Extrai padrões repetidos de logs de execução do gateway e os converte
//! em candidate SKILL.md drafts com contratos formais preenchidos.
//!
//! ## Pipeline
//! 1. Parseia gateway.log buscando sequências input→output repetidas
//! 2. Agrupa ocorrências por similaridade estrutural
//! 3. Gera candidate skill com contract auto-preenchido
//! 4. Submete ao EvalFlywheel para validação (gate obrigatório)
//! 5. Notifica via callback quando candidate é aprovado
//!
//! ## Gate Anti-Falso-Positivo
//! Candidates só são promovidos após aprovação pelo EvalFlywheel.
//! O miner NUNCA cria skills automaticamente sem validação.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::contract::{OutputSchema, Predicate, SkillContract};

/// Um padrão candidato extraído dos logs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CandidatePattern {
    /// Hash identificador do padrão (SHA-256 da assinatura normalizada).
    pub pattern_hash: String,
    /// Sequência de operações detectadas.
    pub operations: Vec<String>,
    /// Número de ocorrências nos logs.
    pub occurrence_count: usize,
    /// Timestamps das ocorrências (ISO 8601).
    pub timestamps: Vec<String>,
    /// Inputs comuns observados.
    pub common_inputs: Vec<String>,
    /// Outputs comuns observados.
    pub common_outputs: Vec<String>,
}

/// Uma skill candidata gerada pelo miner.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateSkill {
    /// Nome sugerido para a skill.
    pub suggested_name: String,
    /// Descrição gerada a partir do padrão.
    pub description: String,
    /// Contrato formal auto-preenchido.
    pub contract: SkillContract,
    /// Padrão subjacente que originou esta candidate.
    pub source_pattern: CandidatePattern,
    /// Score de confiança (0.0-1.0) baseado em frequência e consistência.
    pub confidence_score: f64,
    /// Status da candidate no pipeline de validação.
    pub status: CandidateStatus,
    /// Feedback do EvalFlywheel (se já avaliado).
    pub eval_feedback: Option<String>,
}

/// Status de uma skill candidata no pipeline de validação.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateStatus {
    /// Recém-extraída, aguardando avaliação.
    Pending,
    /// Aprovada pelo EvalFlywheel.
    Approved,
    /// Rejeitada pelo EvalFlywheel.
    Rejected,
    /// Promovida a skill oficial no registry.
    Promoted,
}

/// Configuração do SkillMiner.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinerConfig {
    /// Mínimo de ocorrências para considerar um padrão como candidato.
    pub min_occurrences: usize,
    /// Similaridade mínima para agrupar operações como mesmo padrão.
    pub similarity_threshold: f64,
    /// Máximo de candidates a gerar por ciclo de mineração.
    pub max_candidates_per_cycle: usize,
    /// Regex patterns para identificar operações nos logs.
    pub operation_patterns: Vec<String>,
}

impl Default for MinerConfig {
    fn default() -> Self {
        Self {
            min_occurrences: 3,
            similarity_threshold: 0.8,
            max_candidates_per_cycle: 10,
            operation_patterns: vec![
                r"(?i)executing\s+(?:skill|task|command)\s*[:\s]\s*(.+?)$".into(),
                r"(?i)running\s+(.+?)$".into(),
                r"(?i)invoking\s+tool\s*[:\s]\s*(.+?)$".into(),
                r"(?i)calling\s+(.+?)$".into(),
            ],
        }
    }
}

/// Resultado de um ciclo de mineração.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiningReport {
    /// Número total de linhas de log processadas.
    pub lines_processed: usize,
    /// Número de operações extraídas.
    pub operations_extracted: usize,
    /// Número de padrões únicos identificados.
    pub unique_patterns: usize,
    /// Candidates geradas neste ciclo.
    pub candidates_generated: usize,
    /// Duração do ciclo em milissegundos.
    pub duration_ms: u64,
}

/// Minerador de skills a partir de logs de execução.
pub struct SkillMiner {
    config: MinerConfig,
    compiled_patterns: Vec<Regex>,
}

impl SkillMiner {
    /// Cria um novo miner com a configuração fornecida.
    pub fn new(config: MinerConfig) -> Result<Self, regex::Error> {
        let compiled_patterns: Result<Vec<_>, _> = config
            .operation_patterns
            .iter()
            .map(|p| Regex::new(p))
            .collect();

        Ok(Self {
            config,
            compiled_patterns: compiled_patterns?,
        })
    }

    /// Cria um miner com configuração default.
    pub fn default_config() -> Result<Self, regex::Error> {
        Self::new(MinerConfig::default())
    }

    /// Extrai operações de uma linha de log.
    fn extract_operations(&self, line: &str) -> Vec<String> {
        let mut ops = Vec::new();
        for pattern in &self.compiled_patterns {
            if let Some(captures) = pattern.captures(line) {
                if let Some(op) = captures.get(1) {
                    ops.push(op.as_str().trim().to_string());
                }
            }
        }
        ops
    }

    /// Normaliza uma operação para agrupamento (lowercase, trim whitespace extra).
    fn normalize_operation(op: &str) -> String {
        let trimmed = op.trim();
        // Colapsar múltiplos espaços
        let collapsed: String = trimmed
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        collapsed.to_lowercase()
    }

    /// Computa hash SHA-256 de uma sequência de operações normalizadas.
    fn compute_pattern_hash(operations: &[String]) -> String {
        use sha2::{Digest, Sha256};
        let normalized: Vec<String> = operations.iter().map(|op| Self::normalize_operation(op)).collect();
        let joined = normalized.join("|");
        let mut hasher = Sha256::new();
        hasher.update(joined.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Processa linhas de log e extrai padrões candidatos.
    pub fn mine_from_lines(&self, lines: &[&str]) -> (Vec<CandidatePattern>, MiningReport) {
        let start = std::time::Instant::now();
        let mut operation_sequences: HashMap<String, CandidatePattern> = HashMap::new();
        let mut total_ops = 0;

        // Extrair operações de cada linha
        let mut current_sequence: Vec<String> = Vec::new();
        let mut current_timestamps: Vec<String> = Vec::new();

        for line in lines {
            let ops = self.extract_operations(line);
            if !ops.is_empty() {
                total_ops += ops.len();
                current_sequence.extend(ops);
                // Extrair timestamp se presente (formato ISO 8601 no início da linha)
                if let Some(ts) = extract_timestamp(line) {
                    current_timestamps.push(ts);
                }
            } else if !current_sequence.is_empty() {
                // Fim de uma sequência de operações → agrupar
                let hash = Self::compute_pattern_hash(&current_sequence);
                let entry = operation_sequences
                    .entry(hash.clone())
                    .or_insert_with(|| CandidatePattern {
                        pattern_hash: hash,
                        operations: current_sequence.clone(),
                        occurrence_count: 0,
                        timestamps: Vec::new(),
                        common_inputs: Vec::new(),
                        common_outputs: Vec::new(),
                    });
                entry.occurrence_count += 1;
                entry.timestamps.extend(current_timestamps.iter().cloned());

                current_sequence.clear();
                current_timestamps.clear();
            }
        }

        // Processar última sequência pendente
        if !current_sequence.is_empty() {
            let hash = Self::compute_pattern_hash(&current_sequence);
            let entry = operation_sequences
                .entry(hash.clone())
                .or_insert_with(|| CandidatePattern {
                    pattern_hash: hash,
                    operations: current_sequence.clone(),
                    occurrence_count: 0,
                    timestamps: Vec::new(),
                    common_inputs: Vec::new(),
                    common_outputs: Vec::new(),
                });
            entry.occurrence_count += 1;
            entry.timestamps.extend(current_timestamps.iter().cloned());
        }

        // Filtrar por min_occurrences
        let candidates: Vec<CandidatePattern> = operation_sequences
            .into_values()
            .filter(|p| p.occurrence_count >= self.config.min_occurrences)
            .collect();

        let report = MiningReport {
            lines_processed: lines.len(),
            operations_extracted: total_ops,
            unique_patterns: candidates.len(),
            candidates_generated: candidates.len().min(self.config.max_candidates_per_cycle),
            duration_ms: start.elapsed().as_millis() as u64,
        };

        (candidates, report)
    }

    /// Gera uma CandidateSkill a partir de um CandidatePattern.
    pub fn generate_candidate_skill(&self, pattern: &CandidatePattern) -> CandidateSkill {
        // Gerar nome a partir das operações
        let name_parts: Vec<&str> = pattern
            .operations
            .iter()
            .take(3)
            .map(|op| {
                let first_word = op.split_whitespace().next().unwrap_or("unknown");
                first_word
            })
            .collect();
        let suggested_name = if name_parts.is_empty() {
            "auto-mined-skill".to_string()
        } else {
            format!("auto-{}", name_parts.join("-"))
        };

        // Gerar descrição
        let description = format!(
            "Auto-mined skill from {} occurrences of pattern: {}",
            pattern.occurrence_count,
            pattern.operations.join(" → ")
        );

        // Auto-preencher contrato
        let mut contract = SkillContract::new();

        // Adicionar outputs baseados nas operações
        for (i, op) in pattern.operations.iter().enumerate() {
            contract.outputs.push(
                OutputSchema::new(
                    format!("step_{}_output", i + 1),
                    "object",
                )
                .with_description(format!("Output of: {}", op))
                .optional(),
            );
        }

        // Adicionar precondition genérica
        contract.preconditions.push(Predicate::new(
            "log_pattern_verified",
            vec![pattern.pattern_hash.clone()],
        ));

        // Calcular confidence score baseada em frequência e consistência
        let frequency_score = (pattern.occurrence_count as f64 / 10.0).min(1.0);
        let consistency_score = if pattern.timestamps.len() > 1 { 0.8 } else { 0.5 };
        let confidence = (frequency_score * 0.6 + consistency_score * 0.4).min(1.0);

        CandidateSkill {
            suggested_name,
            description,
            contract,
            source_pattern: pattern.clone(),
            confidence_score: confidence,
            status: CandidateStatus::Pending,
            eval_feedback: None,
        }
    }

    /// Executa um ciclo completo de mineração: extrai padrões e gera candidates.
    pub fn run_mining_cycle(&self, log_lines: &[&str]) -> (Vec<CandidateSkill>, MiningReport) {
        let (patterns, report) = self.mine_from_lines(log_lines);

        let candidates: Vec<CandidateSkill> = patterns
            .iter()
            .take(self.config.max_candidates_per_cycle)
            .map(|p| self.generate_candidate_skill(p))
            .collect();

        (candidates, report)
    }
}

/// Extrai timestamp ISO 8601 do início de uma linha de log.
fn extract_timestamp(line: &str) -> Option<String> {
    // Match patterns like "2026-11-11T00:00:00Z" or "2026-11-11 00:00:00"
    let ts_regex = Regex::new(r"^(\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:?\d{2})?)").ok()?;
    ts_regex.captures(line).map(|c| c[1].to_string())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_miner() -> SkillMiner {
        SkillMiner::default_config().unwrap()
    }

    #[test]
    fn test_extract_operations() {
        let miner = test_miner();
        let ops = miner.extract_operations("2026-11-11T00:00:00Z executing skill: analyze-code");
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0], "analyze-code");
    }

    #[test]
    fn test_extract_operations_no_match() {
        let miner = test_miner();
        let ops = miner.extract_operations("just a regular log line with no operations");
        assert!(ops.is_empty());
    }

    #[test]
    fn test_normalize_operation() {
        assert_eq!(
            SkillMiner::normalize_operation("  Analyze  Code  "),
            "analyze code"
        );
        assert_eq!(
            SkillMiner::normalize_operation("RUNNING   TESTS"),
            "running tests"
        );
    }

    #[test]
    fn test_compute_pattern_hash_deterministic() {
        let ops = vec!["build".into(), "test".into(), "deploy".into()];
        let h1 = SkillMiner::compute_pattern_hash(&ops);
        let h2 = SkillMiner::compute_pattern_hash(&ops);
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_compute_pattern_hash_different_order() {
        let ops1 = vec!["build".into(), "test".into()];
        let ops2 = vec!["test".into(), "build".into()];
        assert_ne!(
            SkillMiner::compute_pattern_hash(&ops1),
            SkillMiner::compute_pattern_hash(&ops2)
        );
    }

    #[test]
    fn test_mine_from_lines_basic() {
        let miner = SkillMiner::new(MinerConfig {
            min_occurrences: 2,
            ..MinerConfig::default()
        })
        .unwrap();

        let lines = vec![
            "2026-11-11T00:00:00Z executing skill: build-project",
            "2026-11-11T00:00:01Z executing skill: run-tests",
            "some other log line",
            "2026-11-11T00:01:00Z executing skill: build-project",
            "2026-11-11T00:01:01Z executing skill: run-tests",
        ];

        let (patterns, report) = miner.mine_from_lines(&lines);
        assert_eq!(report.lines_processed, 5);
        assert_eq!(report.operations_extracted, 4);
        // Deve encontrar 1 padrão repetido (build-project → run-tests)
        assert_eq!(patterns.len(), 1);
        assert_eq!(patterns[0].occurrence_count, 2);
    }

    #[test]
    fn test_mine_filters_below_threshold() {
        let miner = SkillMiner::new(MinerConfig {
            min_occurrences: 5,
            ..MinerConfig::default()
        })
        .unwrap();

        let lines = vec![
            "executing skill: one-off-task",
            "executing skill: another-task",
        ];

        let (patterns, _) = miner.mine_from_lines(&lines);
        assert!(patterns.is_empty()); // Abaixo do threshold de 5
    }

    #[test]
    fn test_generate_candidate_skill() {
        let miner = test_miner();
        let pattern = CandidatePattern {
            pattern_hash: "abc123".into(),
            operations: vec!["build".into(), "test".into(), "deploy".into()],
            occurrence_count: 10,
            timestamps: vec!["2026-11-11T00:00:00Z".into()],
            common_inputs: vec![],
            common_outputs: vec![],
        };

        let candidate = miner.generate_candidate_skill(&pattern);
        assert!(candidate.suggested_name.starts_with("auto-"));
        assert!(candidate.description.contains("10 occurrences"));
        assert_eq!(candidate.status, CandidateStatus::Pending);
        assert!(candidate.confidence_score > 0.0);
        assert!(candidate.contract.outputs.len() == 3);
        assert!(candidate.contract.preconditions.len() == 1);
    }

    #[test]
    fn test_run_mining_cycle() {
        let miner = SkillMiner::new(MinerConfig {
            min_occurrences: 1,
            max_candidates_per_cycle: 5,
            ..MinerConfig::default()
        })
        .unwrap();

        let lines = vec![
            "executing skill: compile-rust",
            "executing skill: run-clippy",
        ];

        let (candidates, report) = miner.run_mining_cycle(&lines);
        assert_eq!(report.lines_processed, 2);
        assert!(candidates.len() <= 5);
        for c in &candidates {
            assert_eq!(c.status, CandidateStatus::Pending);
            assert!(c.confidence_score >= 0.0 && c.confidence_score <= 1.0);
        }
    }

    #[test]
    fn test_extract_timestamp() {
        assert_eq!(
            extract_timestamp("2026-11-11T00:00:00Z some log"),
            Some("2026-11-11T00:00:00Z".into())
        );
        assert_eq!(
            extract_timestamp("2026-11-11 12:30:45 some log"),
            Some("2026-11-11 12:30:45".into())
        );
        assert!(extract_timestamp("no timestamp here").is_none());
    }

    #[test]
    fn test_candidate_status_serialization() {
        for status in [
            CandidateStatus::Pending,
            CandidateStatus::Approved,
            CandidateStatus::Rejected,
            CandidateStatus::Promoted,
        ] {
            let json = serde_json::to_string(&status).unwrap();
            let restored: CandidateStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(status, restored);
        }
    }

    #[test]
    fn test_mining_report_serialization() {
        let report = MiningReport {
            lines_processed: 1000,
            operations_extracted: 50,
            unique_patterns: 5,
            candidates_generated: 3,
            duration_ms: 150,
        };
        let json = serde_json::to_string(&report).unwrap();
        let restored: MiningReport = serde_json::from_str(&json).unwrap();
        assert_eq!(report.lines_processed, restored.lines_processed);
        assert_eq!(report.candidates_generated, restored.candidates_generated);
    }

    #[test]
    fn test_confidence_score_high_frequency() {
        let miner = test_miner();
        let pattern = CandidatePattern {
            pattern_hash: "h".into(),
            operations: vec!["op".into()],
            occurrence_count: 100,
            timestamps: vec!["t1".into(), "t2".into(), "t3".into()],
            common_inputs: vec![],
            common_outputs: vec![],
        };
        let candidate = miner.generate_candidate_skill(&pattern);
        // Alta frequência + múltiplos timestamps → alta confiança
        assert!(candidate.confidence_score >= 0.8);
    }

    #[test]
    fn test_confidence_score_low_frequency() {
        let miner = test_miner();
        let pattern = CandidatePattern {
            pattern_hash: "h".into(),
            operations: vec!["op".into()],
            occurrence_count: 1,
            timestamps: vec!["t1".into()],
            common_inputs: vec![],
            common_outputs: vec![],
        };
        let candidate = miner.generate_candidate_skill(&pattern);
        // Baixa frequência + único timestamp → baixa confiança
        assert!(candidate.confidence_score < 0.6);
    }

    #[test]
    fn test_empty_log_produces_no_candidates() {
        let miner = test_miner();
        let (candidates, report) = miner.run_mining_cycle(&[]);
        assert!(candidates.is_empty());
        assert_eq!(report.lines_processed, 0);
        assert_eq!(report.operations_extracted, 0);
    }

    #[test]
    fn test_max_candidates_limit() {
        let miner = SkillMiner::new(MinerConfig {
            min_occurrences: 1,
            max_candidates_per_cycle: 2,
            ..MinerConfig::default()
        })
        .unwrap();

        // Criar muitas operações únicas
        let _lines: Vec<&str> = (0..20)
            .map(|_| "executing skill: unique-task")
            .collect::<Vec<_>>();
        // Nota: todas são iguais, então será 1 padrão. Vamos variar:
        let varied_lines: Vec<String> = (0..20)
            .map(|i| format!("executing skill: task-{}", i))
            .collect();
        let refs: Vec<&str> = varied_lines.iter().map(|s| s.as_str()).collect();

        let (candidates, _) = miner.run_mining_cycle(&refs);
        assert!(candidates.len() <= 2);
    }
}