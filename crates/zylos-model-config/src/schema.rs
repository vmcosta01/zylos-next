//! Schema de dados para Multi-Model Selector
//!
//! Tipos serializáveis para persistência SQLite e comunicação via API REST.
//! Suporta hierarquia de resolução: conversation > bot > global default.

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Estratégia de tier para seleção automática de modelo
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum TierStrategy {
    /// Modelo mais barato disponível
    Cheapest,
    /// Equilíbrio entre custo e qualidade (padrão)
    #[default]
    Balanced,
    /// Modelo mais rápido disponível
    Fastest,
    /// Modelo mais inteligente disponível
    Smartest,
}

impl TierStrategy {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Cheapest => "cheapest",
            Self::Balanced => "balanced",
            Self::Fastest => "fastest",
            Self::Smartest => "smartest",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "cheapest" => Some(Self::Cheapest),
            "balanced" => Some(Self::Balanced),
            "fastest" => Some(Self::Fastest),
            "smartest" => Some(Self::Smartest),
            _ => None,
        }
    }
}

/// Nível de esforço de raciocínio do modelo
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum ReasoningEffort {
    Low,
    #[default]
    Medium,
    High,
    Auto,
}

impl ReasoningEffort {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Auto => "auto",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            "auto" => Some(Self::Auto),
            _ => None,
        }
    }
}

/// Configuração de modelo para um bot ou contexto global
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ModelConfig {
    pub id: String,
    pub bot_id: String,   // "__global__" para config global
    pub provider: String, // openai, anthropic, ollama, etc.
    pub model_id: String, // gpt-4o, claude-3.5-sonnet, etc.
    pub tier_strategy: String,
    pub reasoning_effort: String,
    pub max_tokens: Option<i64>,
    pub temperature: Option<f64>,
    pub is_default: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// Configuração efetiva resolvida (após herança)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedModelConfig {
    pub provider: String,
    pub model_id: String,
    pub tier_strategy: String,
    pub reasoning_effort: String,
    pub max_tokens: Option<i64>,
    pub temperature: Option<f64>,
    pub source: String, // "bot", "global", "default"
}

/// Request body para criar/atualizar config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertModelConfig {
    pub provider: String,
    pub model_id: String,
    pub tier_strategy: Option<String>,
    pub reasoning_effort: Option<String>,
    pub max_tokens: Option<i64>,
    pub temperature: Option<f64>,
    pub is_default: Option<bool>,
}

/// Modelo disponível listado por provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailableModel {
    pub provider: String,
    pub model_id: String,
    pub display_name: String,
    pub context_window: Option<i64>,
    pub supports_vision: bool,
    pub supports_tools: bool,
    pub cost_per_million_input: Option<f64>,
    pub cost_per_million_output: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier_strategy_roundtrip() {
        let strategies = [
            TierStrategy::Cheapest,
            TierStrategy::Balanced,
            TierStrategy::Fastest,
            TierStrategy::Smartest,
        ];
        for s in strategies {
            let str_val = s.as_str();
            let parsed = TierStrategy::parse(str_val).unwrap();
            assert_eq!(s, parsed);
        }
    }

    #[test]
    fn test_reasoning_effort_roundtrip() {
        let efforts = [
            ReasoningEffort::Low,
            ReasoningEffort::Medium,
            ReasoningEffort::High,
            ReasoningEffort::Auto,
        ];
        for e in efforts {
            let str_val = e.as_str();
            let parsed = ReasoningEffort::parse(str_val).unwrap();
            assert_eq!(e, parsed);
        }
    }

    #[test]
    fn test_defaults() {
        assert_eq!(TierStrategy::default(), TierStrategy::Balanced);
        assert_eq!(ReasoningEffort::default(), ReasoningEffort::Medium);
    }

    #[test]
    fn test_invalid_from_str_returns_none() {
        assert!(TierStrategy::parse("invalid").is_none());
        assert!(ReasoningEffort::parse("invalid").is_none());
    }

    #[test]
    fn test_upsert_serialization() {
        let upsert = UpsertModelConfig {
            provider: "openai".into(),
            model_id: "gpt-4o".into(),
            tier_strategy: Some("balanced".into()),
            reasoning_effort: Some("high".into()),
            max_tokens: Some(4096),
            temperature: Some(0.7),
            is_default: Some(true),
        };
        let json = serde_json::to_string(&upsert).unwrap();
        assert!(json.contains("gpt-4o"));
        assert!(json.contains("balanced"));
    }
}
