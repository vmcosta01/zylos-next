//! Feedback loop de auto-melhoria (T11.10.5).
//!
//! Analisa uma janela recente (`window_days`) contra a janela anterior de
//! mesmo tamanho e emite [`Recommendation`]s para skills que:
//! - têm taxa de sucesso abaixo de `min_success_rate` (padrão 70%) →
//!   [`RecommendationKind::EvolveSkill`];
//! - pioraram mais que `regression_drop` pontos vs janela anterior →
//!   [`RecommendationKind::InvestigateRegression`];
//! - têm latência média acima de `max_avg_latency_ms` →
//!   [`RecommendationKind::OptimizeLatency`];
//! - concentram falhas num único agente (≥ 80% das falhas, ≥ 2 agentes) →
//!   [`RecommendationKind::ReviewAgentConfig`].
//!
//! Skills com menos de `min_samples` execuções são ignoradas para evitar
//! falsos positivos. O consumo das recomendações (abrir proposta de evolução
//! via `/api/v1/hermes/skill-evolution`, notificar, etc.) é descrito em
//! `docs/soar/soar-analytics-auto-improvement.md` — este módulo só detecta
//! e recomenda; nunca altera skills sozinho (gate humano obrigatório).

use crate::aggregations::GroupBy;
use crate::filter::Filter;
use crate::store::AnalyticsStore;
use crate::{Result, DAY_SECS};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Parâmetros do detector.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeedbackConfig {
    /// Tamanho da janela analisada (dias).
    pub window_days: i64,
    /// Taxa de sucesso mínima aceitável (0..1).
    pub min_success_rate: f64,
    /// Queda absoluta de taxa de sucesso que caracteriza regressão (0..1).
    pub regression_drop: f64,
    /// Latência média máxima aceitável (ms).
    pub max_avg_latency_ms: f64,
    /// Execuções mínimas para avaliar uma skill.
    pub min_samples: u64,
}

impl Default for FeedbackConfig {
    fn default() -> Self {
        Self {
            window_days: 7,
            min_success_rate: 0.70,
            regression_drop: 0.15,
            max_avg_latency_ms: 5_000.0,
            min_samples: 20,
        }
    }
}

/// Tipo de recomendação.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecommendationKind {
    /// Skill abaixo do limiar de sucesso → propor evolução.
    EvolveSkill,
    /// Queda significativa vs período anterior.
    InvestigateRegression,
    /// Latência média acima do limite.
    OptimizeLatency,
    /// Falhas concentradas em um agente.
    ReviewAgentConfig,
}

/// Severidade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Baixa.
    Low,
    /// Média.
    Medium,
    /// Alta.
    High,
    /// Crítica.
    Critical,
}

/// Uma recomendação acionável.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Recommendation {
    /// Skill afetada.
    pub skill: String,
    /// Agente (para `ReviewAgentConfig`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    /// Tipo.
    pub kind: RecommendationKind,
    /// Severidade.
    pub severity: Severity,
    /// Taxa de sucesso na janela atual.
    pub success_rate: f64,
    /// Taxa na janela anterior (se houver amostras).
    #[serde(default)]
    pub previous_success_rate: Option<f64>,
    /// Execuções na janela atual.
    pub samples: u64,
    /// Latência média (ms) na janela atual.
    pub avg_latency_ms: f64,
    /// Explicação legível (pt-BR).
    pub rationale: String,
    /// Ação sugerida (pt-BR).
    pub suggested_action: String,
}

/// Resultado de uma análise.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeedbackReport {
    /// Momento de referência (`now`).
    pub generated_at: i64,
    /// Início da janela analisada.
    pub window_start: i64,
    /// Configuração usada.
    pub config: FeedbackConfig,
    /// Skills avaliadas (≥ `min_samples`).
    pub skills_evaluated: usize,
    /// Recomendações ordenadas por severidade desc, depois taxa asc.
    pub recommendations: Vec<Recommendation>,
}

