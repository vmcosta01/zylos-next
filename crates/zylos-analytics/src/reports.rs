//! Relatórios preset (T11.10.7):
//! - **Weekly Bot Performance** — últimos 7 dias, por agente + série diária.
//! - **Monthly Cost Analysis** — últimos 30 dias, custo por skill/agente + série semanal.
//! - **Skill Evolution Summary** — últimas 4 semanas, taxa de sucesso semanal
//!   por skill + recomendações do feedback loop.
//!
//! Todo relatório é um [`Report`] neutro (seções de tabela/barras/texto)
//! renderizável em JSON, CSV ([`crate::export::report_to_csv`]) e PDF
//! ([`crate::export::report_to_pdf`]).

use crate::aggregations::{fill_gaps, Granularity, GroupBy};
use crate::export::iso;
use crate::feedback::{analyze, FeedbackConfig};
use crate::filter::Filter;
use crate::store::AnalyticsStore;
use crate::{AnalyticsError, Result, DAY_SECS};
use serde::{Deserialize, Serialize};

/// Presets disponíveis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PresetReport {
    /// "Weekly Bot Performance".
    WeeklyBotPerformance,
    /// "Monthly Cost Analysis".
    MonthlyCostAnalysis,
    /// "Skill Evolution Summary".
    SkillEvolutionSummary,
}

impl PresetReport {
    /// Todos os presets.
    pub const ALL: [PresetReport; 3] = [
        Self::WeeklyBotPerformance,
        Self::MonthlyCostAnalysis,
        Self::SkillEvolutionSummary,
    ];

    /// Identificador usado na API (`weekly_bot_performance`, ...).
    pub fn id(self) -> &'static str {
        match self {
            Self::WeeklyBotPerformance => "weekly_bot_performance",
            Self::MonthlyCostAnalysis => "monthly_cost_analysis",
            Self::SkillEvolutionSummary => "skill_evolution_summary",
        }
    }

    /// Nome de exibição.
    pub fn title(self) -> &'static str {
        match self {
            Self::WeeklyBotPerformance => "Weekly Bot Performance",
            Self::MonthlyCostAnalysis => "Monthly Cost Analysis",
            Self::SkillEvolutionSummary => "Skill Evolution Summary",
        }
    }

    /// Parse do id.
    pub fn parse(s: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|p| p.id() == s)
            .ok_or_else(|| AnalyticsError::Invalid(format!("relatório desconhecido: {s}")))
    }

    /// Gera o relatório com referência em `now`. `base` permite restringir
    /// agentes/skills/dimensões (período é definido pelo preset).
    pub fn generate(self, store: &AnalyticsStore, now: i64, base: &Filter) -> Result<Report> {
        match self {
            Self::WeeklyBotPerformance => weekly_bot_performance(store, now, base),
            Self::MonthlyCostAnalysis => monthly_cost(store, now, base),
            Self::SkillEvolutionSummary => skill_evolution(store, now, base),
        }
    }
}

/// Seção de relatório.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Section {
    /// Tabela.
    Table {
        /// Título.
        heading: String,
        /// Cabeçalhos.
        columns: Vec<String>,
        /// Linhas.
        rows: Vec<Vec<String>>,
    },
    /// Gráfico de barras horizontal.
    Bars {
        /// Título.
        heading: String,
        /// Unidade dos valores.
        unit: String,
        /// (rótulo, valor).
        items: Vec<(String, f64)>,
    },
    /// Parágrafo.
    Paragraph {
        /// Título.
        heading: String,
        /// Texto.
        text: String,
    },
}

/// Relatório renderizável.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Report {
    /// Id do preset.
    pub id: String,
    /// Título.
    pub title: String,
    /// Subtítulo/descrição.
    pub subtitle: String,
    /// Início do período (inclusivo).
    pub period_start: i64,
    /// Fim do período (exclusivo).
    pub period_end: i64,
    /// Momento de geração.
    pub generated_at: i64,
    /// Seções.
    pub sections: Vec<Section>,
}

fn pct(r: f64) -> String {
    format!("{:.1}%", r * 100.0)
}

fn day(ts: i64) -> String {
    iso(ts)[..10].to_string()
}

fn ranged(base: &Filter, since: i64, until: i64) -> Filter {
    let mut f = base.clone();
    f.since = Some(since);
    f.until = Some(until);
    f
}

/// Alinha `now` ao fim da hora corrente (exclusivo) para usar o rollup.
fn end_hour(now: i64) -> i64 {
    now - now.rem_euclid(3600) + 3600
}

