//! Analytics Panel — dashboard de analytics do Zylos (Fase 11.10, T11.10.2–T11.10.7).
//!
//! Consome `/api/v1/analytics/*` do gateway (crate `zylos-analytics`):
//! - **Filtros** (T11.10.3): período (24h/7d/30d/90d), granularidade
//!   (hora/dia/semana/mês), agente, skill e status (sucesso/falha).
//! - **Gráficos** (T11.10.2): linha (volume e taxa de sucesso), barras
//!   (sucesso por skill), heatmap (dia × hora), pizza (volume por agente) e
//!   tabela (detalhe por skill). Todos SVG puro — ver [`super::analytics_charts`].
//! - **Export** (T11.10.4): CSV dos dados visíveis e PDF dos relatórios preset.
//! - **Auto-melhoria** (T11.10.5): recomendações do feedback loop.
//! - **Relatórios preset** (T11.10.7): links JSON/CSV/PDF.
//!
//! Estados: Loading / Empty / Success / Error (padrão dos painéis do cockpit).
//!
//! ## Related Documents
//! - Implementation Plan: [IMPLEMENTATION_PLAN_FASE11_MEGA.md](../../../../docs/IMPLEMENTATION_PLAN_FASE11_MEGA.md) §11.10
//! - API: `apps/zylos-gateway/src/analytics_api.rs`

use super::analytics_charts::{BarChart, DataTable, HeatCell, Heatmap, LineChart, PieChart};
use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::Deserialize;

const API: &str = "http://localhost:3000/api/v1";

/// Bucket temporal / estatística agregada (espelha `zylos_analytics::Bucket`).
#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
pub struct Stats {
    /// Início do bucket (epoch s).
    #[serde(default)]
    pub start: i64,
    /// Execuções.
    pub count: u64,
    /// Sucessos.
    #[serde(default)]
    pub success: u64,
    /// Taxa de sucesso 0..1.
    pub success_rate: f64,
    /// Latência média.
    pub avg_latency_ms: f64,
    /// Latência máxima.
    #[serde(default)]
    pub max_latency_ms: u64,
    /// Custo.
    #[serde(default)]
    pub cost_usd: f64,
}

/// Agregado por chave.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Group {
    /// Agente ou skill.
    pub key: String,
    /// Estatísticas.
    pub stats: Stats,
}

/// Recomendação do feedback loop.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Recommendation {
    /// Skill.
    pub skill: String,
    /// Tipo (`evolve_skill`, ...).
    pub kind: String,
    /// Severidade.
    pub severity: String,
    /// Taxa de sucesso na janela.
    pub success_rate: f64,
    /// Amostras.
    pub samples: u64,
    /// Motivo.
    pub rationale: String,
    /// Ação sugerida.
    pub suggested_action: String,
}

#[derive(Deserialize)]
struct SummaryResp {
    summary: Stats,
}
#[derive(Deserialize)]
struct SeriesData {
    series: Vec<Stats>,
}
#[derive(Deserialize)]
struct SeriesResp {
    data: SeriesData,
}
#[derive(Deserialize)]
struct BreakdownResp {
    groups: Vec<Group>,
}
#[derive(Deserialize)]
struct HeatResp {
    cells: Vec<HeatCell>,
}
#[derive(Deserialize)]
struct DimsResp {
    agents: Vec<String>,
    skills: Vec<String>,
}
#[derive(Deserialize)]
struct RecsResp {
    recommendations: Vec<Recommendation>,
}
#[derive(Deserialize, Clone)]
struct PresetInfo {
    id: String,
    title: String,
}
#[derive(Deserialize)]
struct PresetsResp {
    reports: Vec<PresetInfo>,
}

/// Dados de um carregamento completo do dashboard.
#[derive(Debug, Clone, PartialEq, Default)]
struct DashData {
    summary: Stats,
    series: Vec<Stats>,
    by_skill: Vec<Group>,
    by_agent: Vec<Group>,
    heat: Vec<HeatCell>,
}

#[derive(Debug, Clone, PartialEq)]
enum LoadState {
    Loading,
    Empty,
    Success(Box<DashData>),
    Error(String),
}

