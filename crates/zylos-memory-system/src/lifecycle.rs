//! # MemoryLifecycle — Retain/Consolidate/Refresh lifecycle management (T8.2.1)
//!
//! Implementa o ciclo de vida explícito de memória inspirado em Hindsight:
//! - **Retain**: Remove entradas fora da política de retenção (idade, relevância).
//! - **Consolidate**: Merge entradas duplicadas/similares dentro de uma janela temporal.
//! - **Refresh**: Recarrega embeddings stale e atualiza scores de relevância.
//!
//! ## Scheduling
//! Em produção, usar `tokio::time::Interval` ou cron para executar periodicamente.
//! Este módulo fornece a lógica pura; o scheduling é responsabilidade do caller.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Política de retenção de memórias.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RetentionPolicy {
    /// Idade máxima de uma entrada antes de ser candidata a remoção.
    /// Entradas mais recentes que isso são sempre mantidas.
    pub max_age: Duration,
    /// Score mínimo de relevância para manter entradas antigas.
    /// Entradas com score >= threshold sobrevivem mesmo se antigas.
    pub min_relevance_score: f64,
    /// Se true, entradas marcadas como "pinned" nunca são removidas.
    pub respect_pinned: bool,
    /// Número máximo de entradas a remover por ciclo de retain.
    pub max_removals_per_cycle: usize,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            max_age: Duration::from_secs(30 * 24 * 60 * 60), // 30 dias
            min_relevance_score: 0.3,
            respect_pinned: true,
            max_removals_per_cycle: 100,
        }
    }
}

/// Janela temporal para consolidação de entradas similares.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConsolidationWindow {
    /// Duração da janela: entradas dentro desta janela são candidatas a merge.
    pub duration: Duration,
    /// Similaridade mínima (cosine) para considerar duas entradas como duplicatas.
    pub similarity_threshold: f64,
    /// Se true, mantém a entrada mais recente como sobrevivente.
    pub keep_newest: bool,
}

impl Default for ConsolidationWindow {
    fn default() -> Self {
        Self {
            duration: Duration::from_secs(24 * 60 * 60), // 24 horas
            similarity_threshold: 0.92,
            keep_newest: true,
        }
    }
}

/// Gatilho para refresh de embeddings/relevância.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RefreshTrigger {
    /// Refresh periódico baseado em intervalo.
    Periodic { interval: Duration },
    /// Refresh sob demanda (manual ou via evento).
    OnDemand,
    /// Refresh quando embedding model muda.
    ModelChanged { new_model_version: String },
}

impl Default for RefreshTrigger {
    fn default() -> Self {
        Self::Periodic {
            interval: Duration::from_secs(7 * 24 * 60 * 60), // 7 dias
        }
    }
}

/// Relatório de um ciclo de retain.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RetainReport {
    /// Número de entradas avaliadas.
    pub entries_evaluated: usize,
    /// Número de entradas removidas.
    pub entries_removed: usize,
    /// Número de entradas mantidas por serem recentes.
    pub kept_recent: usize,
    /// Número de entradas mantidas por relevance score.
    pub kept_relevant: usize,
    /// Número de entradas mantidas por estarem pinned.
    pub kept_pinned: usize,
    /// Duração do ciclo em milissegundos.
    pub duration_ms: u64,
}

/// Relatório de um ciclo de consolidação.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConsolidationReport {
    /// Número de grupos de duplicatas encontrados.
    pub duplicate_groups_found: usize,
    /// Número total de entradas consolidadas (removidas após merge).
    pub entries_consolidated: usize,
    /// Número de entradas sobreviventes (após merge).
    pub entries_surviving: usize,
    /// Similaridade média dos grupos consolidados.
    pub avg_similarity: f64,
    /// Duração do ciclo em milissegundos.
    pub duration_ms: u64,
}

/// Relatório de um ciclo de refresh.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RefreshReport {
    /// Número de embeddings recalculados.
    pub embeddings_refreshed: usize,
    /// Número de scores de relevância atualizados.
    pub relevance_scores_updated: usize,
    /// Número de erros durante refresh.
    pub errors: usize,
    /// Duração do ciclo em milissegundos.
    pub duration_ms: u64,
}

/// Trait para gerenciamento de ciclo de vida de memória.
///
/// Implementações devem ser Send + Sync para uso em background tasks Tokio.
/// O ContextGraph existente deve implementar esta trait.
pub trait MemoryLifecycle: Send + Sync {
    /// Remove entradas que não atendem à política de retenção.
    ///
    /// Retorna relatório com contagens detalhadas de entradas removidas/mantidas.
    fn retain(
        &self,
        policy: &RetentionPolicy,
    ) -> impl std::future::Future<Output = Result<RetainReport, crate::MemoryError>> + Send;

    /// Consolida entradas duplicadas/similares dentro da janela temporal.
    ///
    /// Merge combina conteúdo e metadata, mantendo a entrada mais relevante/recente.
    fn consolidate(
        &self,
        window: &ConsolidationWindow,
    ) -> impl std::future::Future<Output = Result<ConsolidationReport, crate::MemoryError>> + Send;

    /// Recarrega embeddings stale e atualiza scores de relevância.
    ///
    /// Trigger define o motivo do refresh (periódico, on-demand, model change).
    fn refresh(
        &self,
        trigger: &RefreshTrigger,
    ) -> impl std::future::Future<Output = Result<RefreshReport, crate::MemoryError>> + Send;
}

