//! Agregações temporais (T11.10.1): hourly / daily / weekly / monthly,
//! breakdowns por agente/skill, heatmap dia-da-semana × hora e resumo.
//!
//! ## Estratégia
//! - Filtros compatíveis com rollup ([`Filter::rollup_compatible`]) são
//!   respondidos pela tabela `rollup_hourly` (≤ 1 linha por hora×agente×skill),
//!   independentemente do volume bruto de eventos.
//! - Demais filtros (sucesso, latência, dimensões, limites não alinhados)
//!   agregam sobre `all_events` direto em SQL.
//! - Predicados customizados em Rust usam
//!   [`AnalyticsStore::aggregate_with_predicate`] (varredura em memória).
//!
//! ## Convenções de bucket (UTC)
//! - `Hourly`: início da hora; `Daily`: 00:00 UTC.
//! - `Weekly`: segunda-feira 00:00 UTC (ISO-8601). O epoch Unix (1970-01-01)
//!   foi quinta-feira, daí `ts - ((ts + 3·86400) mod 604800)`.
//! - `Monthly`: dia 1 00:00 UTC (calendário real via `strftime`/chrono).
//!
//! Timestamps negativos (pré-1970) não são suportados.

use crate::event::AnalyticsEvent;
use crate::filter::Filter;
use crate::store::{AnalyticsStore, Source};
use crate::{AnalyticsError, Result};
use chrono::{Datelike, TimeZone, Utc};
use rusqlite::params_from_iter;
use rusqlite::types::Value;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Granularidade temporal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Granularity {
    /// Por hora.
    Hourly,
    /// Por dia.
    Daily,
    /// Por semana ISO (segunda-feira).
    Weekly,
    /// Por mês de calendário.
    Monthly,
}

impl Granularity {
    /// Parse a partir de string (`hourly|daily|weekly|monthly`, também `hour|day|week|month`).
    pub fn parse(s: &str) -> Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "hourly" | "hour" | "h" => Ok(Self::Hourly),
            "daily" | "day" | "d" => Ok(Self::Daily),
            "weekly" | "week" | "w" => Ok(Self::Weekly),
            "monthly" | "month" | "m" => Ok(Self::Monthly),
            other => Err(AnalyticsError::Invalid(format!(
                "granularidade desconhecida: {other}"
            ))),
        }
    }

    /// Expressão SQL que mapeia `col` (Unix s) para o início do bucket.
    fn sql_bucket(self, col: &str) -> String {
        match self {
            Self::Hourly => format!("({col} - ({col} % 3600))"),
            Self::Daily => format!("({col} - ({col} % 86400))"),
            Self::Weekly => format!("({col} - (({col} + 259200) % 604800))"),
            Self::Monthly => {
                format!("CAST(strftime('%s', {col}, 'unixepoch', 'start of month') AS INTEGER)")
            }
        }
    }

    /// Início do bucket que contém `ts`.
    pub fn bucket_start(self, ts: i64) -> i64 {
        match self {
            Self::Hourly => ts - ts.rem_euclid(3600),
            Self::Daily => ts - ts.rem_euclid(86_400),
            Self::Weekly => ts - (ts + 259_200).rem_euclid(604_800),
            Self::Monthly => {
                let d = Utc.timestamp_opt(ts, 0).single().unwrap_or_default();
                Utc.with_ymd_and_hms(d.year(), d.month(), 1, 0, 0, 0)
                    .single()
                    .map(|x| x.timestamp())
                    .unwrap_or(ts)
            }
        }
    }

    /// Início do bucket seguinte a `start` (que deve ser um início de bucket).
    pub fn next_bucket(self, start: i64) -> i64 {
        match self {
            Self::Hourly => start + 3600,
            Self::Daily => start + 86_400,
            Self::Weekly => start + 604_800,
            Self::Monthly => {
                let d = Utc.timestamp_opt(start, 0).single().unwrap_or_default();
                let (y, m) = if d.month() == 12 {
                    (d.year() + 1, 1)
                } else {
                    (d.year(), d.month() + 1)
                };
                Utc.with_ymd_and_hms(y, m, 1, 0, 0, 0)
                    .single()
                    .map(|x| x.timestamp())
                    .unwrap_or(start + 31 * 86_400)
            }
        }
    }
}

