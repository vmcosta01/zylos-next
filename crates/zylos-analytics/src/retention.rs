//! Política de retenção configurável (T11.10.6).
//!
//! - Eventos com idade > `archive_after_days` (padrão 30) são movidos de
//!   `events` para `events_archive` (continuam consultáveis via `all_events`
//!   e no rollup, mas saem dos índices quentes).
//! - Eventos com idade > `delete_after_days` (padrão 90) são apagados de
//!   ambas as tabelas **e** do `rollup_hourly`.
//! - Cortes são alinhados para baixo à hora cheia, para que o rollup horário
//!   permaneça consistente com os eventos brutos.
//!
//! Execução: chamar [`AnalyticsStore::apply_retention`] periodicamente (o
//! gateway roda a cada 6h; ver `playbooks/pop-analytics-retention-policy.md`).

use crate::store::AnalyticsStore;
use crate::{AnalyticsError, Result, DAY_SECS};
use rusqlite::params;
use serde::{Deserialize, Serialize};

/// Configuração de retenção.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RetentionPolicy {
    /// Idade (dias) a partir da qual eventos são arquivados. `None` desativa.
    pub archive_after_days: Option<i64>,
    /// Idade (dias) a partir da qual eventos são apagados.
    pub delete_after_days: i64,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            archive_after_days: Some(30),
            delete_after_days: 90,
        }
    }
}

impl RetentionPolicy {
    /// Valida (delete > archive > 0).
    pub fn validate(&self) -> Result<()> {
        if self.delete_after_days < 1 {
            return Err(AnalyticsError::Invalid(
                "delete_after_days deve ser >= 1".into(),
            ));
        }
        if let Some(a) = self.archive_after_days {
            if a < 1 || a >= self.delete_after_days {
                return Err(AnalyticsError::Invalid(
                    "archive_after_days deve estar em [1, delete_after_days)".into(),
                ));
            }
        }
        Ok(())
    }
}

/// Resultado de uma execução de retenção.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetentionReport {
    /// Momento de referência.
    pub ran_at: i64,
    /// Corte de arquivamento (eventos com `ts <` foram arquivados).
    pub archive_cutoff: Option<i64>,
    /// Corte de exclusão.
    pub delete_cutoff: i64,
    /// Eventos movidos para o arquivo.
    pub archived: u64,
    /// Eventos excluídos (quentes + arquivo).
    pub deleted: u64,
    /// Linhas de rollup excluídas.
    pub rollup_rows_deleted: u64,
}

fn hour_floor(t: i64) -> i64 {
    t - t.rem_euclid(3600)
}

impl AnalyticsStore {
    /// Aplica a política com referência em `now` (transação única).
    pub fn apply_retention(
        &mut self,
        policy: &RetentionPolicy,
        now: i64,
    ) -> Result<RetentionReport> {
        policy.validate()?;
        let delete_cutoff = hour_floor(now - policy.delete_after_days * DAY_SECS);
        let archive_cutoff = policy
            .archive_after_days
            .map(|d| hour_floor(now - d * DAY_SECS));
        let tx = self.conn.transaction()?;
        let d_hot = tx.execute("DELETE FROM events WHERE ts < ?1", params![delete_cutoff])?;
        let d_arch = tx.execute(
            "DELETE FROM events_archive WHERE ts < ?1",
            params![delete_cutoff],
        )?;
        let d_roll = tx.execute(
            "DELETE FROM rollup_hourly WHERE hour < ?1",
            params![delete_cutoff],
        )?;
        tx.execute(
            "DELETE FROM rollup_dim_hourly WHERE hour < ?1",
            params![delete_cutoff],
        )?;
        let mut archived = 0usize;
        if let Some(ac) = archive_cutoff {
            archived = tx.execute(
                "INSERT INTO events_archive (id, ts, agent, skill, success, latency_ms, cost_usd, tokens, dims)
                 SELECT id, ts, agent, skill, success, latency_ms, cost_usd, tokens, dims
                 FROM events WHERE ts < ?1",
                params![ac],
            )?;
            tx.execute("DELETE FROM events WHERE ts < ?1", params![ac])?;
        }
        tx.execute(
            "INSERT INTO meta(key, value) VALUES ('retention.last_run', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![now.to_string()],
        )?;
        tx.commit()?;
        Ok(RetentionReport {
            ran_at: now,
            archive_cutoff,
            delete_cutoff,
            archived: archived as u64,
            deleted: (d_hot + d_arch) as u64,
            rollup_rows_deleted: d_roll as u64,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AnalyticsEvent, Filter};

    const NOW: i64 = 1_794_873_600;

    #[test]
    fn archives_30d_and_deletes_90d_synthetic() {
        let mut s = AnalyticsStore::open_in_memory().unwrap();
        // um evento por dia, 0..120 dias atrás
        let evs: Vec<_> = (0..120)
            .map(|d| AnalyticsEvent::new(NOW - d * DAY_SECS - 60, "a", "s", true, 10))
            .collect();
        s.insert_batch(&evs).unwrap();
        let r = s.apply_retention(&RetentionPolicy::default(), NOW).unwrap();
        // idades 90..119 dias (+60s) → apagados: 30
        assert_eq!(r.deleted, 30);
        // idades 30..89 → arquivados: 60
        assert_eq!(r.archived, 60);
        let (hot, arch) = s.table_counts().unwrap();
        assert_eq!((hot, arch), (30, 60));
        // consultas continuam enxergando o arquivo; rollup consistente com bruto
        assert_eq!(s.count(&Filter::all()).unwrap(), 90);
        assert_eq!(s.count(&Filter::all().range(0, NOW)).unwrap(), 90);
        let raw = Filter {
            success: Some(true),
            ..Filter::default()
        };
        assert_eq!(s.count(&raw).unwrap(), 90);
        assert_eq!(
            s.meta_get("retention.last_run").unwrap(),
            Some(NOW.to_string())
        );
        // idempotente
        let r2 = s.apply_retention(&RetentionPolicy::default(), NOW).unwrap();
        assert_eq!((r2.deleted, r2.archived), (0, 0));
    }

    #[test]
    fn configurable_and_validated() {
        let mut s = AnalyticsStore::open_in_memory().unwrap();
        s.insert(&AnalyticsEvent::new(NOW - 10 * DAY_SECS, "a", "s", true, 1))
            .unwrap();
        let p = RetentionPolicy {
            archive_after_days: None,
            delete_after_days: 7,
        };
        let r = s.apply_retention(&p, NOW).unwrap();
        assert_eq!(r.deleted, 1);
        assert!(RetentionPolicy {
            archive_after_days: Some(90),
            delete_after_days: 30
        }
        .validate()
        .is_err());
    }
}