/// Configuração completa do lifecycle para serialização em config.toml.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifecycleConfig {
    /// Política de retenção.
    pub retain: RetentionPolicy,
    /// Janela de consolidação.
    pub consolidation: ConsolidationWindow,
    /// Gatilho de refresh.
    pub refresh: RefreshTrigger,
    /// Schedule de execução automática (cron expression ou intervalo).
    pub schedule: Option<String>,
    /// Se true, lifecycle roda automaticamente em background.
    pub auto_run: bool,
}

impl Default for LifecycleConfig {
    fn default() -> Self {
        Self {
            retain: RetentionPolicy::default(),
            consolidation: ConsolidationWindow::default(),
            refresh: RefreshTrigger::default(),
            schedule: Some("0 3 * * *".into()), // 3 AM daily
            auto_run: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retention_policy_default() {
        let policy = RetentionPolicy::default();
        assert_eq!(policy.max_age, Duration::from_secs(30 * 24 * 60 * 60));
        assert!((policy.min_relevance_score - 0.3).abs() < f64::EPSILON);
        assert!(policy.respect_pinned);
        assert_eq!(policy.max_removals_per_cycle, 100);
    }

    #[test]
    fn test_consolidation_window_default() {
        let window = ConsolidationWindow::default();
        assert_eq!(window.duration, Duration::from_secs(24 * 60 * 60));
        assert!((window.similarity_threshold - 0.92).abs() < f64::EPSILON);
        assert!(window.keep_newest);
    }

    #[test]
    fn test_refresh_trigger_default() {
        let trigger = RefreshTrigger::default();
        match trigger {
            RefreshTrigger::Periodic { interval } => {
                assert_eq!(interval, Duration::from_secs(7 * 24 * 60 * 60));
            }
            _ => panic!("Expected Periodic variant"),
        }
    }

    #[test]
    fn test_lifecycle_config_default() {
        let config = LifecycleConfig::default();
        assert!(config.auto_run);
        assert_eq!(config.schedule, Some("0 3 * * *".into()));
    }

    #[test]
    fn test_retain_report_serialization() {
        let report = RetainReport {
            entries_evaluated: 1000,
            entries_removed: 50,
            kept_recent: 800,
            kept_relevant: 100,
            kept_pinned: 50,
            duration_ms: 250,
        };

        let json = serde_json::to_string(&report).unwrap();
        let restored: RetainReport = serde_json::from_str(&json).unwrap();
        assert_eq!(report, restored);
    }

    #[test]
    fn test_consolidation_report_serialization() {
        let report = ConsolidationReport {
            duplicate_groups_found: 15,
            entries_consolidated: 30,
            entries_surviving: 15,
            avg_similarity: 0.95,
            duration_ms: 180,
        };

        let json = serde_json::to_string(&report).unwrap();
        let restored: ConsolidationReport = serde_json::from_str(&json).unwrap();
        assert_eq!(report, restored);
    }

    #[test]
    fn test_refresh_report_serialization() {
        let report = RefreshReport {
            embeddings_refreshed: 200,
            relevance_scores_updated: 200,
            errors: 0,
            duration_ms: 5000,
        };

        let json = serde_json::to_string(&report).unwrap();
        let restored: RefreshReport = serde_json::from_str(&json).unwrap();
        assert_eq!(report, restored);
    }

    #[test]
    fn test_refresh_trigger_variants() {
        let periodic = RefreshTrigger::Periodic {
            interval: Duration::from_secs(3600),
        };
        let on_demand = RefreshTrigger::OnDemand;
        let model_changed = RefreshTrigger::ModelChanged {
            new_model_version: "v2.0".into(),
        };

        // Verificar serialização roundtrip para cada variante
        for trigger in [periodic, on_demand, model_changed] {
            let json = serde_json::to_string(&trigger).unwrap();
            let restored: RefreshTrigger = serde_json::from_str(&json).unwrap();
            assert_eq!(trigger, restored);
        }
    }

    #[test]
    fn test_lifecycle_config_custom() {
        let config = LifecycleConfig {
            retain: RetentionPolicy {
                max_age: Duration::from_secs(7 * 24 * 60 * 60),
                min_relevance_score: 0.5,
                respect_pinned: false,
                max_removals_per_cycle: 50,
            },
            consolidation: ConsolidationWindow {
                duration: Duration::from_secs(12 * 60 * 60),
                similarity_threshold: 0.85,
                keep_newest: false,
            },
            refresh: RefreshTrigger::OnDemand,
            schedule: None,
            auto_run: false,
        };

        let json = serde_json::to_string_pretty(&config).unwrap();
        let restored: LifecycleConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(
            restored.retain.max_age,
            Duration::from_secs(7 * 24 * 60 * 60)
        );
        assert!(!restored.auto_run);
        assert!(restored.schedule.is_none());
    }

    #[test]
    fn test_retain_report_metrics_consistency() {
        let report = RetainReport {
            entries_evaluated: 500,
            entries_removed: 100,
            kept_recent: 200,
            kept_relevant: 150,
            kept_pinned: 50,
            duration_ms: 100,
        };

        // entries_removed + kept_* deve igualar entries_evaluated
        let total_kept = report.kept_recent + report.kept_relevant + report.kept_pinned;
        assert_eq!(
            report.entries_removed + total_kept,
            report.entries_evaluated
        );
    }
}