/// Estado dos filtros (T11.10.3).
#[derive(Debug, Clone, PartialEq)]
pub struct FilterState {
    /// Período em dias.
    pub days: i64,
    /// `hourly|daily|weekly|monthly`.
    pub granularity: String,
    /// Agente ("" = todos).
    pub agent: String,
    /// Skill ("" = todas).
    pub skill: String,
    /// "" | "true" | "false".
    pub success: String,
}

impl Default for FilterState {
    fn default() -> Self {
        Self {
            days: 7,
            granularity: "daily".into(),
            agent: String::new(),
            skill: String::new(),
            success: String::new(),
        }
    }
}

fn enc(s: &str) -> String {
    js_sys::encode_uri_component(s)
        .as_string()
        .unwrap_or_default()
}

impl FilterState {
    /// Query string do filtro (sem `?`), relativa a `now` (epoch s).
    pub fn query(&self, now: i64) -> String {
        let mut q = vec![format!("since={}", now - self.days * 86_400)];
        if !self.agent.is_empty() {
            q.push(format!("agents={}", enc(&self.agent)));
        }
        if !self.skill.is_empty() {
            q.push(format!("skills={}", enc(&self.skill)));
        }
        if !self.success.is_empty() {
            q.push(format!("success={}", self.success));
        }
        q.join("&")
    }
}

fn now_secs() -> i64 {
    (js_sys::Date::now() / 1000.0) as i64
}

async fn get_json<T: for<'de> Deserialize<'de>>(path: &str) -> Result<T, String> {
    let resp = crate::auth::authed(gloo_net::http::Request::get(&format!("{API}{path}")))
        .send()
        .await
        .map_err(|e| format!("{e:?}"))?;
    if !resp.ok() {
        return Err(format!("HTTP {} em {path}", resp.status()));
    }
    resp.json::<T>().await.map_err(|e| format!("{e:?}"))
}

async fn load_dashboard(f: FilterState) -> Result<DashData, String> {
    let q = f.query(now_secs());
    let summary: SummaryResp = get_json(&format!("/analytics/summary?{q}")).await?;
    let series: SeriesResp = get_json(&format!(
        "/analytics/timeseries?{q}&granularity={}",
        f.granularity
    ))
    .await?;
    let by_skill: BreakdownResp = get_json(&format!("/analytics/breakdown?{q}&by=skill")).await?;
    let by_agent: BreakdownResp = get_json(&format!("/analytics/breakdown?{q}&by=agent")).await?;
    let heat: HeatResp = get_json(&format!("/analytics/heatmap?{q}")).await?;
    Ok(DashData {
        summary: summary.summary,
        series: series.data.series,
        by_skill: by_skill.groups,
        by_agent: by_agent.groups,
        heat: heat.cells,
    })
}

/// Rótulo do eixo X conforme granularidade (UTC).
fn bucket_label(ts: i64, granularity: &str) -> String {
    let d = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(ts as f64 * 1000.0));
    let (mo, day, h) = (d.get_utc_month() + 1, d.get_utc_date(), d.get_utc_hours());
    match granularity {
        "hourly" => format!("{day:02}/{mo:02} {h:02}h"),
        "monthly" => format!("{mo:02}/{}", d.get_utc_full_year()),
        _ => format!("{day:02}/{mo:02}"),
    }
}

fn pct(r: f64) -> String {
    format!("{:.1}%", r * 100.0)
}

