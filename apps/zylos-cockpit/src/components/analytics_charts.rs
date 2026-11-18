//! Analytics Charts — gráficos SVG puros para o Analytics Dashboard (Fase 11.10, T11.10.2).
//!
//! Sem dependências JS (Chart.js/D3/ECharts): cada gráfico é SVG gerado em
//! Rust pelo Leptos, o que mantém o bundle WASM pequeno e o render
//! determinístico. Tipos de gráfico: **linha**, **barras**, **heatmap**
//! (dia da semana × hora), **pizza** e **tabela**.
//!
//! A geometria (pontos da polilinha, fatias da pizza, cor do heatmap) fica em
//! funções puras testáveis fora do navegador.
//!
//! ## Related Documents
//! - Implementation Plan: [IMPLEMENTATION_PLAN_FASE11_MEGA.md](../../../../docs/IMPLEMENTATION_PLAN_FASE11_MEGA.md) §11.10
//! - Painel: [analytics_panel.rs](./analytics_panel.rs)

use leptos::prelude::*;
use serde::Deserialize;

/// Paleta categórica via CSS custom properties (ADR-008).
/// Valores definidos em `styles/design-tokens.css` como --chart-1..--chart-10.
pub const PALETTE: [&str; 10] = [
    "var(--chart-1)",
    "var(--chart-2)",
    "var(--chart-3)",
    "var(--chart-4)",
    "var(--chart-5)",
    "var(--chart-6)",
    "var(--chart-7)",
    "var(--chart-8)",
    "var(--chart-9)",
    "var(--chart-10)",
];

/// Rótulos dos dias (0 = segunda, como em `zylos_analytics::HeatmapCell`).
pub const WEEKDAYS: [&str; 7] = ["Seg", "Ter", "Qua", "Qui", "Sex", "Sáb", "Dom"];

/// Célula do heatmap (espelha `zylos_analytics::HeatmapCell`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct HeatCell {
    /// 0 = segunda … 6 = domingo.
    pub weekday: u8,
    /// Hora UTC 0–23.
    pub hour: u8,
    /// Execuções.
    pub count: u64,
    /// Taxa de sucesso 0..1.
    pub success_rate: f64,
}

/// Formata número compacto (1.2k, 3.4M).
pub fn compact(v: f64) -> String {
    let a = v.abs();
    if a >= 1_000_000.0 {
        format!("{:.1}M", v / 1_000_000.0)
    } else if a >= 10_000.0 {
        format!("{:.0}k", v / 1_000.0)
    } else if a >= 1_000.0 {
        format!("{:.1}k", v / 1_000.0)
    } else if a >= 10.0 || v == v.trunc() {
        format!("{v:.0}")
    } else {
        format!("{v:.2}")
    }
}

/// Máximo positivo (mínimo 1e-9 para evitar divisão por zero).
pub fn max_of(values: &[f64]) -> f64 {
    values.iter().copied().fold(0.0_f64, f64::max).max(1e-9)
}

/// Coordenadas (x, y) de cada ponto numa área `w×h` com margem `pad`.
pub fn line_coords(values: &[f64], w: f64, h: f64, pad: f64, max: f64) -> Vec<(f64, f64)> {
    let n = values.len();
    let iw = w - 2.0 * pad;
    let ih = h - 2.0 * pad;
    values
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let x = if n <= 1 {
                pad + iw / 2.0
            } else {
                pad + iw * i as f64 / (n - 1) as f64
            };
            let y = h - pad - ih * (v / max).clamp(0.0, 1.0);
            (x, y)
        })
        .collect()
}

