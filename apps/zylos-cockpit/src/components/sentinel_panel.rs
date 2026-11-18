//! Sentinel/AIOps Alerts Panel — exibe alertas de anomalia e saúde do sistema
//!
//! Consome: GET /api/v1/phase2/status → subsystems.agent_shield + browser_takeover
//!          GET /api/v1/sentinel/metrics (polling 3 s) → gráficos SVG de anomalias (T11.7.6)
//!          GET/POST /api/v1/sentinel/remediations → fila HITL de remediação (T11.7.5)
//! Estados obrigatórios: Loading, Empty, Success, Error, Disabled, Validation

use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen_futures::spawn_local;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SentinelStats {
    pub total_audit_events: u64,
    pub critical_events: u64,
    pub high_events: u64,
    pub outbound_blocked: u64,
    pub active_cdp_sessions: u64,
    pub pending_cdp_sessions: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SentinelState {
    Loading,
    Empty,
    Success(SentinelStats),
    Error(String),
}

#[component]
pub fn SentinelPanel() -> impl IntoView {
    let (state, set_state) = signal(SentinelState::Loading);

    Effect::new(move |_| {
        spawn_local(async move {
            match fetch_sentinel_stats().await {
                Ok(stats) => {
                    if stats.total_audit_events == 0 && stats.active_cdp_sessions == 0 {
                        set_state.set(SentinelState::Empty);
                    } else {
                        set_state.set(SentinelState::Success(stats));
                    }
                }
                Err(e) => set_state.set(SentinelState::Error(e)),
            }
        });
    });

    view! {
        <div class="sentinel-panel">
            <SentinelAnomalyCharts />
            {move || match &*state.read() {
                SentinelState::Loading => view! {
                    <div class="skeleton-grid">
                        <div class="skeleton-card pulse"></div>
                        <div class="skeleton-card pulse"></div>
                        <div class="skeleton-card pulse"></div>
                    </div>
                }.into_any(),
                SentinelState::Empty => view! {
                    <div class="empty-state">
                        <p>"Nenhum evento de segurança registrado"</p>
                        <p class="muted">"O Agent Shield e CDP Proxy monitoram automaticamente. Alertas aparecerão aqui quando anomalias forem detectadas."</p>
                    </div>
                }.into_any(),
                SentinelState::Success(stats) => {
                    let audit = stats.total_audit_events.to_string();
                    let critical = stats.critical_events.to_string();
                    let blocked = stats.outbound_blocked.to_string();
                    let cdp_active = stats.active_cdp_sessions.to_string();
                    let cdp_pending = stats.pending_cdp_sessions.to_string();
                    view! {
                        <div class="dashboard-content">
                            <div class="metrics-bar">
                                <div class="metric-card">
                                    <span class="metric-label">"Eventos de Auditoria"</span>
                                    <span class="metric-value">{audit}</span>
                                </div>
                                <div class="metric-card">
                                    <span class="metric-label">"Críticos"</span>
                                    <span class="metric-value" style="color: var(--accent-red)">{critical}</span>
                                </div>
                                <div class="metric-card">
                                    <span class="metric-label">"Outbound Bloqueados"</span>
                                    <span class="metric-value muted">{blocked}</span>
                                </div>
                                <div class="metric-card">
                                    <span class="metric-label">"CDP Ativas"</span>
                                    <span class="metric-value muted">{cdp_active}</span>
                                </div>
                                <div class="metric-card">
                                    <span class="metric-label">"CDP Pendentes"</span>
                                    <span class="metric-value muted">{cdp_pending}</span>
                                </div>
                            </div>
                        </div>
                    }.into_any()
                }
                SentinelState::Error(msg) => {
                    let err_msg = msg.clone();
                    view! {
                        <div class="error-banner">
                            <p>{format!("Erro ao carregar sentinel: {}", crate::ui_error::report("Erro ao carregar sentinel", &err_msg))}</p>
                            <button class="cta-retry" on:click=move |_| {
                                spawn_local(async move {
                                    match fetch_sentinel_stats().await {
                                        Ok(s) => if s.total_audit_events == 0 && s.active_cdp_sessions == 0 {
                                            set_state.set(SentinelState::Empty);
                                        } else {
                                            set_state.set(SentinelState::Success(s));
                                        },
                                        Err(e) => set_state.set(SentinelState::Error(e)),
                                    }
                                });
                            }>"Tentar novamente"</button>
                        </div>
                    }.into_any()
                }
            }}
        </div>
    }
}

async fn fetch_sentinel_stats() -> Result<SentinelStats, String> {
    use wasm_bindgen::JsCast;
    use web_sys::{Request, RequestInit, Response};

    let opts = RequestInit::new();
    opts.set_method("GET");

    let request =
        Request::new_with_str_and_init("http://localhost:3000/api/v1/phase2/status", &opts)
            .map_err(|e| format!("{:?}", e))?;

    crate::auth::authed_web_request(&request);

    let window = web_sys::window().ok_or("no window")?;
    let resp_value = window
        .fetch_with_request(&request)
        .await
        .map_err(|e| format!("{:?}", e))?;

    let response: Response = resp_value.dyn_into().map_err(|_| "not a Response")?;

    if !response.ok() {
        return Err(format!("HTTP {}", response.status()));
    }

    let text_js = response
        .text()
        .map_err(|e| format!("{:?}", e))?
        .await
        .map_err(|e| format!("{:?}", e))?;
    let text: String = text_js.as_string().ok_or("response text is not a string")?;

    let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let subs = v.get("subsystems").ok_or("subsystems not found")?;

    let shield = subs.get("agent_shield");
    let cdp = subs.get("browser_takeover");

    Ok(SentinelStats {
        total_audit_events: shield
            .and_then(|s| s.get("total_audit_events"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        critical_events: shield
            .and_then(|s| s.get("critical_events"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        high_events: shield
            .and_then(|s| s.get("high_events"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        outbound_blocked: shield
            .and_then(|s| s.get("outbound_blocked"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        active_cdp_sessions: cdp
            .and_then(|s| s.get("active_sessions"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        pending_cdp_sessions: cdp
            .and_then(|s| s.get("pending_sessions"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
    })
}

// ─── T11.7.6: gráficos de anomalias (SVG puro, sem dependências JS) ──────────

const SENTINEL_API: &str = "http://localhost:3000/api/v1/sentinel";
/// Intervalo de polling — critério de aceitação: atraso < 5 s.
const POLL_SECS: u64 = 3;

/// Ponto de série devolvido por `/sentinel/metrics`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ChartPoint {
    pub t: u64,
    pub v: f64,
    #[serde(default)]
    pub anomaly: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnomalyItem {
    pub metric: String,
    pub value: f64,
    pub z_score: f64,
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlertItem {
    pub rule_name: String,
    pub current_value: f64,
    pub threshold: f64,
    pub severity: String,
    pub triggered_at_epoch_secs: u64,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SentinelLive {
    #[serde(default)]
    pub series: std::collections::HashMap<String, Vec<ChartPoint>>,
    #[serde(default)]
    pub anomalies: Vec<AnomalyItem>,
    #[serde(default)]
    pub alerts: Vec<AlertItem>,
    #[serde(default)]
    pub remediation_mode: String,
    #[serde(default)]
    pub pending_remediations: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemediationItem {
    pub id: String,
    pub rule_name: String,
    pub action: serde_json::Value,
    pub status: String,
    #[serde(default)]
    pub detail: Option<String>,
}

/// Métricas exibidas: (chave, rótulo, unidade, limite de alerta padrão).
pub const CHART_METRICS: [(&str, &str, &str, f64); 5] = [
    ("cpu_percent", "CPU", "%", 80.0),
    ("memory_percent", "Memória", "%", 90.0),
    ("latency_p95_ms", "Latência p95", "ms", 1500.0),
    ("error_rate_percent", "Taxa de erro", "%", 1.0),
    ("queue_depth", "Fila", "req", 200.0),
];

/// Geometria de um sparkline já calculada (função pura → testável fora do WASM).
#[derive(Debug, Clone, PartialEq)]
pub struct SparklineGeometry {
    pub path: String,
    pub anomaly_dots: Vec<(f64, f64)>,
    /// Y da linha de threshold (se dentro da escala).
    pub threshold_y: Option<f64>,
    pub min: f64,
    pub max: f64,
}

/// Calcula path SVG (`M x y L ...`), pontos de anomalia e linha de limite.
pub fn sparkline_geometry(
    points: &[ChartPoint],
    threshold: f64,
    width: f64,
    height: f64,
) -> Option<SparklineGeometry> {
    if points.is_empty() {
        return None;
    }
    let pad = 4.0;
    let data_min = points.iter().map(|p| p.v).fold(f64::INFINITY, f64::min);
    let data_max = points.iter().map(|p| p.v).fold(f64::NEG_INFINITY, f64::max);
    let min = data_min.min(0.0);
    let mut max = data_max.max(threshold);
    if (max - min).abs() < f64::EPSILON {
        max = min + 1.0;
    }
    let n = points.len();
    let x_of = |i: usize| {
        if n == 1 {
            width / 2.0
        } else {
            pad + (width - 2.0 * pad) * i as f64 / (n - 1) as f64
        }
    };
    let y_of = |v: f64| pad + (height - 2.0 * pad) * (1.0 - (v - min) / (max - min));
    let mut path = String::new();
    let mut dots = Vec::new();
    for (i, p) in points.iter().enumerate() {
        let (x, y) = (x_of(i), y_of(p.v));
        path.push_str(&format!(
            "{}{:.1} {:.1} ",
            if i == 0 { "M" } else { "L" },
            x,
            y
        ));
        if p.anomaly {
            dots.push((x, y));
        }
    }
    Some(SparklineGeometry {
        path: path.trim_end().to_string(),
        anomaly_dots: dots,
        threshold_y: (threshold <= max).then(|| y_of(threshold)),
        min,
        max,
    })
}

fn fmt_value(v: f64) -> String {
    if v.abs() >= 100.0 {
        format!("{v:.0}")
    } else {
        format!("{v:.1}")
    }
}

#[component]
fn MetricSparkline(
    label: &'static str,
    unit: &'static str,
    threshold: f64,
    points: Vec<ChartPoint>,
) -> impl IntoView {
    const W: f64 = 220.0;
    const H: f64 = 60.0;
    let last = points
        .last()
        .map(|p| fmt_value(p.v))
        .unwrap_or_else(|| "—".into());
    let anomalies = points.iter().filter(|p| p.anomaly).count();
    let geom = sparkline_geometry(&points, threshold, W, H);
    let body = match geom {
        None => view! { <text x="8" y="34" class="sparkline-empty">"sem dados"</text> }.into_any(),
        Some(g) => {
            let th = g.threshold_y.map(|y| {
                view! { <line x1="0" x2=W.to_string() y1=y.to_string() y2=y.to_string()
                    class="sparkline-threshold" stroke="var(--accent-red, #e5484d)" stroke-dasharray="4 3" stroke-width="1" /> }
            });
            let dots = g
                .anomaly_dots
                .into_iter()
                .map(|(x, y)| view! { <circle cx=x.to_string() cy=y.to_string() r="3.5" fill="var(--accent-red, #e5484d)" class="sparkline-anomaly" /> })
                .collect_view();
            view! {
                {th}
                <path d=g.path fill="none" stroke="var(--accent-blue, #3e63dd)" stroke-width="1.5" />
                {dots}
            }
            .into_any()
        }
    };
    view! {
        <div class="metric-card sentinel-chart" role="img"
            aria-label=format!("{label}: último valor {last} {unit}, {anomalies} anomalias")>
            <div class="sentinel-chart-header">
                <span class="metric-label">{label}</span>
                <span class="metric-value">{format!("{last} {unit}")}</span>
            </div>
            <svg viewBox=format!("0 0 {W} {H}") width="100%" height=H.to_string() preserveAspectRatio="none">
                {body}
            </svg>
            <span class="muted">{format!("limite {} {unit} · {anomalies} anomalias", fmt_value(threshold))}</span>
        </div>
    }
}

/// Dashboard em tempo real: sparklines por métrica + alertas + fila HITL.
#[component]
pub fn SentinelAnomalyCharts() -> impl IntoView {
    let (live, set_live) = signal(None::<Result<SentinelLive, String>>);
    let (remediations, set_remediations) = signal(Vec::<RemediationItem>::new());
    let (approver, set_approver) = signal(String::new());
    let (action_msg, set_action_msg) = signal(None::<String>);

    let refresh = move || {
        spawn_local(async move {
            set_live.set(Some(fetch_live().await));
            if let Ok(r) = fetch_remediations().await {
                set_remediations.set(r);
            }
        });
    };

    Effect::new(move |_| {
        refresh();
        spawn_local(async move {
            loop {
                gloo_timers::future::sleep(std::time::Duration::from_secs(POLL_SECS)).await;
                set_live.set(Some(fetch_live().await));
                if let Ok(r) = fetch_remediations().await {
                    set_remediations.set(r);
                }
            }
        });
    });

    let decide = move |id: String, approve: bool| {
        let who = approver.get_untracked();
        if who.trim().is_empty() {
            set_action_msg.set(Some("Informe o nome do aprovador antes de decidir.".into()));
            return;
        }
        spawn_local(async move {
            let msg = match post_decision(&id, &who, approve).await {
                Ok(()) => format!("{} {}", id, if approve { "aprovada" } else { "rejeitada" }),
                Err(e) => format!(
                    "Falha em {id}: {}",
                    crate::ui_error::report("sentinel decision", &e)
                ),
            };
            set_action_msg.set(Some(msg));
            if let Ok(r) = fetch_remediations().await {
                set_remediations.set(r);
            }
        });
    };

    view! {
        <section class="sentinel-live" aria-live="polite">
            <h3>"Anomalias em tempo real"</h3>
            {move || match live.get() {
                None => view! { <div class="skeleton-card pulse"></div> }.into_any(),
                Some(Err(e)) => view! {
                    <div class="error-banner"><p>{format!("Métricas do Sentinel indisponíveis: {}", crate::ui_error::report("sentinel metrics", &e))}</p></div>
                }.into_any(),
                Some(Ok(data)) => {
                    let charts = CHART_METRICS
                        .iter()
                        .map(|(key, label, unit, th)| {
                            let pts = data.series.get(*key).cloned().unwrap_or_default();
                            view! { <MetricSparkline label=*label unit=*unit threshold=*th points=pts /> }
                        })
                        .collect_view();
                    let mode = data.remediation_mode.clone();
                    let pending = data.pending_remediations;
                    let alerts = data.alerts.clone();
                    let alerts_view = if alerts.is_empty() {
                        view! { <p class="muted">"Nenhum alerta recente."</p> }.into_any()
                    } else {
                        alerts.into_iter().map(|a| view! {
                            <li class="sentinel-alert">
                                <strong>{a.severity.clone()}</strong>
                                {format!(" {} — valor {} (limite {})", a.rule_name, fmt_value(a.current_value), fmt_value(a.threshold))}
                            </li>
                        }).collect_view().into_any()
                    };
                    view! {
                        <div class="metrics-bar sentinel-charts">{charts}</div>
                        <p class="muted">{format!("Gate de remediação: {mode} · {pending} pendente(s) de aprovação · atualização a cada {POLL_SECS}s")}</p>
                        <h4>"Alertas recentes"</h4>
                        <ul class="sentinel-alert-list">{alerts_view}</ul>
                    }.into_any()
                }
            }}
            <h4>"Remediações (HITL)"</h4>
            <label>
                "Aprovador: "
                <input type="text" placeholder="seu nome / on-call"
                    prop:value=move || approver.get()
                    on:input=move |ev| set_approver.set(event_target_value(&ev)) />
            </label>
            {move || action_msg.get().map(|m| view! { <p class="muted">{m}</p> })}
            {move || {
                let items = remediations.get();
                if items.is_empty() {
                    return view! { <p class="muted">"Nenhuma remediação proposta."</p> }.into_any();
                }
                items.into_iter().rev().take(10).map(|r| {
                    let pending = r.status == "pending_approval";
                    let id_a = r.id.clone();
                    let id_r = r.id.clone();
                    let kind = r.action.get("kind").and_then(|k| k.as_str()).unwrap_or("?").to_string();
                    view! {
                        <div class="sentinel-remediation">
                            <span>{format!("{} · {} · {} · {}", r.id, r.rule_name, kind, r.status)}</span>
                            <Show when=move || pending>
                                <button class="cta-approve" on:click={let id = id_a.clone(); move |_| decide(id.clone(), true)}>"Aprovar"</button>
                                <button class="cta-reject" on:click={let id = id_r.clone(); move |_| decide(id.clone(), false)}>"Rejeitar"</button>
                            </Show>
                        </div>
                    }
                }).collect_view().into_any()
            }}
        </section>
    }
}

async fn fetch_live() -> Result<SentinelLive, String> {
    let resp = crate::auth::authed(gloo_net::http::Request::get(&format!(
        "{SENTINEL_API}/metrics"
    )))
    .send()
    .await
    .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json::<SentinelLive>().await.map_err(|e| e.to_string())
}

async fn fetch_remediations() -> Result<Vec<RemediationItem>, String> {
    let resp = crate::auth::authed(gloo_net::http::Request::get(&format!(
        "{SENTINEL_API}/remediations"
    )))
    .send()
    .await
    .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let v: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    serde_json::from_value(v.get("proposals").cloned().unwrap_or_default())
        .map_err(|e| e.to_string())
}

async fn post_decision(id: &str, approver: &str, approve: bool) -> Result<(), String> {
    let verb = if approve { "approve" } else { "reject" };
    let body = serde_json::json!({ "approver": approver, "reason": "decidido no cockpit" });
    let resp = crate::auth::authed(gloo_net::http::Request::post(&format!(
        "{SENTINEL_API}/remediations/{id}/{verb}"
    )))
    .json(&body)
    .map_err(|e| e.to_string())?
    .send()
    .await
    .map_err(|e| e.to_string())?;
    if resp.ok() {
        Ok(())
    } else {
        Err(crate::auth::approval_error(resp.status()))
    }
}

#[cfg(test)]
mod chart_tests {
    use super::*;

    fn pts(vals: &[(f64, bool)]) -> Vec<ChartPoint> {
        vals.iter()
            .enumerate()
            .map(|(i, (v, a))| ChartPoint {
                t: i as u64,
                v: *v,
                anomaly: *a,
            })
            .collect()
    }

    #[test]
    fn empty_series_has_no_geometry() {
        assert!(sparkline_geometry(&[], 80.0, 220.0, 60.0).is_none());
    }

    #[test]
    fn geometry_marks_anomalies_and_threshold() {
        let g = sparkline_geometry(
            &pts(&[(10.0, false), (20.0, false), (95.0, true)]),
            80.0,
            220.0,
            60.0,
        )
        .unwrap();
        assert!(g.path.starts_with("M4.0 "));
        assert_eq!(g.path.matches('L').count(), 2);
        assert_eq!(g.anomaly_dots.len(), 1);
        assert!((g.anomaly_dots[0].0 - 216.0).abs() < 1e-9);
        assert!((g.anomaly_dots[0].1 - 4.0).abs() < 1e-9, "máximo no topo");
        let ty = g.threshold_y.unwrap();
        assert!(ty > 4.0 && ty < 56.0);
        assert_eq!(g.min, 0.0);
        assert_eq!(g.max, 95.0);
    }

    #[test]
    fn flat_series_does_not_divide_by_zero() {
        let g = sparkline_geometry(&pts(&[(0.0, false)]), 0.0, 100.0, 50.0).unwrap();
        assert!(g.path.contains("50.0"));
        assert!(!g.path.contains("NaN"));
    }

    #[test]
    fn live_payload_deserializes_gateway_shape() {
        let raw = r#"{"current":null,"series":{"cpu_percent":[{"t":1,"v":12.5,"anomaly":false}]},
            "anomalies":[{"metric":"cpu_percent","value":99.0,"baseline_mean":10.0,"baseline_stddev":1.0,"z_score":89.0,"timestamp_ms":5}],
            "alerts":[{"rule_name":"cpu-spike","current_value":99.0,"threshold":80.0,"severity":"SEV-2","triggered_at_epoch_secs":5,"description":""}],
            "remediation_mode":"dry_run","pending_remediations":0,"rules":5}"#;
        let v: SentinelLive = serde_json::from_str(raw).unwrap();
        assert_eq!(v.series["cpu_percent"][0].v, 12.5);
        assert_eq!(v.alerts[0].severity, "SEV-2");
        assert_eq!(v.remediation_mode, "dry_run");
    }
}
