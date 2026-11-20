//! FleetAuditor — painel de auditoria da frota de bots, com o que o gateway mede de verdade.
//!
//! Lê `GET /api/v1/fleet/metrics`: por bot, quantas perguntas respondeu, quantas falharam, a taxa
//! de sucesso e a latência média, desde que o gateway subiu (contadores em memória: reiniciar
//! zera). Custo e tokens **não aparecem**: o gateway não recebe isso dos provedores.
//!
//! ## Related Documents
//! - Implementation Plan: [IMPLEMENTATION_PLAN_DETAILED.md](../../docs/IMPLEMENTATION_PLAN_DETAILED.md) §5.1 Task 4.7

use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::Deserialize;

/// Métricas de um bot, como o gateway devolve.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct BotMetrics {
    pub bot_id: String,
    pub bot_name: String,
    pub total_actions: u64,
    pub failed_actions: u64,
    /// `None` enquanto o bot não respondeu nenhuma pergunta (não é 0%).
    pub success_rate_pct: Option<f64>,
    pub avg_response_ms: Option<u64>,
}

/// Abaixo disto (com [`MIN_ACOES_PARA_ALERTA`] perguntas ou mais) o bot ganha um alerta.
const LIMITE_SUCESSO_PCT: f64 = 80.0;
/// Poucas perguntas não dizem nada sobre eficácia; não alerta antes disso.
const MIN_ACOES_PARA_ALERTA: u64 = 5;