/// Atributo `points` de `<polyline>`.
pub fn polyline_points(coords: &[(f64, f64)]) -> String {
    coords
        .iter()
        .map(|(x, y)| format!("{x:.1},{y:.1}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Path SVG de uma fatia de pizza de `a0` a `a1` (radianos, 0 = topo, horário).
pub fn pie_slice_path(cx: f64, cy: f64, r: f64, a0: f64, a1: f64) -> String {
    let span = a1 - a0;
    if span >= std::f64::consts::TAU - 1e-9 {
        // círculo completo: dois arcos
        return format!(
            "M {cx:.2} {:.2} A {r:.2} {r:.2} 0 1 1 {cx:.2} {:.2} A {r:.2} {r:.2} 0 1 1 {cx:.2} {:.2} Z",
            cy - r,
            cy + r,
            cy - r
        );
    }
    let p = |a: f64| (cx + r * a.sin(), cy - r * a.cos());
    let (x0, y0) = p(a0);
    let (x1, y1) = p(a1);
    let large = if span > std::f64::consts::PI { 1 } else { 0 };
    format!("M {cx:.2} {cy:.2} L {x0:.2} {y0:.2} A {r:.2} {r:.2} 0 {large} 1 {x1:.2} {y1:.2} Z")
}

/// Ângulos (início, fim) de cada fatia proporcional aos valores.
pub fn pie_angles(values: &[f64]) -> Vec<(f64, f64)> {
    let total: f64 = values.iter().filter(|v| **v > 0.0).sum();
    let mut acc = 0.0;
    values
        .iter()
        .map(|v| {
            let frac = if total > 0.0 { v.max(0.0) / total } else { 0.0 };
            let a0 = acc;
            acc += frac * std::f64::consts::TAU;
            (a0, acc)
        })
        .collect()
}

/// Cor do heatmap: intensidade 0..1 → azul com opacidade; taxa de sucesso
/// baixa (<70%) tinge de vermelho.
pub fn heat_fill(intensity: f64, success_rate: f64, count: u64) -> String {
    if count == 0 {
        return "rgba(148,163,184,0.12)".into();
    }
    let a = 0.15 + 0.85 * intensity.clamp(0.0, 1.0);
    if success_rate < 0.70 {
        format!("rgba(239,68,68,{a:.2})")
    } else {
        format!("rgba(79,140,255,{a:.2})")
    }
}

fn empty_chart(title: String) -> AnyView {
    view! {
        <figure class="an-chart an-empty">
            <figcaption>{title}</figcaption>
            <p class="an-empty-msg">"Sem dados para o filtro atual."</p>
        </figure>
    }
    .into_any()
}

/// Gráfico de linha (série temporal).
#[component]
pub fn LineChart(
    #[prop(into)] title: String,
    labels: Vec<String>,
    values: Vec<f64>,
    #[prop(into, optional)] unit: String,
    #[prop(optional)] max: Option<f64>,
    #[prop(into, optional)] color: Option<String>,
) -> impl IntoView {
    if values.is_empty() {
        return empty_chart(title);
    }
    let (w, h, pad) = (640.0, 220.0, 34.0);
    let max = max.unwrap_or_else(|| max_of(&values));
    let coords = line_coords(&values, w, h, pad, max);
    let pts = polyline_points(&coords);
    let color = color.unwrap_or_else(|| PALETTE[0].to_string());
    let area = format!(
        "{:.1},{:.1} {} {:.1},{:.1}",
        coords[0].0,
        h - pad,
        pts,
        coords[coords.len() - 1].0,
        h - pad
    );
    let step = (labels.len() / 8).max(1);
    let xlabels = coords
        .iter()
        .zip(labels.iter())
        .enumerate()
        .filter(|(i, _)| i % step == 0)
        .map(|(_, ((x, _), l))| {
            view! {
                <text x=format!("{x:.1}") y=format!("{:.1}", h - pad + 14.0)
                    text-anchor="middle" class="an-axis">{l.clone()}</text>
            }
        })
        .collect_view();
    let dots = if coords.len() <= 60 {
        coords
            .iter()
            .map(|(x, y)| {
                view! {
                    <circle cx=format!("{x:.1}") cy=format!("{y:.1}") r="2.5"
                        fill=color.clone()></circle>
                }
            })
            .collect_view()
            .into_any()
    } else {
        ().into_any()
    };
    let gridlines = (0..=4)
        .map(|k| {
            let v = max * k as f64 / 4.0;
            let y = h - pad - (h - 2.0 * pad) * k as f64 / 4.0;
            view! {
                <line x1=format!("{pad}") y1=format!("{y:.1}") x2=format!("{:.1}", w - pad)
                    y2=format!("{y:.1}") class="an-grid"></line>
                <text x=format!("{:.1}", pad - 4.0) y=format!("{:.1}", y + 3.0)
                    text-anchor="end" class="an-axis">{format!("{}{}", compact(v), unit)}</text>
            }
        })
        .collect_view();
    view! {
        <figure class="an-chart an-line" data-chart="line">
            <figcaption>{title}</figcaption>
            <svg viewBox=format!("0 0 {w} {h}") class="an-svg" role="img" preserveAspectRatio="none">
                {gridlines}
                <polygon points=area fill=color.clone() fill-opacity="0.12"></polygon>
                <polyline points=pts fill="none" stroke=color.clone() stroke-width="2"></polyline>
                {dots}
                {xlabels}
            </svg>
        </figure>
    }
    .into_any()
}

/// Gráfico de barras horizontal.
#[component]
pub fn BarChart(
    #[prop(into)] title: String,
    items: Vec<(String, f64)>,
    #[prop(into, optional)] unit: String,
    #[prop(optional)] max: Option<f64>,
    /// Valores abaixo deste limiar são destacados em vermelho.
    #[prop(optional)]
    warn_below: Option<f64>,
) -> impl IntoView {
    if items.is_empty() {
        return empty_chart(title);
    }
    let (w, row, label_w) = (640.0, 24.0, 130.0);
    let h = row * items.len() as f64 + 8.0;
    let vals: Vec<f64> = items.iter().map(|(_, v)| *v).collect();
    let max = max.unwrap_or_else(|| max_of(&vals));
    let bar_w = w - label_w - 70.0;
    let bars = items
        .into_iter()
        .enumerate()
        .map(|(i, (label, v))| {
            let y = 4.0 + i as f64 * row;
            let bw = (bar_w * (v / max).clamp(0.0, 1.0)).max(1.0);
            let fill = match warn_below {
                Some(t) if v < t => PALETTE[3],
                _ => PALETTE[i % PALETTE.len()],
            };
            view! {
                <text x=format!("{:.1}", label_w - 6.0) y=format!("{:.1}", y + 15.0)
                    text-anchor="end" class="an-label">{label}</text>
                <rect x=format!("{label_w}") y=format!("{:.1}", y + 3.0) width=format!("{bw:.1}")
                    height=format!("{:.1}", row - 6.0) rx="3" fill=fill></rect>
                <text x=format!("{:.1}", label_w + bw + 6.0) y=format!("{:.1}", y + 15.0)
                    class="an-value">{format!("{}{}", compact(v), unit)}</text>
            }
        })
        .collect_view();
    view! {
        <figure class="an-chart an-bar" data-chart="bar">
            <figcaption>{title}</figcaption>
            <svg viewBox=format!("0 0 {w} {h:.0}") class="an-svg" role="img">{bars}</svg>
        </figure>
    }
    .into_any()
}

/// Heatmap 7×24 (dia da semana × hora UTC).
#[component]
pub fn Heatmap(#[prop(into)] title: String, cells: Vec<HeatCell>) -> impl IntoView {
    if cells.iter().all(|c| c.count == 0) {
        return empty_chart(title);
    }
    let (cw, ch, left, top) = (24.0, 20.0, 36.0, 16.0);
    let w = left + 24.0 * cw + 4.0;
    let h = top + 7.0 * ch + 4.0;
    let max = cells.iter().map(|c| c.count).max().unwrap_or(1).max(1) as f64;
    let rects = cells
        .iter()
        .map(|c| {
            let x = left + c.hour as f64 * cw;
            let y = top + c.weekday as f64 * ch;
            let fill = heat_fill(c.count as f64 / max, c.success_rate, c.count);
            view! {
                <rect x=format!("{x:.1}") y=format!("{y:.1}") width=format!("{:.1}", cw - 2.0)
                    height=format!("{:.1}", ch - 2.0) rx="2" fill=fill
                    data-count=c.count.to_string()
                    data-success=format!("{:.3}", c.success_rate)></rect>
            }
        })
        .collect_view();
    let rows = WEEKDAYS
        .iter()
        .enumerate()
        .map(|(i, d)| {
            view! {
                <text x=format!("{:.1}", left - 6.0) y=format!("{:.1}", top + i as f64 * ch + 14.0)
                    text-anchor="end" class="an-axis">{*d}</text>
            }
        })
        .collect_view();
    let cols = (0..24)
        .step_by(3)
        .map(|hr| {
            view! {
                <text x=format!("{:.1}", left + hr as f64 * cw + cw / 2.0) y="11"
                    text-anchor="middle" class="an-axis">{format!("{hr:02}h")}</text>
            }
        })
        .collect_view();
    view! {
        <figure class="an-chart an-heatmap" data-chart="heatmap">
            <figcaption>{title}</figcaption>
            <svg viewBox=format!("0 0 {w:.0} {h:.0}") class="an-svg" role="img">
                {cols}
                {rows}
                {rects}
            </svg>
            <p class="an-legend-note">
                "Intensidade = volume · vermelho = sucesso < 70% · horas em UTC"
            </p>
        </figure>
    }
    .into_any()
}

/// Gráfico de pizza com legenda.
#[component]
pub fn PieChart(#[prop(into)] title: String, items: Vec<(String, f64)>) -> impl IntoView {
    let total: f64 = items.iter().map(|(_, v)| v.max(0.0)).sum();
    if items.is_empty() || total <= 0.0 {
        return empty_chart(title);
    }
    let vals: Vec<f64> = items.iter().map(|(_, v)| *v).collect();
    let angles = pie_angles(&vals);
    let (cx, cy, r) = (110.0, 110.0, 100.0);
    let slices = angles
        .iter()
        .enumerate()
        .filter(|(_, (a0, a1))| a1 > a0)
        .map(|(i, (a0, a1))| {
            view! {
                <path d=pie_slice_path(cx, cy, r, *a0, *a1) fill=PALETTE[i % PALETTE.len()]
                    stroke="var(--bg, #fff)" stroke-width="1"></path>
            }
        })
        .collect_view();
    let legend = items
        .into_iter()
        .enumerate()
        .map(|(i, (label, v))| {
            let style = format!("background:{}", PALETTE[i % PALETTE.len()]);
            view! {
                <li>
                    <span class="an-swatch" style=style></span>
                    {label}
                    " — "
                    {format!("{} ({:.1}%)", compact(v), v.max(0.0) / total * 100.0)}
                </li>
            }
        })
        .collect_view();
    view! {
        <figure class="an-chart an-pie" data-chart="pie">
            <figcaption>{title}</figcaption>
            <div class="an-pie-body">
                <svg viewBox="0 0 220 220" class="an-svg an-pie-svg" role="img">{slices}</svg>
                <ul class="an-legend">{legend}</ul>
            </div>
        </figure>
    }
    .into_any()
}

/// Tabela de dados.
#[component]
pub fn DataTable(
    #[prop(into)] title: String,
    columns: Vec<String>,
    rows: Vec<Vec<String>>,
) -> impl IntoView {
    if rows.is_empty() {
        return empty_chart(title);
    }
    let head = columns
        .into_iter()
        .map(|c| view! { <th>{c}</th> })
        .collect_view();
    let body = rows
        .into_iter()
        .map(|r| {
            let cells = r.into_iter().map(|c| view! { <td>{c}</td> }).collect_view();
            view! { <tr>{cells}</tr> }
        })
        .collect_view();
    view! {
        <figure class="an-chart an-table" data-chart="table">
            <figcaption>{title}</figcaption>
            <table class="an-data-table">
                <thead><tr>{head}</tr></thead>
                <tbody>{body}</tbody>
            </table>
        </figure>
    }
    .into_any()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_coords_scale() {
        let c = line_coords(&[0.0, 5.0, 10.0], 100.0, 100.0, 10.0, 10.0);
        assert_eq!(c[0], (10.0, 90.0));
        assert_eq!(c[2], (90.0, 10.0));
        assert_eq!(c[1], (50.0, 50.0));
        assert_eq!(polyline_points(&c[..1]), "10.0,90.0");
    }

    #[test]
    fn pie_angles_sum_to_tau() {
        let a = pie_angles(&[1.0, 1.0, 2.0]);
        assert!((a[2].1 - std::f64::consts::TAU).abs() < 1e-9);
        assert!((a[0].1 - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
        assert!(pie_slice_path(0.0, 0.0, 1.0, 0.0, std::f64::consts::TAU).contains(" A "));
    }

    #[test]
    fn heat_and_compact() {
        assert!(heat_fill(1.0, 0.5, 3).starts_with("rgba(239"));
        assert!(heat_fill(1.0, 0.9, 3).starts_with("rgba(79"));
        assert_eq!(compact(1_500.0), "1.5k");
        assert_eq!(compact(2_000_000.0), "2.0M");
        assert_eq!(compact(42.0), "42");
    }
}
