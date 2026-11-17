//! Filtros avançados (T11.10.3): período, agente, skill, sucesso, latência,
//! dimensões customizadas e predicado arbitrário em Rust.
//!
//! Um [`Filter`] é serializável (usado pela API do gateway) e é compilado para
//! uma cláusula `WHERE` parametrizada — valores nunca são interpolados no SQL.
//! Chaves de dimensão são validadas (`[A-Za-z0-9_.-]`, até 64 chars) porque
//! entram no path JSON de `json_extract`.
//!
//! Predicados customizados (`Fn(&AnalyticsEvent) -> bool`) não são
//! serializáveis; são aplicados em memória via
//! [`crate::AnalyticsStore::aggregate_with_predicate`] e
//! [`crate::AnalyticsStore::query_events_with`].

use crate::event::AnalyticsEvent;
use crate::{AnalyticsError, Result};
use rusqlite::types::Value;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Filtro declarativo sobre eventos.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Filter {
    /// Início inclusivo (Unix s).
    #[serde(default)]
    pub since: Option<i64>,
    /// Fim exclusivo (Unix s).
    #[serde(default)]
    pub until: Option<i64>,
    /// Agentes aceitos (vazio = todos).
    #[serde(default)]
    pub agents: Vec<String>,
    /// Skills aceitas (vazio = todas).
    #[serde(default)]
    pub skills: Vec<String>,
    /// Somente sucesso (`Some(true)`) ou falha (`Some(false)`).
    #[serde(default)]
    pub success: Option<bool>,
    /// Latência mínima inclusiva (ms).
    #[serde(default)]
    pub min_latency_ms: Option<u64>,
    /// Latência máxima inclusiva (ms).
    #[serde(default)]
    pub max_latency_ms: Option<u64>,
    /// Igualdade em dimensões customizadas (`chave = valor`, AND entre chaves).
    #[serde(default)]
    pub dimensions: BTreeMap<String, String>,
}

/// Valida uma chave de dimensão customizada.
pub fn validate_dim_key(key: &str) -> Result<()> {
    let ok = !key.is_empty()
        && key.len() <= 64
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'));
    if ok {
        Ok(())
    } else {
        Err(AnalyticsError::Invalid(format!(
            "chave de dimensão inválida: {key:?}"
        )))
    }
}

impl Filter {
    /// Filtro vazio (tudo).
    pub fn all() -> Self {
        Self::default()
    }

    /// Define o intervalo `[since, until)`.
    pub fn range(mut self, since: i64, until: i64) -> Self {
        self.since = Some(since);
        self.until = Some(until);
        self
    }

    /// Adiciona um agente.
    pub fn agent(mut self, a: impl Into<String>) -> Self {
        self.agents.push(a.into());
        self
    }

    /// Adiciona uma skill.
    pub fn skill(mut self, s: impl Into<String>) -> Self {
        self.skills.push(s.into());
        self
    }

    /// Adiciona igualdade de dimensão.
    pub fn dim(mut self, k: impl Into<String>, v: impl Into<String>) -> Self {
        self.dimensions.insert(k.into(), v.into());
        self
    }

    /// Valida o filtro.
    pub fn validate(&self) -> Result<()> {
        if let (Some(s), Some(u)) = (self.since, self.until) {
            if s > u {
                return Err(AnalyticsError::Invalid("since > until".into()));
            }
        }
        if let (Some(a), Some(b)) = (self.min_latency_ms, self.max_latency_ms) {
            if a > b {
                return Err(AnalyticsError::Invalid(
                    "min_latency_ms > max_latency_ms".into(),
                ));
            }
        }
        for k in self.dimensions.keys() {
            validate_dim_key(k)?;
        }
        Ok(())
    }

    /// `true` se o filtro pode ser respondido pela tabela de rollup horário
    /// (sem filtros por evento individual e com limites alinhados à hora).
    pub fn rollup_compatible(&self) -> bool {
        let aligned = |t: Option<i64>| t.is_none_or(|v| v.rem_euclid(3600) == 0);
        self.success.is_none()
            && self.min_latency_ms.is_none()
            && self.max_latency_ms.is_none()
            && self.dimensions.is_empty()
            && aligned(self.since)
            && aligned(self.until)
    }

    /// `true` se o filtro pode ser respondido por `rollup_dim_hourly`:
    /// como [`Filter::rollup_compatible`], mas com exatamente uma dimensão.
    pub fn dim_rollup_compatible(&self) -> bool {
        self.dimensions.len() == 1
            && Filter {
                dimensions: BTreeMap::new(),
                ..self.clone()
            }
            .rollup_compatible()
    }