fn weekly_bot_performance(store: &AnalyticsStore, now: i64, base: &Filter) -> Result<Report> {
    let end = end_hour(now);
    let start = end - 7 * DAY_SECS;
    let f = ranged(base, start, end);
    let total = store.summary(&f)?;
    let agents = store.breakdown(&f, GroupBy::Agent)?;
    let daily = fill_gaps(
        &store.aggregate(&f, Granularity::Daily)?,
        Granularity::Daily,
        start,
        end,
    );
    let p = PresetReport::WeeklyBotPerformance;
    Ok(Report {
        id: p.id().into(),
        title: p.title().into(),
        subtitle: format!(
            "{} execuções · sucesso {} · latência média {:.0} ms · {} agentes",
            total.count,
            pct(total.success_rate),
            total.avg_latency_ms,
            agents.len()
        ),
        period_start: start,
        period_end: end,
        generated_at: now,
        sections: vec![
            Section::Table {
                heading: "Desempenho por agente".into(),
                columns: [
                    "Agente",
                    "Execuções",
                    "Sucesso",
                    "Lat. média",
                    "Lat. máx",
                    "Custo USD",
                ]
                .map(String::from)
                .to_vec(),
                rows: agents
                    .iter()
                    .map(|g| {
                        vec![
                            g.key.clone(),
                            g.stats.count.to_string(),
                            pct(g.stats.success_rate),
                            format!("{:.0} ms", g.stats.avg_latency_ms),
                            format!("{} ms", g.stats.max_latency_ms),
                            format!("{:.2}", g.stats.cost_usd),
                        ]
                    })
                    .collect(),
            },
            Section::Bars {
                heading: "Taxa de sucesso por agente".into(),
                unit: "%".into(),
                items: agents
                    .iter()
                    .map(|g| (g.key.clone(), g.stats.success_rate * 100.0))
                    .collect(),
            },
            Section::Table {
                heading: "Volume diário".into(),
                columns: ["Dia", "Execuções", "Sucesso", "Lat. média"]
                    .map(String::from)
                    .to_vec(),
                rows: daily
                    .iter()
                    .map(|b| {
                        vec![
                            day(b.start),
                            b.count.to_string(),
                            pct(b.success_rate),
                            format!("{:.0} ms", b.avg_latency_ms),
                        ]
                    })
                    .collect(),
            },
        ],
    })
}

fn monthly_cost(store: &AnalyticsStore, now: i64, base: &Filter) -> Result<Report> {
    let end = end_hour(now);
    let start = end - 30 * DAY_SECS;
    let f = ranged(base, start, end);
    let total = store.summary(&f)?;
    let mut skills = store.breakdown(&f, GroupBy::Skill)?;
    skills.sort_by(|a, b| b.stats.cost_usd.total_cmp(&a.stats.cost_usd));
    let mut agents = store.breakdown(&f, GroupBy::Agent)?;
    agents.sort_by(|a, b| b.stats.cost_usd.total_cmp(&a.stats.cost_usd));
    let weekly = store.aggregate(&f, Granularity::Weekly)?;
    let per_exec = if total.count > 0 {
        total.cost_usd / total.count as f64
    } else {
        0.0
    };
    let p = PresetReport::MonthlyCostAnalysis;
    let cost_rows = |gs: &[crate::GroupStats]| -> Vec<Vec<String>> {
        gs.iter()
            .map(|g| {
                let share = if total.cost_usd > 0.0 {
                    g.stats.cost_usd / total.cost_usd
                } else {
                    0.0
                };
                let per = if g.stats.count > 0 {
                    g.stats.cost_usd / g.stats.count as f64
                } else {
                    0.0
                };
                vec![
                    g.key.clone(),
                    format!("{:.2}", g.stats.cost_usd),
                    pct(share),
                    g.stats.tokens.to_string(),
                    format!("{per:.4}"),
                ]
            })
            .collect()
    };
    Ok(Report {
        id: p.id().into(),
        title: p.title().into(),
        subtitle: format!(
            "Custo total USD {:.2} · {} tokens · {} execuções · USD {:.4}/execução",
            total.cost_usd, total.tokens, total.count, per_exec
        ),
        period_start: start,
        period_end: end,
        generated_at: now,
        sections: vec![
            Section::Bars {
                heading: "Custo por skill (top 10)".into(),
                unit: "USD".into(),
                items: skills
                    .iter()
                    .take(10)
                    .map(|g| (g.key.clone(), g.stats.cost_usd))
                    .collect(),
            },
            Section::Table {
                heading: "Custo por skill".into(),
                columns: ["Skill", "Custo USD", "% do total", "Tokens", "USD/exec"]
                    .map(String::from)
                    .to_vec(),
                rows: cost_rows(&skills),
            },
            Section::Table {
                heading: "Custo por agente".into(),
                columns: ["Agente", "Custo USD", "% do total", "Tokens", "USD/exec"]
                    .map(String::from)
                    .to_vec(),
                rows: cost_rows(&agents),
            },
            Section::Table {
                heading: "Custo semanal".into(),
                columns: ["Semana (seg)", "Custo USD", "Tokens", "Execuções"]
                    .map(String::from)
                    .to_vec(),
                rows: weekly
                    .iter()
                    .map(|b| {
                        vec![
                            day(b.start),
                            format!("{:.2}", b.cost_usd),
                            b.tokens.to_string(),
                            b.count.to_string(),
                        ]
                    })
                    .collect(),
            },
        ],
    })
}