impl BotMetrics {
    /// Alertas calculados a partir dos números reais.
    pub fn alerts(&self) -> Vec<String> {
        match self.success_rate_pct {
            Some(p) if self.total_actions >= MIN_ACOES_PARA_ALERTA && p < LIMITE_SUCESSO_PCT => {
                vec![format!(
                    "Taxa de sucesso {p:.1}% abaixo de {LIMITE_SUCESSO_PCT:.0}% ({} de {} perguntas falharam)",
                    self.failed_actions, self.total_actions
                )]
            }
            _ => Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct FleetResp {
    /// Quando o gateway subiu (RFC 3339): de onde vêm os contadores.
    since: String,
    bots: Vec<BotMetrics>,
}

/// Totais da frota, somados dos bots.
#[derive(Debug, Clone, PartialEq)]
pub struct Totais {
    pub acoes: u64,
    pub falhas: u64,
    /// `None` sem nenhuma pergunta ainda.
    pub sucesso_pct: Option<f64>,
    /// Média ponderada pelo número de perguntas de cada bot.
    pub latencia_ms: Option<u64>,
}

pub fn somar(bots: &[BotMetrics]) -> Totais {
    let acoes: u64 = bots.iter().map(|b| b.total_actions).sum();
    let falhas: u64 = bots.iter().map(|b| b.failed_actions).sum();
    let soma_ms: u64 = bots
        .iter()
        .map(|b| b.avg_response_ms.unwrap_or(0) * b.total_actions)
        .sum();
    Totais {
        acoes,
        falhas,
        sucesso_pct: (acoes > 0).then(|| (acoes - falhas) as f64 * 100.0 / acoes as f64),
        latencia_ms: (acoes > 0).then(|| soma_ms / acoes),
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

fn pct(v: Option<f64>) -> String {
    v.map(|p| format!("{p:.1}%")).unwrap_or_else(|| "—".into())
}

fn ms(v: Option<u64>) -> String {
    v.map(|m| format!("{m} ms")).unwrap_or_else(|| "—".into())
}

#[component]
pub fn FleetAuditorPanel() -> impl IntoView {
    let bots = RwSignal::new(Vec::<BotMetrics>::new());
    let desde = RwSignal::new(None::<String>);
    let erro = RwSignal::new(None::<String>);
    let loading = RwSignal::new(true);

    Effect::new(move |_| {
        spawn_local(async move {
            loading.set(true);
            match crate::gateway_http::get_json::<FleetResp>("/api/v1/fleet/metrics").await {
                Ok(r) => {
                    bots.set(r.bots);
                    desde.set(Some(r.since));
                    erro.set(None);
                }
                Err(e) => erro.set(Some(crate::ui_error::report("frota", &e))),
            }
            loading.set(false);
        });
    });

    view! {
        <div class="fleet-auditor-panel">
            <div class="fleet-header">
                <h3 class="fleet-title"><i class="ph ph-chart-line-up" aria-hidden="true"></i>" Fleet Auditor"</h3>
            </div>
            <p class="fleet-scope">
                {move || match desde.get() {
                    Some(d) => format!(
                        "Contadores em memória desde {}; reiniciar o gateway zera. Custo e tokens não são medidos.",
                        d.chars().take(16).collect::<String>().replace('T', " ")
                    ),
                    None => "Custo e tokens não são medidos.".to_string(),
                }}
            </p>

            {move || {
                let t = somar(&bots.get());
                (!bots.get().is_empty()).then(|| view! {
                    <div class="fleet-ledger-summary">
                        <div class="ledger-card">
                            <span class="ledger-label">"Perguntas"</span>
                            <span class="ledger-value">{format_with_commas(t.acoes)}</span>
                        </div>
                        <div class="ledger-card">
                            <span class="ledger-label">"Falhas"</span>
                            <span class="ledger-value">{format_with_commas(t.falhas)}</span>
                        </div>
                        <div class="ledger-card">
                            <span class="ledger-label">"Taxa Sucesso"</span>
                            <span class="ledger-value ledger-success">{pct(t.sucesso_pct)}</span>
                        </div>
                        <div class="ledger-card">
                            <span class="ledger-label">"Latência média"</span>
                            <span class="ledger-value">{ms(t.latencia_ms)}</span>
                        </div>
                    </div>
                })
            }}

            {move || erro.get().map(|e| view! { <div class="fleet-empty" role="alert">{e}</div> })}

            {move || {
                if loading.get() {
                    view! {
                        <div class="fleet-loading">
                            <span class="spinner"></span>
                            <span>"Carregando métricas da frota..."</span>
                        </div>
                    }.into_any()
                } else if bots.get().is_empty() && erro.get().is_none() {
                    view! {
                        <div class="fleet-empty">"Nenhum bot configurado na pasta de bots do gateway."</div>
                    }.into_any()
                } else {
                    view! {
                        <div class="fleet-grid">
                            <For
                                each=move || bots.get()
                                key=|b| b.bot_id.clone()
                                children=move |bot| {
                                    let alerts = bot.alerts();
                                    let has_alert = !alerts.is_empty();
                                    let card_css = if has_alert {
                                        "fleet-bot-card fleet-bot-alert"
                                    } else {
                                        "fleet-bot-card"
                                    };
                                    view! {
                                        <div class=card_css>
                                            <div class="fleet-bot-header">
                                                <span class="fleet-bot-name">{bot.bot_name.clone()}</span>
                                                {has_alert.then(|| view! {
                                                    <span class="fleet-alert-badge"><i class="ph ph-warning" aria-hidden="true"></i></span>
                                                })}
                                            </div>
                                            <div class="fleet-bot-metrics">
                                                <div class="fleet-metric">
                                                    <span class="metric-label">"Sucesso"</span>
                                                    <span class="metric-value">{pct(bot.success_rate_pct)}</span>
                                                </div>
                                                <div class="fleet-metric">
                                                    <span class="metric-label">"Latência"</span>
                                                    <span class="metric-value">{ms(bot.avg_response_ms)}</span>
                                                </div>
                                                <div class="fleet-metric">
                                                    <span class="metric-label">"Perguntas"</span>
                                                    <span class="metric-value">{bot.total_actions}{" ("}{bot.failed_actions}" falhas)"</span>
                                                </div>
                                            </div>
                                            {has_alert.then(|| view! {
                                                <div class="fleet-alerts">
                                                    {alerts.iter().map(|a| view! { <div class="fleet-alert-item">{a.clone()}</div> }).collect_view()}
                                                </div>
                                            })}
                                        </div>
                                    }
                                }
                            />
                        </div>
                    }.into_any()
                }
            }}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bot(total: u64, falhas: u64, ms: Option<u64>) -> BotMetrics {
        BotMetrics {
            bot_id: "b".into(),
            bot_name: "B".into(),
            total_actions: total,
            failed_actions: falhas,
            success_rate_pct: (total > 0).then(|| (total - falhas) as f64 * 100.0 / total as f64),
            avg_response_ms: ms,
        }
    }

    #[test]
    fn resposta_do_gateway_com_nulos() {
        let r: FleetResp = serde_json::from_str(
            r#"{"since":"2026-11-20T10:00:00Z","bots":[
                {"bot_id":"vigia","bot_name":"Vigia","total_actions":0,"failed_actions":0,"success_rate_pct":null,"avg_response_ms":null},
                {"bot_id":"arq","bot_name":"Arq","total_actions":4,"failed_actions":1,"success_rate_pct":75.0,"avg_response_ms":120}
            ]}"#,
        )
        .unwrap();
        assert_eq!(r.bots[0].success_rate_pct, None);
        assert_eq!(pct(r.bots[0].success_rate_pct), "—");
        assert_eq!(pct(r.bots[1].success_rate_pct), "75.0%");
        assert_eq!(ms(r.bots[1].avg_response_ms), "120 ms");
    }

    #[test]
    fn totais_somam_e_ponderam_a_latencia() {
        let t = somar(&[bot(4, 1, Some(100)), bot(6, 0, Some(200)), bot(0, 0, None)]);
        assert_eq!(t.acoes, 10);
        assert_eq!(t.falhas, 1);
        assert_eq!(t.sucesso_pct, Some(90.0));
        // (4*100 + 6*200) / 10
        assert_eq!(t.latencia_ms, Some(160));
        assert_eq!(somar(&[]).sucesso_pct, None);
        assert_eq!(somar(&[bot(0, 0, None)]).latencia_ms, None);
    }

    #[test]
    fn alerta_so_com_perguntas_suficientes_e_taxa_baixa() {
        assert!(bot(10, 5, Some(1)).alerts()[0].contains("50.0%"));
        assert!(
            bot(4, 4, Some(1)).alerts().is_empty(),
            "poucas perguntas: sem alerta"
        );
        assert!(
            bot(10, 1, Some(1)).alerts().is_empty(),
            "90% está acima do limite"
        );
        assert!(bot(0, 0, None).alerts().is_empty());
    }

    #[test]
    fn separador_de_milhar() {
        assert_eq!(format_with_commas(999), "999");
        assert_eq!(format_with_commas(1_234_567), "1,234,567");
    }
}