fn severity_for(rate: f64, min: f64) -> Severity {
    if rate < min - 0.30 {
        Severity::Critical
    } else if rate < min - 0.15 {
        Severity::High
    } else {
        Severity::Medium
    }
}

/// Executa a análise com referência em `now`.
pub fn analyze(store: &AnalyticsStore, now: i64, cfg: &FeedbackConfig) -> Result<FeedbackReport> {
    let win = cfg.window_days.max(1) * DAY_SECS;
    let cur_f = Filter::all().range(now - win, now);
    let prev_f = Filter::all().range(now - 2 * win, now - win);
    let current = store.breakdown(&cur_f, GroupBy::Skill)?;
    let previous: HashMap<String, crate::Bucket> = store
        .breakdown(&prev_f, GroupBy::Skill)?
        .into_iter()
        .map(|g| (g.key, g.stats))
        .collect();

    let mut recs = Vec::new();
    let mut evaluated = 0usize;
    for g in &current {
        let s = &g.stats;
        if s.count < cfg.min_samples {
            continue;
        }
        evaluated += 1;
        let prev_rate = previous
            .get(&g.key)
            .filter(|p| p.count >= cfg.min_samples)
            .map(|p| p.success_rate);
        let pct = |r: f64| format!("{:.1}%", r * 100.0);
        let base = |kind, severity, rationale: String, action: &str| Recommendation {
            skill: g.key.clone(),
            agent: None,
            kind,
            severity,
            success_rate: s.success_rate,
            previous_success_rate: prev_rate,
            samples: s.count,
            avg_latency_ms: s.avg_latency_ms,
            rationale,
            suggested_action: action.to_string(),
        };

        if s.success_rate < cfg.min_success_rate {
            recs.push(base(
                RecommendationKind::EvolveSkill,
                severity_for(s.success_rate, cfg.min_success_rate),
                format!(
                    "Taxa de sucesso {} em {} execuções nos últimos {} dias (limiar {}).",
                    pct(s.success_rate),
                    s.count,
                    cfg.window_days,
                    pct(cfg.min_success_rate)
                ),
                "Abrir proposta de evolução da skill (skill-evolution-lifecycle): revisar prompts/contrato, \
                 adicionar casos de falha ao eval e validar em canário antes de promover.",
            ));
        }
        if let Some(p) = prev_rate {
            if p - s.success_rate >= cfg.regression_drop {
                recs.push(base(
                    RecommendationKind::InvestigateRegression,
                    if p - s.success_rate >= 2.0 * cfg.regression_drop {
                        Severity::High
                    } else {
                        Severity::Medium
                    },
                    format!(
                        "Sucesso caiu de {} para {} vs janela anterior.",
                        pct(p),
                        pct(s.success_rate)
                    ),
                    "Correlacionar com deploys/alterações de modelo no período; considerar rollback da última versão da skill.",
                ));
            }
        }
        if s.avg_latency_ms > cfg.max_avg_latency_ms {
            recs.push(base(
                RecommendationKind::OptimizeLatency,
                Severity::Low,
                format!(
                    "Latência média {:.0} ms acima do limite {:.0} ms.",
                    s.avg_latency_ms, cfg.max_avg_latency_ms
                ),
                "Avaliar tier de modelo mais rápido, cache de resultados ou paralelização de chamadas externas.",
            ));
        }
        // concentração de falhas por agente
        let failures = s.count - s.success;
        if failures >= cfg.min_samples / 2 && failures > 0 {
            let by_agent = store.breakdown(&cur_f.clone().skill(g.key.clone()), GroupBy::Agent)?;
            if by_agent.len() >= 2 {
                if let Some(worst) = by_agent
                    .iter()
                    .max_by_key(|a| a.stats.count - a.stats.success)
                {
                    let wf = worst.stats.count - worst.stats.success;
                    if wf as f64 / failures as f64 >= 0.8 {
                        let mut r = base(
                            RecommendationKind::ReviewAgentConfig,
                            Severity::Medium,
                            format!(
                                "{wf} de {failures} falhas ({:.0}%) vêm do agente '{}'.",
                                wf as f64 * 100.0 / failures as f64,
                                worst.key
                            ),
                            "Revisar configuração/credenciais/personalidade do agente antes de alterar a skill.",
                        );
                        r.agent = Some(worst.key.clone());
                        recs.push(r);
                    }
                }
            }
        }
    }
    recs.sort_by(|a, b| {
        b.severity
            .cmp(&a.severity)
            .then(a.success_rate.total_cmp(&b.success_rate))
            .then(a.skill.cmp(&b.skill))
    });
    Ok(FeedbackReport {
        generated_at: now,
        window_start: now - win,
        config: cfg.clone(),
        skills_evaluated: evaluated,
        recommendations: recs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::AnalyticsEvent;

    const NOW: i64 = 1_794_873_600; // 2026-11-17 00:00 UTC

    fn push(
        v: &mut Vec<AnalyticsEvent>,
        ts: i64,
        agent: &str,
        skill: &str,
        n: usize,
        ok: usize,
        lat: u64,
    ) {
        for i in 0..n {
            v.push(AnalyticsEvent::new(
                ts + i as i64,
                agent,
                skill,
                i < ok,
                lat,
            ));
        }
    }

    #[test]
    fn detects_low_success_skill() {
        let mut s = AnalyticsStore::open_in_memory().unwrap();
        let mut v = Vec::new();
        push(&mut v, NOW - DAY_SECS, "a1", "good", 100, 95, 100);
        push(&mut v, NOW - DAY_SECS, "a1", "bad", 50, 25, 100); // 50% < 70%
        push(&mut v, NOW - DAY_SECS, "a1", "rare", 5, 0, 100); // < min_samples
        s.insert_batch(&v).unwrap();
        let r = analyze(&s, NOW, &FeedbackConfig::default()).unwrap();
        assert_eq!(r.skills_evaluated, 2);
        let evolve: Vec<_> = r
            .recommendations
            .iter()
            .filter(|x| x.kind == RecommendationKind::EvolveSkill)
            .collect();
        assert_eq!(evolve.len(), 1);
        assert_eq!(evolve[0].skill, "bad");
        assert_eq!(evolve[0].severity, Severity::High); // 0.50 < 0.55
        assert!(evolve[0].rationale.contains("50.0%"));
    }

    #[test]
    fn detects_regression_latency_and_agent_concentration() {
        let mut s = AnalyticsStore::open_in_memory().unwrap();
        let mut v = Vec::new();
        // janela anterior: 95%
        push(&mut v, NOW - 10 * DAY_SECS, "a1", "sk", 100, 95, 100);
        // janela atual: 75% (acima de 70% mas caiu 20pp); falhas todas em a2; lento
        push(&mut v, NOW - DAY_SECS, "a1", "sk", 60, 60, 6_000);
        push(&mut v, NOW - DAY_SECS + 1000, "a2", "sk", 40, 15, 6_000);
        s.insert_batch(&v).unwrap();
        let r = analyze(&s, NOW, &FeedbackConfig::default()).unwrap();
        let kinds: Vec<_> = r.recommendations.iter().map(|x| x.kind).collect();
        assert!(kinds.contains(&RecommendationKind::InvestigateRegression));
        assert!(kinds.contains(&RecommendationKind::OptimizeLatency));
        assert!(!kinds.contains(&RecommendationKind::EvolveSkill));
        let agent = r
            .recommendations
            .iter()
            .find(|x| x.kind == RecommendationKind::ReviewAgentConfig)
            .unwrap();
        assert_eq!(agent.agent.as_deref(), Some("a2"));
    }

    #[test]
    fn healthy_data_no_recommendations() {
        let mut s = AnalyticsStore::open_in_memory().unwrap();
        let mut v = Vec::new();
        push(&mut v, NOW - DAY_SECS, "a1", "ok", 100, 99, 100);
        s.insert_batch(&v).unwrap();
        let r = analyze(&s, NOW, &FeedbackConfig::default()).unwrap();
        assert!(r.recommendations.is_empty());
    }
}