/// Painel principal do Analytics Dashboard.
#[component]
pub fn AnalyticsPanel() -> impl IntoView {
    let (filters, set_filters) = signal(FilterState::default());
    let (state, set_state) = signal(LoadState::Loading);
    let (agents, set_agents) = signal(Vec::<String>::new());
    let (skills, set_skills) = signal(Vec::<String>::new());
    let (recs, set_recs) = signal(Vec::<Recommendation>::new());
    let (presets, set_presets) = signal(Vec::<PresetInfo>::new());

    // dimensões, recomendações e presets: uma vez
    spawn_local(async move {
        if let Ok(d) = get_json::<DimsResp>("/analytics/dimensions").await {
            set_agents.set(d.agents);
            set_skills.set(d.skills);
        }
        if let Ok(r) = get_json::<RecsResp>("/analytics/recommendations").await {
            set_recs.set(r.recommendations);
        }
        if let Ok(p) = get_json::<PresetsResp>("/analytics/reports").await {
            set_presets.set(p.reports);
        }
    });

    // recarrega a cada mudança de filtro
    Effect::new(move |_| {
        let f = filters.get();
        set_state.set(LoadState::Loading);
        spawn_local(async move {
            let next = match load_dashboard(f).await {
                Ok(d) if d.summary.count == 0 => LoadState::Empty,
                Ok(d) => LoadState::Success(Box::new(d)),
                Err(e) => LoadState::Error(e),
            };
            set_state.set(next);
        });
    });

    let csv_href = move |kind: &str| {
        let f = filters.get();
        format!(
            "{API}/analytics/export?kind={kind}&format=csv&granularity={}&{}",
            f.granularity,
            f.query(now_secs())
        )
    };

    let body = move || match state.get() {
        LoadState::Loading => {
            view! { <div class="an-state an-loading">"Carregando analytics…"</div> }.into_any()
        }
        LoadState::Empty => view! {
            <div class="an-state an-empty">"Nenhum evento no período/filtro selecionado."</div>
        }
        .into_any(),
        LoadState::Error(e) => view! {
            <div class="an-state an-error">"Erro ao carregar analytics: " {e}</div>
        }
        .into_any(),
        LoadState::Success(d) => {
            let g = filters.get_untracked().granularity;
            let labels: Vec<String> = d.series.iter().map(|b| bucket_label(b.start, &g)).collect();
            let counts: Vec<f64> = d.series.iter().map(|b| b.count as f64).collect();
            let rates: Vec<f64> = d.series.iter().map(|b| b.success_rate * 100.0).collect();
            let skill_bars: Vec<(String, f64)> = d
                .by_skill
                .iter()
                .map(|s| (s.key.clone(), s.stats.success_rate * 100.0))
                .collect();
            let agent_pie: Vec<(String, f64)> = d
                .by_agent
                .iter()
                .map(|a| (a.key.clone(), a.stats.count as f64))
                .collect();
            let table_rows: Vec<Vec<String>> = d
                .by_skill
                .iter()
                .map(|s| {
                    vec![
                        s.key.clone(),
                        s.stats.count.to_string(),
                        pct(s.stats.success_rate),
                        format!("{:.0} ms", s.stats.avg_latency_ms),
                        format!("{} ms", s.stats.max_latency_ms),
                        format!("{:.2}", s.stats.cost_usd),
                    ]
                })
                .collect();
            let s = d.summary.clone();
            view! {
                <div class="an-kpis">
                    <div class="an-kpi"><span>"Execuções"</span><strong>{s.count.to_string()}</strong></div>
                    <div class="an-kpi"><span>"Sucesso"</span><strong>{pct(s.success_rate)}</strong></div>
                    <div class="an-kpi"><span>"Latência média"</span><strong>{format!("{:.0} ms", s.avg_latency_ms)}</strong></div>
                    <div class="an-kpi"><span>"Custo"</span><strong>{format!("US$ {:.2}", s.cost_usd)}</strong></div>
                </div>
                <div class="an-grid-charts">
                    <LineChart title="Execuções por período" labels=labels.clone() values=counts />
                    <LineChart title="Taxa de sucesso (%)" labels=labels values=rates unit="%"
                        max=100.0 color="#22c55e" />
                    <BarChart title="Sucesso por skill (%)" items=skill_bars unit="%" max=100.0
                        warn_below=70.0 />
                    <PieChart title="Volume por agente" items=agent_pie />
                    <Heatmap title="Atividade: dia da semana × hora (UTC)" cells=d.heat.clone() />
                    <DataTable title="Detalhe por skill"
                        columns=["Skill", "Execuções", "Sucesso", "Lat. média", "Lat. máx", "Custo USD"]
                            .map(String::from).to_vec()
                        rows=table_rows />
                </div>
            }
            .into_any()
        }
    };

    let select_opts = |items: Vec<String>, current: String| {
        items
            .into_iter()
            .map(|i| {
                let sel = i == current;
                view! { <option value=i.clone() selected=sel>{i.clone()}</option> }
            })
            .collect_view()
    };

    view! {
        <section class="analytics-panel" aria-label="Analytics Dashboard">
            <header class="an-header">
                <h2>"Analytics"</h2>
                <p class="an-filter-desc">
                    {move || {
                        let f = filters.get();
                        format!(
                            "Últimos {} dias · {} · agente: {} · skill: {}",
                            f.days,
                            f.granularity,
                            if f.agent.is_empty() { "todos".to_string() } else { f.agent.clone() },
                            if f.skill.is_empty() { "todas".to_string() } else { f.skill.clone() },
                        )
                    }}
                </p>
            </header>

            <form class="an-filters" on:submit=|ev| ev.prevent_default()>
                <label>"Período"
                    <select on:change=move |ev| {
                        let v = event_target_value(&ev).parse().unwrap_or(7);
                        set_filters.update(|f| f.days = v);
                    }>
                        <option value="1">"24 h"</option>
                        <option value="7" selected=true>"7 dias"</option>
                        <option value="30">"30 dias"</option>
                        <option value="90">"90 dias"</option>
                    </select>
                </label>
                <label>"Granularidade"
                    <select on:change=move |ev| {
                        let v = event_target_value(&ev);
                        set_filters.update(|f| f.granularity = v);
                    }>
                        <option value="hourly">"Hora"</option>
                        <option value="daily" selected=true>"Dia"</option>
                        <option value="weekly">"Semana"</option>
                        <option value="monthly">"Mês"</option>
                    </select>
                </label>
                <label>"Agente"
                    <select on:change=move |ev| {
                        let v = event_target_value(&ev);
                        set_filters.update(|f| f.agent = v);
                    }>
                        <option value="">"Todos"</option>
                        {move || select_opts(agents.get(), filters.get_untracked().agent)}
                    </select>
                </label>
                <label>"Skill"
                    <select on:change=move |ev| {
                        let v = event_target_value(&ev);
                        set_filters.update(|f| f.skill = v);
                    }>
                        <option value="">"Todas"</option>
                        {move || select_opts(skills.get(), filters.get_untracked().skill)}
                    </select>
                </label>
                <label>"Status"
                    <select on:change=move |ev| {
                        let v = event_target_value(&ev);
                        set_filters.update(|f| f.success = v);
                    }>
                        <option value="">"Todos"</option>
                        <option value="true">"Sucesso"</option>
                        <option value="false">"Falha"</option>
                    </select>
                </label>
                <div class="an-export">
                    <a class="btn" href=move || csv_href("timeseries") download="">"CSV série"</a>
                    <a class="btn" href=move || csv_href("breakdown") download="">"CSV por skill"</a>
                    <a class="btn" href=move || csv_href("events") download="">"CSV eventos"</a>
                </div>
            </form>

            <div class="an-body">{body}</div>

            <section class="an-recs" aria-label="Recomendações de auto-melhoria">
                <h3>"Auto-melhoria: recomendações"</h3>
                {move || {
                    let r = recs.get();
                    if r.is_empty() {
                        view! { <p class="an-state">"Nenhuma skill abaixo dos limiares."</p> }.into_any()
                    } else {
                        r.into_iter()
                            .map(|x| {
                                let cls = format!("an-rec an-sev-{}", x.severity);
                                view! {
                                    <article class=cls>
                                        <strong>{x.skill.clone()}</strong>
                                        " · " {x.kind.clone()} " · " {x.severity.clone()}
                                        " · sucesso " {pct(x.success_rate)}
                                        " (" {x.samples.to_string()} " amostras)"
                                        <p>{x.rationale.clone()}</p>
                                        <p class="an-action">{x.suggested_action.clone()}</p>
                                    </article>
                                }
                            })
                            .collect_view()
                            .into_any()
                    }
                }}
            </section>

            <section class="an-presets" aria-label="Relatórios preset">
                <h3>"Relatórios preset"</h3>
                <ul>
                    {move || {
                        presets
                            .get()
                            .into_iter()
                            .map(|p| {
                                let base = format!("{API}/analytics/reports/{}", p.id);
                                view! {
                                    <li>
                                        {p.title.clone()} " — "
                                        <a href=format!("{base}?format=pdf") download="">"PDF"</a> " · "
                                        <a href=format!("{base}?format=csv") download="">"CSV"</a> " · "
                                        <a href=format!("{base}?format=json") target="_blank">"JSON"</a>
                                    </li>
                                }
                            })
                            .collect_view()
                    }}
                </ul>
            </section>
        </section>
    }
}
