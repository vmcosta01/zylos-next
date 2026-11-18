//! Modelo de evento de analytics.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Um evento de execução de skill por um agente.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalyticsEvent {
    /// Id atribuído pelo storage (ausente antes da inserção).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    /// Timestamp Unix (segundos, UTC).
    pub ts: i64,
    /// Identificador do agente/bot.
    pub agent: String,
    /// Identificador da skill executada.
    pub skill: String,
    /// `true` se a execução terminou com sucesso.
    pub success: bool,
    /// Latência da execução em milissegundos.
    pub latency_ms: u64,
    /// Custo estimado em USD (tokens de LLM, APIs externas).
    #[serde(default)]
    pub cost_usd: f64,
    /// Tokens consumidos.
    #[serde(default)]
    pub tokens: u64,
    /// Dimensões customizadas (ex.: `channel=slack`, `model=haiku`).
    #[serde(default)]
    pub dimensions: BTreeMap<String, String>,
}

impl AnalyticsEvent {
    /// Construtor mínimo (custo/tokens zero, sem dimensões).
    pub fn new(
        ts: i64,
        agent: impl Into<String>,
        skill: impl Into<String>,
        success: bool,
        latency_ms: u64,
    ) -> Self {
        Self {
            id: None,
            ts,
            agent: agent.into(),
            skill: skill.into(),
            success,
            latency_ms,
            cost_usd: 0.0,
            tokens: 0,
            dimensions: BTreeMap::new(),
        }
    }

    /// Define o custo (builder).
    pub fn with_cost(mut self, cost_usd: f64, tokens: u64) -> Self {
        self.cost_usd = cost_usd;
        self.tokens = tokens;
        self
    }

    /// Adiciona uma dimensão customizada (builder).
    pub fn with_dim(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.dimensions.insert(key.into(), value.into());
        self
    }

    /// Valida campos obrigatórios.
    pub fn validate(&self) -> crate::Result<()> {
        if self.agent.trim().is_empty() {
            return Err(crate::AnalyticsError::Invalid("agent vazio".into()));
        }
        if self.skill.trim().is_empty() {
            return Err(crate::AnalyticsError::Invalid("skill vazia".into()));
        }
        if !self.cost_usd.is_finite() || self.cost_usd < 0.0 {
            return Err(crate::AnalyticsError::Invalid("cost_usd inválido".into()));
        }
        for k in self.dimensions.keys() {
            crate::filter::validate_dim_key(k)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_rejects_empty_agent_and_bad_dim() {
        assert!(AnalyticsEvent::new(0, "", "s", true, 1).validate().is_err());
        assert!(AnalyticsEvent::new(0, "a", "s", true, 1)
            .with_dim("bad key'", "x")
            .validate()
            .is_err());
        assert!(AnalyticsEvent::new(0, "a", "s", true, 1)
            .with_dim("channel", "slack")
            .validate()
            .is_ok());
    }

    #[test]
    fn serde_roundtrip() {
        let e = AnalyticsEvent::new(10, "a", "s", false, 5).with_cost(0.1, 100);
        let j = serde_json::to_string(&e).unwrap();
        let back: AnalyticsEvent = serde_json::from_str(&j).unwrap();
        assert_eq!(e, back);
    }
}