    /// Compila para `WHERE ...` parametrizado. `ts_col` é a coluna de tempo
    /// (`ts` na tabela bruta, `hour` no rollup). Quando `rollup` é `true`,
    /// apenas período/agente/skill são emitidos.
    pub fn to_sql(&self, ts_col: &str, rollup: bool) -> Result<(String, Vec<Value>)> {
        self.validate()?;
        let mut clauses: Vec<String> = Vec::new();
        let mut params: Vec<Value> = Vec::new();
        if let Some(s) = self.since {
            clauses.push(format!("{ts_col} >= ?"));
            params.push(Value::Integer(s));
        }
        if let Some(u) = self.until {
            clauses.push(format!("{ts_col} < ?"));
            params.push(Value::Integer(u));
        }
        push_in(&mut clauses, &mut params, "agent", &self.agents);
        push_in(&mut clauses, &mut params, "skill", &self.skills);
        if !rollup {
            if let Some(ok) = self.success {
                clauses.push("success = ?".into());
                params.push(Value::Integer(ok as i64));
            }
            if let Some(v) = self.min_latency_ms {
                clauses.push("latency_ms >= ?".into());
                params.push(Value::Integer(v as i64));
            }
            if let Some(v) = self.max_latency_ms {
                clauses.push("latency_ms <= ?".into());
                params.push(Value::Integer(v as i64));
            }
            for (k, v) in &self.dimensions {
                // chave validada acima — seguro no path JSON
                clauses.push(format!("json_extract(dims, '$.\"{k}\"') = ?"));
                params.push(Value::Text(v.clone()));
            }
        }
        let sql = if clauses.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", clauses.join(" AND "))
        };
        Ok((sql, params))
    }

    /// Avaliação em memória (equivalente semântico de [`Filter::to_sql`]).
    pub fn matches(&self, e: &AnalyticsEvent) -> bool {
        self.since.is_none_or(|s| e.ts >= s)
            && self.until.is_none_or(|u| e.ts < u)
            && (self.agents.is_empty() || self.agents.contains(&e.agent))
            && (self.skills.is_empty() || self.skills.contains(&e.skill))
            && self.success.is_none_or(|s| s == e.success)
            && self.min_latency_ms.is_none_or(|m| e.latency_ms >= m)
            && self.max_latency_ms.is_none_or(|m| e.latency_ms <= m)
            && self
                .dimensions
                .iter()
                .all(|(k, v)| e.dimensions.get(k) == Some(v))
    }

    /// Resumo legível (usado como metadata em exports).
    pub fn describe(&self) -> String {
        let fmt_ts = |t: i64| {
            chrono::DateTime::from_timestamp(t, 0)
                .map(|d| d.format("%Y-%m-%d %H:%M UTC").to_string())
                .unwrap_or_else(|| t.to_string())
        };
        let mut parts = Vec::new();
        if let Some(s) = self.since {
            parts.push(format!("desde {}", fmt_ts(s)));
        }
        if let Some(u) = self.until {
            parts.push(format!("até {}", fmt_ts(u)));
        }
        if !self.agents.is_empty() {
            parts.push(format!("agentes [{}]", self.agents.join(", ")));
        }
        if !self.skills.is_empty() {
            parts.push(format!("skills [{}]", self.skills.join(", ")));
        }
        if let Some(s) = self.success {
            parts.push(
                if s {
                    "somente sucesso"
                } else {
                    "somente falhas"
                }
                .into(),
            );
        }
        if let Some(v) = self.min_latency_ms {
            parts.push(format!("latência >= {v}ms"));
        }
        if let Some(v) = self.max_latency_ms {
            parts.push(format!("latência <= {v}ms"));
        }
        for (k, v) in &self.dimensions {
            parts.push(format!("{k}={v}"));
        }
        if parts.is_empty() {
            "sem filtros".into()
        } else {
            parts.join("; ")
        }
    }
}

fn push_in(clauses: &mut Vec<String>, params: &mut Vec<Value>, col: &str, vals: &[String]) {
    if vals.is_empty() {
        return;
    }
    let marks = vec!["?"; vals.len()].join(",");
    clauses.push(format!("{col} IN ({marks})"));
    params.extend(vals.iter().map(|v| Value::Text(v.clone())));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sql_is_parametrized() {
        let f = Filter::all()
            .range(0, 3600)
            .agent("a'; DROP TABLE events;--")
            .dim("channel", "slack");
        let (sql, params) = f.to_sql("ts", false).unwrap();
        assert!(!sql.contains("DROP"));
        assert_eq!(params.len(), 4);
        assert!(sql.contains("json_extract"));
    }

    #[test]
    fn invalid_dim_key_rejected() {
        let f = Filter::all().dim("x') OR 1=1 --", "v");
        assert!(f.to_sql("ts", false).is_err());
    }

    #[test]
    fn rollup_compat() {
        assert!(Filter::all().range(3600, 7200).rollup_compatible());
        assert!(!Filter::all().range(3601, 7200).rollup_compatible());
        assert!(!Filter::all().dim("k", "v").rollup_compatible());
        assert!(Filter::all().dim("k", "v").dim_rollup_compatible());
        assert!(Filter::all()
            .range(3600, 7200)
            .agent("a")
            .dim("k", "v")
            .dim_rollup_compatible());
        assert!(!Filter::all()
            .dim("k", "v")
            .dim("j", "w")
            .dim_rollup_compatible());
        assert!(!Filter::all()
            .range(3601, 7200)
            .dim("k", "v")
            .dim_rollup_compatible());
        let mut f = Filter::all().dim("k", "v");
        f.success = Some(true);
        assert!(!f.dim_rollup_compatible());
        assert!(!Filter::all().dim_rollup_compatible());
    }

    #[test]
    fn matches_in_memory() {
        let e = AnalyticsEvent::new(100, "a", "s", true, 50).with_dim("channel", "slack");
        assert!(Filter::all().agent("a").dim("channel", "slack").matches(&e));
        assert!(!Filter::all().skill("x").matches(&e));
        assert!(!Filter::all().range(0, 100).matches(&e));
    }
}
