//! FleetAuditor — painel de auditoria de frota de bots com métricas de custo e eficácia
//!
//! Inspirado em: awesome-grok-bot (Bot Fleet Auditor), Paperclip (cost tracking).
//! Exibe métricas agregadas por bot: tokens consumidos, custo estimado,
//! taxa de sucesso, tempo médio de resposta e ledger semanal.
//!
//! ## Features
//! - Cards por bot com métricas chave
//! - Ranking de custo e eficácia
//! - Ledger semanal consolidado
//! - Alertas de anomalia (custo spike, eficácia drop)
//! - Filtros por período (24h, 7d, 30d)
//!
//! ## Related Documents
//! - Implementation Plan: [IMPLEMENTATION_PLAN_DETAILED.md](../../docs/IMPLEMENTATION_PLAN_DETAILED.md) §5.1 Task 4.7

use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::{Deserialize, Serialize};

/// Métricas de um bot individual
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BotMetrics {
    pub bot_id: String,
    pub bot_name: String,
    pub total_tokens: u64,
    pub estimated_cost_usd: f64,
    pub success_rate_pct: f64,
    pub avg_response_ms: u64,
    pub total_actions: u64,
    pub failed_actions: u64,
    pub alerts: Vec<String>,
}

impl BotMetrics {
    #[allow(dead_code)]
    pub fn cost_per_action(&self) -> f64 {
        if self.total_actions > 0 {
            self.estimated_cost_usd / self.total_actions as f64
        } else {
            0.0
        }
    }

    pub fn has_alerts(&self) -> bool {
        !self.alerts.is_empty()
    }
}

/// Ledger semanal consolidado
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeeklyLedger {
    pub period: String,
    pub total_cost_usd: f64,
    pub total_tokens: u64,
    pub total_actions: u64,
    pub avg_success_rate: f64,
    pub top_cost_bot: Option<String>,
    pub top_performer_bot: Option<String>,
}

/// Período de filtro
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditPeriod {
    Last24h,
    Last7d,
    Last30d,
}

impl AuditPeriod {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Last24h => "24h",
            Self::Last7d => "7 dias",
            Self::Last30d => "30 dias",
        }
    }
}

/// Formata número com separador de milhar (ex: 1,234,567)
fn format_with_commas(n: u64) -> String {
    let s = n.to_string();
    let len = s.len();
    if len <= 3 {
        return s;
    }
    let mut result = String::with_capacity(len + len / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (len - i).is_multiple_of(3) {
            result.push(',');
        }
        result.push(c);
    }
    result
}