/// Métricas de um bucket temporal.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Bucket {
    /// Início do bucket (Unix s, UTC).
    pub start: i64,
    /// Total de execuções.
    pub count: u64,
    /// Execuções com sucesso.
    pub success: u64,
    /// `success / count` (0 quando vazio).
    pub success_rate: f64,
    /// Latência média (ms).
    pub avg_latency_ms: f64,
    /// Latência máxima (ms).
    pub max_latency_ms: u64,
    /// Custo total (USD).
    pub cost_usd: f64,
    /// Tokens totais.
    pub tokens: u64,
}

impl Bucket {
    fn finish(mut self, lat_sum: f64) -> Self {
        if self.count > 0 {
            self.success_rate = self.success as f64 / self.count as f64;
            self.avg_latency_ms = lat_sum / self.count as f64;
        }
        self
    }
}

/// Dimensão de agrupamento.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GroupBy {
    /// Por agente.
    Agent,
    /// Por skill.
    Skill,
}

impl GroupBy {
    /// Nome da coluna SQL (`agent`|`skill`).
    pub fn column(self) -> &'static str {
        match self {
            Self::Agent => "agent",
            Self::Skill => "skill",
        }
    }

    /// Parse (`agent|skill`).
    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "agent" => Ok(Self::Agent),
            "skill" => Ok(Self::Skill),
            o => Err(AnalyticsError::Invalid(format!("group_by inválido: {o}"))),
        }
    }
}

/// Métricas agregadas por chave (agente ou skill).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroupStats {
    /// Valor da chave (nome do agente/skill).
    pub key: String,
    /// Métricas consolidadas.
    pub stats: Bucket,
}

/// Célula do heatmap (dia da semana × hora do dia, UTC).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeatmapCell {
    /// 0 = segunda … 6 = domingo.
    pub weekday: u8,
    /// 0..=23.
    pub hour: u8,
    /// Execuções.
    pub count: u64,
    /// Taxa de sucesso.
    pub success_rate: f64,
}

/// Fonte dos dados de uma agregação: `(tabela, coluna de tempo, é rollup,
/// WHERE, parâmetros)`.
fn source(f: &Filter) -> Result<(&'static str, &'static str, bool, String, Vec<Value>)> {
    let src = Source::of(f);
    let (w, p) = src.where_sql(f)?;
    Ok((src.table(), src.ts_col(), src.is_rollup(), w, p))
}

/// Seleção de métricas para rollup vs bruto.
fn metric_cols(rollup: bool) -> &'static str {
    if rollup {
        "SUM(n), SUM(ok), SUM(lat_sum), MAX(lat_max), SUM(cost), SUM(tokens)"
    } else {
        "COUNT(*), SUM(success), SUM(latency_ms), MAX(latency_ms), SUM(cost_usd), SUM(tokens)"
    }
}

fn row_bucket(r: &rusqlite::Row<'_>, off: usize, start: i64) -> rusqlite::Result<Bucket> {
    let n: i64 = r.get::<_, Option<i64>>(off)?.unwrap_or(0);
    let ok: i64 = r.get::<_, Option<i64>>(off + 1)?.unwrap_or(0);
    let lat_sum: f64 = r.get::<_, Option<f64>>(off + 2)?.unwrap_or(0.0);
    let lat_max: i64 = r.get::<_, Option<i64>>(off + 3)?.unwrap_or(0);
    let cost: f64 = r.get::<_, Option<f64>>(off + 4)?.unwrap_or(0.0);
    let tokens: i64 = r.get::<_, Option<i64>>(off + 5)?.unwrap_or(0);
    Ok(Bucket {
        start,
        count: n.max(0) as u64,
        success: ok.max(0) as u64,
        max_latency_ms: lat_max.max(0) as u64,
        cost_usd: cost,
        tokens: tokens.max(0) as u64,
        ..Bucket::default()
    }
    .finish(lat_sum))
}