fn skill_evolution(store: &AnalyticsStore, now: i64, base: &Filter) -> Result<Report> {
    let end = end_hour(now);
    let start = Granularity::Weekly.bucket_start(end - 28 * DAY_SECS);
    let f = ranged(base, start, end);
    let series = store.aggregate_by(&f, Granularity::Weekly, GroupBy::Skill)?;
    let mut weeks = Vec::new();
    let mut t = start;
    while t < end {
        weeks.push(t);
        t = Granularity::Weekly.next_bucket(t);
    }
    let mut columns = vec!["Skill".to_string()];
    columns.extend(weeks.iter().map(|w| format!("Sem {}", &day(*w)[5..])));
    columns.push("Tendência".into());
    let mut rows = Vec::new();
    let mut deltas = Vec::new();
    for (skill, buckets) in &series {
        let mut row = vec![skill.clone()];
        let mut rates = Vec::new();
        for w in &weeks {
            match buckets.iter().find(|b| b.start == *w && b.count > 0) {
                Some(b) => {
                    row.push(pct(b.success_rate));
                    rates.push(b.success_rate);
                }
                None => row.push("-".into()),
            }
        }
        let trend = match (rates.first(), rates.last()) {
            (Some(a), Some(b)) if rates.len() >= 2 => {
                deltas.push((skill.clone(), (b - a) * 100.0));
                format!("{:+.1} pp", (b - a) * 100.0)
            }
            _ => "n/d".into(),
        };
        row.push(trend);
        rows.push(row);
    }
    deltas.sort_by(|a, b| a.1.total_cmp(&b.1));
    let fb = analyze(store, now, &FeedbackConfig::default())?;
    let rec_rows: Vec<Vec<String>> = fb
        .recommendations
        .iter()
        .filter(|r| base.skills.is_empty() || base.skills.contains(&r.skill))
        .map(|r| {
            vec![
                r.skill.clone(),
                format!("{:?}", r.kind),
                format!("{:?}", r.severity),
                pct(r.success_rate),
                r.rationale.clone(),
            ]
        })
        .collect();
    let p = PresetReport::SkillEvolutionSummary;
    Ok(Report {
        id: p.id().into(),
        title: p.title().into(),
        subtitle: format!(
            "{} skills · {} recomendações de evolução (limiar de sucesso {})",
            series.len(),
            rec_rows.len(),
            pct(FeedbackConfig::default().min_success_rate)
        ),
        period_start: start,
        period_end: end,
        generated_at: now,
        sections: vec![
            Section::Table {
                heading: "Taxa de sucesso semanal por skill".into(),
                columns,
                rows,
            },
            Section::Bars {
                heading: "Variação de sucesso (primeira → última semana, pp)".into(),
                unit: "pp".into(),
                items: deltas,
            },
            Section::Table {
                heading: "Recomendações do feedback loop".into(),
                columns: ["Skill", "Tipo", "Severidade", "Sucesso", "Motivo"]
                    .map(String::from)
                    .to_vec(),
                rows: rec_rows,
            },
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::export::{report_to_csv, report_to_pdf};
    use crate::synthetic::{generate, SyntheticConfig};

    const NOW: i64 = 1_794_873_600;

    #[test]
    fn all_presets_generate_and_render() {
        let mut s = AnalyticsStore::open_in_memory().unwrap();
        let cfg = SyntheticConfig {
            days: 35,
            events_per_day: 400,
            ..SyntheticConfig::default()
        };
        s.insert_batch(&generate(&cfg, NOW)).unwrap();
        for p in PresetReport::ALL {
            let r = p.generate(&s, NOW, &Filter::all()).unwrap();
            assert_eq!(r.title, p.title());
            assert!(!r.sections.is_empty());
            let pdf = report_to_pdf(&r);
            assert!(pdf.starts_with(b"%PDF-1.4"));
            let csv = report_to_csv(&r);
            assert!(csv.starts_with(&format!("# {}", p.title())));
            assert_eq!(PresetReport::parse(p.id()).unwrap(), p);
        }
        let evo = PresetReport::SkillEvolutionSummary
            .generate(&s, NOW, &Filter::all())
            .unwrap();
        // o gerador sintético inclui uma skill degradada → ≥1 recomendação
        match &evo.sections[2] {
            Section::Table { rows, .. } => assert!(!rows.is_empty()),
            _ => panic!("seção 3 deve ser tabela"),
        }
    }

    #[test]
    fn empty_store_reports_ok() {
        let s = AnalyticsStore::open_in_memory().unwrap();
        for p in PresetReport::ALL {
            let r = p.generate(&s, NOW, &Filter::all()).unwrap();
            assert!(report_to_pdf(&r).len() > 500);
        }
    }
}