#[component]
pub fn FleetAuditorPanel() -> impl IntoView {
    let bots = RwSignal::new(Vec::<BotMetrics>::new());
    let ledger = RwSignal::new(None::<WeeklyLedger>);
    let loading = RwSignal::new(true);
    let period = RwSignal::new(AuditPeriod::Last7d);

    // Fetch dados ao montar
    Effect::new(move |_| {
        spawn_local(async move {
            loading.set(true);
            if let Ok(data) = fetch_fleet_metrics().await {
                bots.set(data.bots);
                ledger.set(Some(data.ledger));
            }
            loading.set(false);
        });
    });

    let _sorted_by_cost = Memo::new(move |_| {
        let mut b = bots.get();
        b.sort_by(|a, b| {
            b.estimated_cost_usd
                .partial_cmp(&a.estimated_cost_usd)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        b
    });

    let _sorted_by_effectiveness = Memo::new(move |_| {
        let mut b = bots.get();
        b.sort_by(|a, b| {
            b.success_rate_pct
                .partial_cmp(&a.success_rate_pct)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        b
    });

    view! {
        <div class="fleet-auditor-panel">
            <div class="fleet-header">
                <h3 class="fleet-title"><i class="ph ph-chart-line-up" aria-hidden="true"></i>" Fleet Auditor"</h3>
                <div class="fleet-period-selector">
                    <For
                        each=move || vec![AuditPeriod::Last24h, AuditPeriod::Last7d, AuditPeriod::Last30d]
                        key=|p| format!("{:?}", p)
                        children=move |p| {
                            let p_clone = p;
                            let label = p.label().to_string();
                            let current = period;
                            let is_active = move || current.get() == p_clone;
                            let css = if is_active() {
                                "period-btn period-btn-active".to_string()
                            } else {
                                "period-btn".to_string()
                            };
                            view! {
                                <button
                                    class=css
                                    on:click=move |_| period.set(p_clone)
                                >
                                    {label.clone()}
                                </button>
                            }
                        }
                    />
                </div>
            </div>

            // Weekly Ledger Summary
            {move || {
                if let Some(ref l) = ledger.get() {
                    view! {
                        <div class="fleet-ledger-summary">
                            <div class="ledger-card">
                                <span class="ledger-label">"Custo Total"</span>
                                <span class="ledger-value ledger-cost">
                                    "$"{format!("{:.2}", l.total_cost_usd)}
                                </span>
                            </div>
                            <div class="ledger-card">
                                <span class="ledger-label">"Tokens"</span>
                                <span class="ledger-value">{format_with_commas(l.total_tokens)}</span>
                            </div>
                            <div class="ledger-card">
                                <span class="ledger-label">"Ações"</span>
                                <span class="ledger-value">{format_with_commas(l.total_actions)}</span>
                            </div>
                            <div class="ledger-card">
                                <span class="ledger-label">"Taxa Sucesso"</span>
                                <span class="ledger-value ledger-success">
                                    {format!("{:.1}%", l.avg_success_rate)}
                                </span>
                            </div>
                        </div>
                    }.into_any()
                } else {
                    view! { <div></div> }.into_any()
                }
            }}

            // Bot Cards Grid
            {move || {
                if loading.get() {
                    view! {
                        <div class="fleet-loading">
                            <span class="spinner"></span>
                            <span>"Carregando métricas da frota..."</span>
                        </div>
                    }.into_any()
                } else {
                    let items = bots.get();
                    if items.is_empty() {
                        view! {
                            <div class="fleet-empty">"Nenhum bot registrado no auditor."</div>
                        }.into_any()
                    } else {
                        view! {
                            <div class="fleet-grid">
                                <For
                                    each=move || bots.get()
                                    key=|b| b.bot_id.clone()
                                    children=move |bot| {
                                        let name = bot.bot_name.clone();
                                        let cost = bot.estimated_cost_usd;
                                        let tokens = bot.total_tokens;
                                        let success = bot.success_rate_pct;
                                        let avg_ms = bot.avg_response_ms;
                                        let actions = bot.total_actions;
                                        let failed = bot.failed_actions;
                                        let alerts = bot.alerts.clone();
                                        let has_alert = bot.has_alerts();

                                        let card_css = if has_alert {
                                            "fleet-bot-card fleet-bot-alert".to_string()
                                        } else {
                                            "fleet-bot-card".to_string()
                                        };

                                        view! {
                                            <div class=card_css>
                                                <div class="fleet-bot-header">
                                                    <span class="fleet-bot-name">{name.clone()}</span>
                                                    {if has_alert {
                                                        view! { <span class="fleet-alert-badge"><i class="ph ph-warning" aria-hidden="true"></i></span> }.into_any()
                                                    } else {
                                                        view! { <div></div> }.into_any()
                                                    }}
                                                </div>
                                                <div class="fleet-bot-metrics">
                                                    <div class="fleet-metric">
                                                        <span class="metric-label">"Custo"</span>
                                                        <span class="metric-value">${format!("{:.4}", cost)}</span>
                                                    </div>
                                                    <div class="fleet-metric">
                                                        <span class="metric-label">"Tokens"</span>
                                                        <span class="metric-value">{format_with_commas(tokens)}</span>
                                                    </div>
                                                    <div class="fleet-metric">
                                                        <span class="metric-label">"Sucesso"</span>
                                                        <span class="metric-value">{format!("{:.1}%", success)}</span>
                                                    </div>
                                                    <div class="fleet-metric">
                                                        <span class="metric-label">"Latência"</span>
                                                        <span class="metric-value">{avg_ms}"ms"</span>
                                                    </div>
                                                    <div class="fleet-metric">
                                                        <span class="metric-label">"Ações"</span>
                                                        <span class="metric-value">{actions}{" ("}{failed}" falhas)"</span>
                                                    </div>
                                                </div>
                                                {if !alerts.is_empty() {
                                                    view! {
                                                        <div class="fleet-alerts">
                                                            <For
                                                                each=move || alerts.clone()
                                                                key=|a| a.clone()
                                                                children=move |alert| {
                                                                    view! {
                                                                        <div class="fleet-alert-item">{alert.clone()}</div>
                                                                    }
                                                                }
                                                            />
                                                        </div>
                                                    }.into_any()
                                                } else {
                                                    view! { <div></div> }.into_any()
                                                }}
                                            </div>
                                        }
                                    }
                                />
                            </div>
                        }.into_any()
                    }
                }
            }}
        </div>
    }
}

// === API Function ===

struct FleetData {
    bots: Vec<BotMetrics>,
    ledger: WeeklyLedger,
}

async fn fetch_fleet_metrics() -> Result<FleetData, String> {
    // Placeholder: métricas virão do backend quando fleet auditing estiver integrado
    Ok(FleetData {
        bots: vec![
            BotMetrics {
                bot_id: "bot-builder".into(),
                bot_name: "Code Builder".into(),
                total_tokens: 2_450_000,
                estimated_cost_usd: 12.35,
                success_rate_pct: 94.2,
                avg_response_ms: 1850,
                total_actions: 342,
                failed_actions: 20,
                alerts: vec![],
            },
            BotMetrics {
                bot_id: "bot-security".into(),
                bot_name: "Shield Guard".into(),
                total_tokens: 1_890_000,
                estimated_cost_usd: 9.45,
                success_rate_pct: 98.7,
                avg_response_ms: 920,
                total_actions: 567,
                failed_actions: 7,
                alerts: vec![],
            },
            BotMetrics {
                bot_id: "bot-devops".into(),
                bot_name: "Pipeline Runner".into(),
                total_tokens: 3_200_000,
                estimated_cost_usd: 18.90,
                success_rate_pct: 87.3,
                avg_response_ms: 3200,
                total_actions: 189,
                failed_actions: 24,
                alerts: vec!["Custo spike: +40% vs semana anterior".into()],
            },
            BotMetrics {
                bot_id: "bot-research".into(),
                bot_name: "Deep Researcher".into(),
                total_tokens: 4_100_000,
                estimated_cost_usd: 24.60,
                success_rate_pct: 91.5,
                avg_response_ms: 4500,
                total_actions: 156,
                failed_actions: 13,
                alerts: vec![],
            },
            BotMetrics {
                bot_id: "bot-support".into(),
                bot_name: "Help Desk".into(),
                total_tokens: 980_000,
                estimated_cost_usd: 3.92,
                success_rate_pct: 96.8,
                avg_response_ms: 650,
                total_actions: 890,
                failed_actions: 28,
                alerts: vec![],
            },
            BotMetrics {
                bot_id: "bot-analyst".into(),
                bot_name: "Data Analyst".into(),
                total_tokens: 1_560_000,
                estimated_cost_usd: 7.80,
                success_rate_pct: 72.1,
                avg_response_ms: 2800,
                total_actions: 234,
                failed_actions: 65,
                alerts: vec![
                    "Eficácia baixa: 72.1% < threshold 80%".into(),
                    "Failed actions spike".into(),
                ],
            },
        ],
        ledger: WeeklyLedger {
            period: "2026-11-05 → 2026-11-11".into(),
            total_cost_usd: 77.02,
            total_tokens: 14_180_000,
            total_actions: 2378,
            avg_success_rate: 90.1,
            top_cost_bot: Some("Deep Researcher ($24.60)".into()),
            top_performer_bot: Some("Shield Guard (98.7%)".into()),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cost_per_action() {
        let bot = BotMetrics {
            bot_id: "test".into(),
            bot_name: "Test".into(),
            total_tokens: 1000,
            estimated_cost_usd: 10.0,
            success_rate_pct: 95.0,
            avg_response_ms: 1000,
            total_actions: 100,
            failed_actions: 5,
            alerts: vec![],
        };
        assert!((bot.cost_per_action() - 0.1).abs() < 0.001);
    }

    #[test]
    fn test_cost_per_action_zero_actions() {
        let bot = BotMetrics {
            bot_id: "test".into(),
            bot_name: "Test".into(),
            total_tokens: 0,
            estimated_cost_usd: 0.0,
            success_rate_pct: 0.0,
            avg_response_ms: 0,
            total_actions: 0,
            failed_actions: 0,
            alerts: vec![],
        };
        assert_eq!(bot.cost_per_action(), 0.0);
    }

    #[test]
    fn test_has_alerts() {
        let no_alerts = BotMetrics {
            bot_id: "test".into(),
            bot_name: "Test".into(),
            total_tokens: 0,
            estimated_cost_usd: 0.0,
            success_rate_pct: 0.0,
            avg_response_ms: 0,
            total_actions: 0,
            failed_actions: 0,
            alerts: vec![],
        };
        assert!(!no_alerts.has_alerts());

        let with_alerts = BotMetrics {
            alerts: vec!["alert".into()],
            ..no_alerts.clone()
        };
        assert!(with_alerts.has_alerts());
    }

    #[test]
    fn test_period_labels() {
        assert_eq!(AuditPeriod::Last24h.label(), "24h");
        assert_eq!(AuditPeriod::Last7d.label(), "7 dias");
        assert_eq!(AuditPeriod::Last30d.label(), "30 dias");
    }

    #[test]
    fn test_bot_metrics_serialization() {
        let bot = BotMetrics {
            bot_id: "bot-1".into(),
            bot_name: "Test Bot".into(),
            total_tokens: 50000,
            estimated_cost_usd: 1.25,
            success_rate_pct: 95.0,
            avg_response_ms: 800,
            total_actions: 100,
            failed_actions: 5,
            alerts: vec!["test alert".into()],
        };
        let json = serde_json::to_string(&bot).unwrap();
        assert!(json.contains("Test Bot"));
        assert!(json.contains("test alert"));
    }
}