impl AnalyticsStore {
    /// Série temporal agregada por `granularity` (ordenada por `start`).
    /// Apenas buckets com dados; use [`fill_gaps`] para série contínua.
    pub fn aggregate(&self, f: &Filter, granularity: Granularity) -> Result<Vec<Bucket>> {
        let (table, tcol, rollup, w, p) = source(f)?;
        let b = granularity.sql_bucket(tcol);
        let sql = format!(
            "SELECT {b} AS bk, {m} FROM {table}{w} GROUP BY bk ORDER BY bk",
            m = metric_cols(rollup)
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(p), |r| {
            let start: i64 = r.get(0)?;
            row_bucket(r, 1, start)
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Resumo total do filtro (um único bucket com `start = since` ou 0).
    pub fn summary(&self, f: &Filter) -> Result<Bucket> {
        let (table, _tcol, rollup, w, p) = source(f)?;
        let sql = format!("SELECT {m} FROM {table}{w}", m = metric_cols(rollup));
        let start = f.since.unwrap_or(0);
        Ok(self
            .conn
            .query_row(&sql, params_from_iter(p), |r| row_bucket(r, 0, start))?)
    }

    /// Métricas por agente ou skill (ordenado por volume desc).
    pub fn breakdown(&self, f: &Filter, by: GroupBy) -> Result<Vec<GroupStats>> {
        let (table, _tcol, rollup, w, p) = source(f)?;
        let c = by.column();
        let sql = format!(
            "SELECT {c}, {m} FROM {table}{w} GROUP BY {c} ORDER BY 2 DESC, {c}",
            m = metric_cols(rollup)
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let start = f.since.unwrap_or(0);
        let rows = stmt.query_map(params_from_iter(p), |r| {
            Ok(GroupStats {
                key: r.get(0)?,
                stats: row_bucket(r, 1, start)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Série temporal por chave (ex.: evolução diária de cada skill).
    pub fn aggregate_by(
        &self,
        f: &Filter,
        granularity: Granularity,
        by: GroupBy,
    ) -> Result<BTreeMap<String, Vec<Bucket>>> {
        let (table, tcol, rollup, w, p) = source(f)?;
        let b = granularity.sql_bucket(tcol);
        let c = by.column();
        let sql = format!(
            "SELECT {c}, {b} AS bk, {m} FROM {table}{w} GROUP BY {c}, bk ORDER BY {c}, bk",
            m = metric_cols(rollup)
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(p), |r| {
            let k: String = r.get(0)?;
            let start: i64 = r.get(1)?;
            Ok((k, row_bucket(r, 2, start)?))
        })?;
        let mut out: BTreeMap<String, Vec<Bucket>> = BTreeMap::new();
        for r in rows {
            let (k, bk) = r?;
            out.entry(k).or_default().push(bk);
        }
        Ok(out)
    }

    /// Heatmap dia-da-semana (seg=0) × hora (UTC). Sempre 168 células.
    pub fn heatmap(&self, f: &Filter) -> Result<Vec<HeatmapCell>> {
        let (table, tcol, rollup, w, p) = source(f)?;
        let (n, ok) = if rollup {
            ("SUM(n)", "SUM(ok)")
        } else {
            ("COUNT(*)", "SUM(success)")
        };
        let sql = format!(
            "SELECT ((({tcol} / 86400) + 3) % 7) AS wd, (({tcol} % 86400) / 3600) AS hr, {n}, {ok}
             FROM {table}{w} GROUP BY wd, hr"
        );
        let mut grid = vec![(0u64, 0u64); 168];
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(p), |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, Option<i64>>(3)?.unwrap_or(0),
            ))
        })?;
        for r in rows {
            let (wd, hr, cnt, okc) = r?;
            if (0..7).contains(&wd) && (0..24).contains(&hr) {
                grid[(wd * 24 + hr) as usize] = (cnt.max(0) as u64, okc.max(0) as u64);
            }
        }
        Ok(grid
            .into_iter()
            .enumerate()
            .map(|(i, (count, okc))| HeatmapCell {
                weekday: (i / 24) as u8,
                hour: (i % 24) as u8,
                count,
                success_rate: if count > 0 {
                    okc as f64 / count as f64
                } else {
                    0.0
                },
            })
            .collect())
    }

    /// Agregação com predicado customizado arbitrário (avaliado em memória).
    pub fn aggregate_with_predicate(
        &self,
        f: &Filter,
        granularity: Granularity,
        pred: impl Fn(&AnalyticsEvent) -> bool,
    ) -> Result<Vec<Bucket>> {
        let mut acc: BTreeMap<i64, (Bucket, f64)> = BTreeMap::new();
        // buffer reaproveitado e sem ORDER BY: o BTreeMap já ordena os buckets
        self.scan_events(f, false, |e| {
            if !pred(e) {
                return;
            }
            let start = granularity.bucket_start(e.ts);
            let (b, lat) = acc.entry(start).or_insert_with(|| {
                (
                    Bucket {
                        start,
                        ..Bucket::default()
                    },
                    0.0,
                )
            });
            b.count += 1;
            b.success += e.success as u64;
            b.max_latency_ms = b.max_latency_ms.max(e.latency_ms);
            b.cost_usd += e.cost_usd;
            b.tokens += e.tokens;
            *lat += e.latency_ms as f64;
        })?;
        Ok(acc.into_values().map(|(b, lat)| b.finish(lat)).collect())
    }
}

/// Preenche buckets vazios entre `since` e `until` (exclusivo) para séries contínuas.
/// Limita a 10 000 buckets para evitar explosão de memória.
pub fn fill_gaps(buckets: &[Bucket], g: Granularity, since: i64, until: i64) -> Vec<Bucket> {
    let by_start: BTreeMap<i64, &Bucket> = buckets.iter().map(|b| (b.start, b)).collect();
    let mut out = Vec::new();
    let mut t = g.bucket_start(since);
    while t < until && out.len() < 10_000 {
        out.push(by_start.get(&t).map(|b| (*b).clone()).unwrap_or(Bucket {
            start: t,
            ..Bucket::default()
        }));
        t = g.next_bucket(t);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const MON_2026_11_16: i64 = 1_794_787_200; // 2026-11-16 00:00 UTC (segunda)

    fn seeded() -> AnalyticsStore {
        let mut s = AnalyticsStore::open_in_memory().unwrap();
        let t = MON_2026_11_16;
        let evs = vec![
            AnalyticsEvent::new(t + 60, "a1", "s1", true, 100).with_cost(0.01, 10),
            AnalyticsEvent::new(t + 120, "a1", "s1", false, 300).with_cost(0.02, 20),
            AnalyticsEvent::new(t + 3600 + 5, "a2", "s2", true, 50),
            AnalyticsEvent::new(t + 86_400 + 5, "a2", "s1", true, 70).with_dim("channel", "slack"),
            AnalyticsEvent::new(t + 8 * 86_400, "a1", "s2", false, 10),
        ];
        s.insert_batch(&evs).unwrap();
        s
    }

    #[test]
    fn dim_rollup_matches_raw_scan() {
        let mut s = AnalyticsStore::open_in_memory().unwrap();
        let evs = crate::synthetic::generate(
            &crate::synthetic::SyntheticConfig {
                days: 3,
                events_per_day: 400,
                ..Default::default()
            },
            MON_2026_11_16,
        );
        s.insert_batch(&evs).unwrap();
        let since = MON_2026_11_16 - 3 * 86_400;
        let dimf = Filter::all()
            .range(since, MON_2026_11_16)
            .agent("hermes")
            .dim("channel", "slack");
        assert!(dimf.dim_rollup_compatible());
        // referência: varredura em memória dos eventos brutos
        let reference = s
            .aggregate_with_predicate(&Filter::all(), Granularity::Daily, |e| dimf.matches(e))
            .unwrap();
        let fast = s.aggregate(&dimf, Granularity::Daily).unwrap();
        assert_eq!(fast.len(), reference.len());
        for (a, b) in fast.iter().zip(&reference) {
            assert_eq!(
                (a.start, a.count, a.success, a.max_latency_ms, a.tokens),
                (b.start, b.count, b.success, b.max_latency_ms, b.tokens)
            );
            assert!((a.avg_latency_ms - b.avg_latency_ms).abs() < 1e-9);
            assert!((a.cost_usd - b.cost_usd).abs() < 1e-9);
        }
        let n_ref: u64 = reference.iter().map(|b| b.count).sum();
        assert!(n_ref > 0);
        assert_eq!(s.count(&dimf).unwrap(), n_ref);
        assert_eq!(s.summary(&dimf).unwrap().count, n_ref);
        let by_skill: u64 = s
            .breakdown(&dimf, GroupBy::Skill)
            .unwrap()
            .iter()
            .map(|g| g.stats.count)
            .sum();
        assert_eq!(by_skill, n_ref);
        let heat: u64 = s.heatmap(&dimf).unwrap().iter().map(|c| c.count).sum();
        assert_eq!(heat, n_ref);
        // dimensão/valor inexistentes → vazio
        assert_eq!(s.count(&Filter::all().dim("channel", "fax")).unwrap(), 0);
    }

    #[test]
    fn weekly_bucket_starts_monday() {
        assert_eq!(
            Granularity::Weekly.bucket_start(MON_2026_11_16 + 5 * 86_400),
            MON_2026_11_16
        );
        // 1970-01-05 foi segunda-feira
        assert_eq!(
            Granularity::Weekly.bucket_start(4 * 86_400 + 10),
            4 * 86_400
        );
    }

    #[test]
    fn monthly_bucket_and_next() {
        let nov1 = 1_793_491_200; // 2026-11-01
        let dec1 = 1_796_083_200; // 2026-12-01
        assert_eq!(Granularity::Monthly.bucket_start(MON_2026_11_16), nov1);
        assert_eq!(Granularity::Monthly.next_bucket(nov1), dec1);
    }

    #[test]
    fn hourly_daily_weekly_monthly() {
        let s = seeded();
        let f = Filter::all();
        let h = s.aggregate(&f, Granularity::Hourly).unwrap();
        assert_eq!(h.len(), 4);
        assert_eq!(h[0].count, 2);
        assert_eq!(h[0].success, 1);
        assert!((h[0].success_rate - 0.5).abs() < 1e-9);
        assert!((h[0].avg_latency_ms - 200.0).abs() < 1e-9);
        assert_eq!(h[0].max_latency_ms, 300);
        assert!((h[0].cost_usd - 0.03).abs() < 1e-9);
        let d = s.aggregate(&f, Granularity::Daily).unwrap();
        assert_eq!(d.iter().map(|b| b.count).collect::<Vec<_>>(), vec![3, 1, 1]);
        let w = s.aggregate(&f, Granularity::Weekly).unwrap();
        assert_eq!(w.iter().map(|b| b.count).collect::<Vec<_>>(), vec![4, 1]);
        let m = s.aggregate(&f, Granularity::Monthly).unwrap();
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].count, 5);
    }

    #[test]
    fn rollup_and_raw_paths_agree() {
        let s = seeded();
        // alinhado → rollup
        let f1 = Filter::all().range(MON_2026_11_16, MON_2026_11_16 + 10 * 86_400);
        // desalinhado por 1s → bruto, mesmo conjunto de eventos
        let f2 = Filter::all().range(MON_2026_11_16 + 1, MON_2026_11_16 + 10 * 86_400);
        assert!(f1.rollup_compatible() && !f2.rollup_compatible());
        let a = s.aggregate(&f1, Granularity::Daily).unwrap();
        let b = s.aggregate(&f2, Granularity::Daily).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn breakdown_and_by_skill_series() {
        let s = seeded();
        let bd = s.breakdown(&Filter::all(), GroupBy::Skill).unwrap();
        assert_eq!(bd[0].key, "s1");
        assert_eq!(bd[0].stats.count, 3);
        let series = s
            .aggregate_by(&Filter::all(), Granularity::Daily, GroupBy::Agent)
            .unwrap();
        assert_eq!(series["a1"].len(), 2);
        assert_eq!(series["a2"].len(), 2);
    }

    #[test]
    fn heatmap_has_168_cells() {
        let s = seeded();
        let hm = s.heatmap(&Filter::all()).unwrap();
        assert_eq!(hm.len(), 168);
        // segunda 00h → 2 eventos
        assert_eq!(hm[0].count, 2);
        // segunda 01h → 1
        assert_eq!(hm[1].count, 1);
        // terça 00h → 2 (dia +1 e dia +8)
        assert_eq!(hm[24].count, 2);
        let total: u64 = hm.iter().map(|c| c.count).sum();
        assert_eq!(total, 5);
    }

    #[test]
    fn custom_dimension_and_predicate() {
        let s = seeded();
        let d = s
            .aggregate(&Filter::all().dim("channel", "slack"), Granularity::Daily)
            .unwrap();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].count, 1);
        let slow = s
            .aggregate_with_predicate(&Filter::all(), Granularity::Weekly, |e| e.latency_ms >= 100)
            .unwrap();
        assert_eq!(slow.len(), 1);
        assert_eq!(slow[0].count, 2);
    }

    #[test]
    fn fill_gaps_continuous() {
        let s = seeded();
        let d = s.aggregate(&Filter::all(), Granularity::Daily).unwrap();
        let full = fill_gaps(
            &d,
            Granularity::Daily,
            MON_2026_11_16,
            MON_2026_11_16 + 10 * 86_400,
        );
        assert_eq!(full.len(), 10);
        assert_eq!(full[2].count, 0);
        assert_eq!(full[8].count, 1);
    }

    #[test]
    fn summary_totals() {
        let s = seeded();
        let t = s.summary(&Filter::all()).unwrap();
        assert_eq!(t.count, 5);
        assert_eq!(t.success, 3);
        let empty = s.summary(&Filter::all().agent("nobody")).unwrap();
        assert_eq!(empty.count, 0);
        assert_eq!(empty.success_rate, 0.0);
    }

    #[test]
    fn granularity_parse() {
        assert_eq!(Granularity::parse("week").unwrap(), Granularity::Weekly);
        assert!(Granularity::parse("year").is_err());
    }
}
