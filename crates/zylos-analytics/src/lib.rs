//! # zylos-analytics — Analytics de uso de agentes e skills (Fase 11.10)
//!
//! Crate responsável por coletar, armazenar e analisar eventos de execução
//! (`agente`, `skill`, `sucesso`, `latência`, `custo`, `timestamp`).
//!
//! Módulos:
//! - [`event`] — modelo de evento ([`AnalyticsEvent`]).
//! - [`store`] — armazenamento SQLite ([`AnalyticsStore`]) com índices por tempo/skill/agente.
//! - [`aggregations`] — agregações temporais hourly/daily/weekly/monthly, breakdowns e heatmap (T11.10.1).
//! - [`filter`] — filtros avançados: período, agente, skill, dimensões customizadas e predicado (T11.10.3).
//! - [`export`] + [`pdf`] — export CSV e PDF A4 (writer PDF próprio, sem dependências) (T11.10.4).
//! - [`feedback`] — feedback loop de auto-melhoria: detecta skills com baixa taxa de sucesso (T11.10.5).
//! - [`retention`] — política de retenção configurável: archive >30d, delete >90d (T11.10.6).
//! - [`reports`] — relatórios preset (Weekly Bot Performance, Monthly Cost Analysis, Skill Evolution Summary) (T11.10.7).
//! - [`synthetic`] — gerador determinístico de dados sintéticos para testes e benchmark (T11.10.8).
//!
//! Timestamps são sempre segundos Unix em UTC (`i64`).

pub mod aggregations;
pub mod event;
pub mod export;
pub mod feedback;
pub mod filter;
pub mod pdf;
pub mod reports;
pub mod retention;
pub mod store;
pub mod synthetic;

pub use aggregations::{Bucket, Granularity, GroupBy, GroupStats, HeatmapCell};
pub use event::AnalyticsEvent;
pub use feedback::{FeedbackConfig, FeedbackReport, Recommendation, RecommendationKind, Severity};
pub use filter::Filter;
pub use reports::{PresetReport, Report};
pub use retention::{RetentionPolicy, RetentionReport};
pub use store::AnalyticsStore;

/// Erros do crate de analytics.
#[derive(Debug, thiserror::Error)]
pub enum AnalyticsError {
    /// Erro do SQLite.
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// Erro de (de)serialização JSON.
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    /// Entrada inválida (filtro, parâmetro, dimensão).
    #[error("entrada inválida: {0}")]
    Invalid(String),
}

/// Resultado padrão do crate.
pub type Result<T> = std::result::Result<T, AnalyticsError>;

/// Segundos em um dia.
pub const DAY_SECS: i64 = 86_400;
