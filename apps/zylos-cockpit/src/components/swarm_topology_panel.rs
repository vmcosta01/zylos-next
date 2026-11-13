//! Swarm Topology Graph — visualização SVG interativa da topologia do swarm
//!
//! Fase 10.5: Drag-and-drop de nós, zoom/pan, persistência localStorage,
//! polling 5s, tooltip on hover, animação de arestas ativas.
//! Layout force-directed simplificado com posições calculadas deterministicamente.
//! Sem dependências externas de grafo; SVG inline no view! macro.

use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Tipo de nó na topologia do swarm
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeType {
    Agent,
    Orchestrator,
    Gateway,
}

impl NodeType {
    /// CSS class específica para cada tipo de nó
    pub fn css_class(&self) -> &'static str {
        match self {
            NodeType::Agent => "node-agent",
            NodeType::Orchestrator => "node-orchestrator",
            NodeType::Gateway => "node-gateway",
        }
    }

    /// Cor de preenchimento SVG por tipo
    pub fn fill_color(&self) -> &'static str {
        match self {
            NodeType::Agent => "#3b82f6",
            NodeType::Orchestrator => "#f59e0b",
            NodeType::Gateway => "#10b981",
        }
    }

    /// Raio do círculo SVG por tipo
    pub fn radius(&self) -> f64 {
        match self {
            NodeType::Agent => 18.0,
            NodeType::Orchestrator => 26.0,
            NodeType::Gateway => 22.0,
        }
    }

    /// Descrição legível para tooltip
    pub fn description(&self) -> &'static str {
        match self {
            NodeType::Agent => "Agente executor de tarefas",
            NodeType::Orchestrator => "Orquestrador de workflow",
            NodeType::Gateway => "Gateway de comunicação",
        }
    }
}

/// Status operacional de um nó
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeStatus {
    Online,
    Degraded,
    Offline,
}

impl NodeStatus {
    /// Cor do anel de status ao redor do nó
    pub fn stroke_color(&self) -> &'static str {
        match self {
            NodeStatus::Online => "#22c55e",
            NodeStatus::Degraded => "#eab308",
            NodeStatus::Offline => "#ef4444",
        }
    }

    /// Label legível para tooltip
    pub fn label(&self) -> &'static str {
        match self {
            NodeStatus::Online => "Online",
            NodeStatus::Degraded => "Degradado",
            NodeStatus::Offline => "Offline",
        }
    }
}

/// Tipo de conexão entre nós
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EdgeType {
    Control,
    Data,
    Heartbeat,
}

impl EdgeType {
    /// Estilo de traço SVG por tipo de aresta
    pub fn dash_array(&self) -> &'static str {
        match self {
            EdgeType::Control => "none",
            EdgeType::Data => "8,4",
            EdgeType::Heartbeat => "3,3",
        }
    }

    /// Se a aresta deve ter animação de pulse
    pub fn is_animated(&self) -> bool {
        matches!(self, EdgeType::Heartbeat | EdgeType::Data)
    }
}

/// Nó na topologia do swarm
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TopologyNode {
    pub id: String,
    pub label: String,
    pub node_type: NodeType,
    pub status: NodeStatus,
    pub x: f64,
    pub y: f64,
}

/// Aresta na topologia do swarm
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TopologyEdge {
    pub source: String,
    pub target: String,
    pub edge_type: EdgeType,
    pub weight: f64,
}

/// Estatísticas agregadas da topologia
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TopologyStats {
    pub total_nodes: usize,
    pub online_nodes: usize,
    pub total_edges: usize,
    pub avg_weight: f64,
}

/// Dados completos da topologia
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TopologyData {
    pub nodes: Vec<TopologyNode>,
    pub edges: Vec<TopologyEdge>,
    pub stats: TopologyStats,
}

/// Estado de transformação SVG (zoom + pan)
#[derive(Debug, Clone, Copy, PartialEq)]
struct SvgTransform {
    scale: f64,
    translate_x: f64,
    translate_y: f64,
}

impl Default for SvgTransform {
    fn default() -> Self {
        Self {
            scale: 1.0,
            translate_x: 0.0,
            translate_y: 0.0,
        }
    }
}

/// Estado de drag-and-drop
#[derive(Debug, Clone, PartialEq)]
struct DragState {
    active: bool,
    node_id: Option<String>,
    offset_x: f64,
    offset_y: f64,
}

impl Default for DragState {
    fn default() -> Self {
        Self {
            active: false,
            node_id: None,
            offset_x: 0.0,
            offset_y: 0.0,
        }
    }
}

/// Estado do tooltip
#[derive(Debug, Clone, PartialEq)]
struct TooltipState {
    visible: bool,
    node_id: Option<String>,
    x: f64,
    y: f64,
}

impl Default for TooltipState {
    fn default() -> Self {
        Self {
            visible: false,
            node_id: None,
            x: 0.0,
            y: 0.0,
        }
    }
}

const LOCAL_STORAGE_KEY: &str = "zylos-topology-positions";
const POLL_INTERVAL_SECS: u64 = 5;
const MIN_SCALE: f64 = 0.3;
const MAX_SCALE: f64 = 3.0;

/// Calcula layout determinístico circular para os nós.
fn compute_layout(nodes: &mut [TopologyNode]) {
    let n = nodes.len();
    if n == 0 {
        return;
    }
    let cx = 400.0;
    let cy = 250.0;
    let base_radius = 120.0;
    for (i, node) in nodes.iter_mut().enumerate() {
        let angle = 2.0 * std::f64::consts::PI * (i as f64) / (n as f64);
        let r = match node.node_type {
            NodeType::Orchestrator => base_radius * 0.5,
            NodeType::Gateway => base_radius * 0.75,
            NodeType::Agent => base_radius + (i as f64 * 8.0),
        };
        node.x = cx + r * angle.cos();
        node.y = cy + r * angle.sin();
    }
}

/// Carrega posições customizadas do localStorage
fn load_custom_positions() -> HashMap<String, (f64, f64)> {
    let window = match web_sys::window() {
        Some(w) => w,
        None => return HashMap::new(),
    };
    let storage = match window.local_storage() {
        Ok(Some(s)) => s,
        _ => return HashMap::new(),
    };
    let raw = match storage.get_item(LOCAL_STORAGE_KEY) {
        Ok(Some(v)) => v,
        _ => return HashMap::new(),
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

/// Salva posições customizadas no localStorage
fn save_custom_positions(positions: &HashMap<String, (f64, f64)>) {
    let window = match web_sys::window() {
        Some(w) => w,
        None => return,
    };
    let storage = match window.local_storage() {
        Ok(Some(s)) => s,
        _ => return,
    };
    if let Ok(json) = serde_json::to_string(positions) {
        let _ = storage.set_item(LOCAL_STORAGE_KEY, &json);
    }
}

/// Limpa posições customizadas do localStorage (para POP reset-layout)
pub fn reset_topology_layout() {
    let window = match web_sys::window() {
        Some(w) => w,
        None => return,
    };
    let storage = match window.local_storage() {
        Ok(Some(s)) => s,
        _ => return,
    };
    let _ = storage.remove_item(LOCAL_STORAGE_KEY);
}

/// Aplica posições customizadas salvas sobre o layout computado
fn apply_custom_positions(nodes: &mut [TopologyNode]) {
    let custom = load_custom_positions();
    for node in nodes.iter_mut() {
        if let Some(&(x, y)) = custom.get(&node.id) {
            node.x = x;
            node.y = y;
        }
    }
}

async fn fetch_json(url: &str) -> Result<serde_json::Value, String> {
    let full_url = format!("http://localhost:3000{}", url);
    let resp = gloo_net::http::Request::get(&full_url)
        .send()
        .await
        .map_err(|e| format!("fetch error: {e}"))?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json::<serde_json::Value>()
        .await
        .map_err(|e| format!("json parse: {e}"))
}

/// Resolve coordenadas de um nó por ID para desenhar arestas
fn find_node_pos(nodes: &[TopologyNode], id: &str) -> Option<(f64, f64)> {
    nodes.iter().find(|n| n.id == id).map(|n| (n.x, n.y))
}

/// Converte coordenadas de tela para coordenadas SVG considerando transform
fn screen_to_svg(
    client_x: f64,
    client_y: f64,
    svg_rect: &web_sys::DomRect,
    transform: &SvgTransform,
) -> (f64, f64) {
    let svg_x =
        (client_x - svg_rect.left()) / transform.scale - transform.translate_x / transform.scale;
    let svg_y =
        (client_y - svg_rect.top()) / transform.scale - transform.translate_y / transform.scale;
    (svg_x, svg_y)
}

/// Componente principal do painel de topologia do swarm
#[component]
pub fn SwarmTopologyPanel() -> impl IntoView {
    let data = RwSignal::new(None::<TopologyData>);
    let loading = RwSignal::new(true);
    let error_msg = RwSignal::new(None::<String>);
    let transform = RwSignal::new(SvgTransform::default());
    let drag = RwSignal::new(DragState::default());
    let tooltip = RwSignal::new(TooltipState::default());
    let custom_positions = RwSignal::new(load_custom_positions());

    // Polling a cada 5s
    Effect::new(move |_| {
        spawn_local(async move {
            loop {
                loading.set(true);
                error_msg.set(None);
                match fetch_json("/api/v1/hermes/bot/spawn-crew").await {
                    Ok(json) => {
                        let mut td: TopologyData = serde_json::from_value(json.clone())
                            .unwrap_or_else(|_| TopologyData {
                                nodes: vec![],
                                edges: vec![],
                                stats: TopologyStats {
                                    total_nodes: 0,
                                    online_nodes: 0,
                                    total_edges: 0,
                                    avg_weight: 0.0,
                                },
                            });
                        compute_layout(&mut td.nodes);
                        apply_custom_positions(&mut td.nodes);
                        data.set(Some(td));
                    }
                    Err(e) => {
                        if data.get().is_none() {
                            error_msg.set(Some(format!("Gateway indisponível: {e}")));
                        }
                    }
                }
                loading.set(false);
                gloo_timers::future::sleep(std::time::Duration::from_secs(POLL_INTERVAL_SECS))
                    .await;
            }
        });
    });

    // Handler para mousedown em nó — inicia drag
    let on_node_mousedown = move |ev: web_sys::MouseEvent, node_id: String| {
        ev.prevent_default();
        let svg_el = match web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id("topology-svg"))
        {
            Some(el) => el,
            None => return,
        };
        let rect = svg_el.get_bounding_client_rect();
        let t = transform.get();
        let (svg_x, svg_y) = screen_to_svg(
            ev.client_x() as f64,
            ev.client_y() as f64,
            &rect,
            &t,
        );

        if let Some(ref d) = data.get() {
            if let Some(node) = d.nodes.iter().find(|n| n.id == node_id) {
                drag.set(DragState {
                    active: true,
                    node_id: Some(node_id),
                    offset_x: svg_x - node.x,
                    offset_y: svg_y - node.y,
                });
            }
        }
    };

    // Handler para mousemove no SVG — drag ativo
    let on_svg_mousemove = move |ev: web_sys::MouseEvent| {
        if drag.get().active {
            ev.prevent_default();
            let svg_el = match web_sys::window()
                .and_then(|w| w.document())
                .and_then(|d| d.get_element_by_id("topology-svg"))
            {
                Some(el) => el,
                None => return,
            };
            let rect = svg_el.get_bounding_client_rect();
            let t = transform.get();
            let (svg_x, svg_y) = screen_to_svg(
                ev.client_x() as f64,
                ev.client_y() as f64,
                &rect,
                &t,
            );

            let d = drag.get();
            if let Some(ref node_id) = d.node_id {
                let new_x = svg_x - d.offset_x;
                let new_y = svg_y - d.offset_y;

                // Atualizar posição no data signal
                if let Some(ref mut td) = data.write().as_mut() {
                    if let Some(node) = td.nodes.iter_mut().find(|n| n.id == *node_id) {
                        node.x = new_x;
                        node.y = new_y;
                    }
                }

                // Persistir no custom_positions
                let mut positions = custom_positions.get();
                positions.insert(node_id.clone(), (new_x, new_y));
                custom_positions.set(positions);
            }
        }
    };

    // Handler para mouseup — finaliza drag e salva
    let on_svg_mouseup = move |_ev: web_sys::MouseEvent| {
        if drag.get().active {
            save_custom_positions(&custom_positions.get());
            drag.set(DragState::default());
        }
    };

    // Handler para wheel — zoom
    let on_svg_wheel = move |ev: web_sys::WheelEvent| {
        ev.prevent_default();
        let delta = ev.delta_y();
        let mut t = transform.get();
        let factor = if delta < 0.0 { 1.1 } else { 0.9 };
        let new_scale = (t.scale * factor).clamp(MIN_SCALE, MAX_SCALE);

        let svg_el = match web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id("topology-svg"))
        {
            Some(el) => el,
            None => return,
        };
        let rect = svg_el.get_bounding_client_rect();
        let mx = ev.client_x() as f64 - rect.left();
        let my = ev.client_y() as f64 - rect.top();

        let ratio = new_scale / t.scale;
        t.translate_x = mx - ratio * (mx - t.translate_x);
        t.translate_y = my - ratio * (my - t.translate_y);
        t.scale = new_scale;
        transform.set(t);
    };

    // Handler para tooltip on hover
    let on_node_mouseenter = move |ev: web_sys::MouseEvent, node_id: String| {
        tooltip.set(TooltipState {
            visible: true,
            node_id: Some(node_id),
            x: ev.client_x() as f64,
            y: ev.client_y() as f64,
        });
    };

    let on_node_mouseleave = move |_ev: web_sys::MouseEvent| {
        tooltip.set(TooltipState::default());
    };

    // Reset layout handler
    let on_reset_click = move |_ev: web_sys::MouseEvent| {
        reset_topology_layout();
        custom_positions.set(HashMap::new());
        transform.set(SvgTransform::default());
        // Forçar re-fetch aplicando layout padrão
        if let Some(ref mut td) = data.write().as_mut() {
            compute_layout(&mut td.nodes);
        }
    };

    view! {
        <div class="swarm-topology-panel">
            <style>
                {r#"
                @keyframes edge-pulse {
                    0% { stroke-opacity: 0.4; stroke-dashoffset: 0; }
                    50% { stroke-opacity: 0.9; stroke-dashoffset: 12; }
                    100% { stroke-opacity: 0.4; stroke-dashoffset: 24; }
                }
                .topology-edge-animated {
                    animation: edge-pulse 1.5s ease-in-out infinite;
                }
                .topology-node { cursor: grab; transition: filter 0.15s ease; }
                .topology-node:hover { filter: brightness(1.2) drop-shadow(0 0 6px rgba(255,255,255,0.3)); }
                .topology-node.dragging { cursor: grabbing; }
                .topology-tooltip {
                    position: fixed;
                    background: #1e293b;
                    border: 1px solid #475569;
                    border-radius: 8px;
                    padding: 10px 14px;
                    color: #e2e8f0;
                    font-size: 12px;
                    line-height: 1.5;
                    pointer-events: none;
                    z-index: 1000;
                    box-shadow: 0 4px 12px rgba(0,0,0,0.4);
                    max-width: 240px;
                }
                .topology-tooltip .tt-label { font-weight: 600; font-size: 13px; margin-bottom: 4px; }
                .topology-tooltip .tt-type { color: #94a3b8; font-size: 11px; }
                .topology-tooltip .tt-status { margin-top: 4px; font-size: 11px; }
                .topology-controls {
                    display: flex; gap: 6px; margin-bottom: 8px; justify-content: flex-end;
                }
                .topology-controls button {
                    background: #334155; border: 1px solid #475569; color: #e2e8f0;
                    border-radius: 4px; padding: 4px 10px; font-size: 12px; cursor: pointer;
                }
                .topology-controls button:hover { background: #475569; }
                "#}
            </style>

            {move || {
                if loading.get() && data.get().is_none() {
                    view! {
                        <div class="topology-loading">
                            <span class="spinner"></span>
                            <span>"Carregando topologia..."</span>
                        </div>
                    }.into_any()
                } else if let Some(ref err) = error_msg.get() {
                    view! {
                        <div class="topology-error">
                            <span class="error-icon">"\u{26a0}\u{fe0f}"</span>
                            <span>{err.clone()}</span>
                        </div>
                    }.into_any()
                } else if let Some(ref d) = data.get() {
                    let stats = d.stats.clone();
                    let nodes = d.nodes.clone();
                    let edges = d.edges.clone();

                    let total_nodes = stats.total_nodes.to_string();
                    let online_nodes = stats.online_nodes.to_string();
                    let total_edges = stats.total_edges.to_string();
                    let avg_weight = format!("{:.2}", stats.avg_weight);

                    let t = transform.get();
                    let transform_str = format!(
                        "translate({},{}) scale({})",
                        t.translate_x, t.translate_y, t.scale
                    );

                    // Renderizar arestas
                    let edge_views: Vec<_> = edges
                        .iter()
                        .filter_map(|edge| {
                            let (x1, y1) = find_node_pos(&nodes, &edge.source)?;
                            let (x2, y2) = find_node_pos(&nodes, &edge.target)?;
                            let stroke_width = (edge.weight * 3.0).max(1.0);
                            let dash = edge.edge_type.dash_array();
                            let animated = edge.edge_type.is_animated();
                            let css = if animated {
                                "topology-edge topology-edge-animated"
                            } else {
                                "topology-edge"
                            };
                            Some(view! {
                                <line
                                    class=css
                                    x1=x1.to_string()
                                    y1=y1.to_string()
                                    x2=x2.to_string()
                                    y2=y2.to_string()
                                    stroke="#94a3b8"
                                    stroke-width=stroke_width.to_string()
                                    stroke-dasharray=dash
                                    opacity="0.6"
                                />
                            })
                        })
                        .collect::<Vec<_>>();

                    // Renderizar nós
                    let node_views: Vec<_> = nodes
                        .iter()
                        .map(|node| {
                            let r = node.node_type.radius();
                            let fill = node.node_type.fill_color();
                            let stroke = node.status.stroke_color();
                            let css = format!("topology-node {}", node.node_type.css_class());
                            let label_y = node.y + r + 14.0;
                            let label = node.label.clone();
                            let nid = node.id.clone();
                            let nid_hover = node.id.clone();
                            view! {
                                <g
                                    class=css
                                    on:mousedown=move |ev| on_node_mousedown(ev, nid.clone())
                                    on:mouseenter=move |ev| on_node_mouseenter(ev, nid_hover.clone())
                                    on:mouseleave=on_node_mouseleave
                                >
                                    <circle
                                        cx=node.x.to_string()
                                        cy=node.y.to_string()
                                        r=r.to_string()
                                        fill=fill
                                        stroke=stroke
                                        stroke-width="3"
                                    />
                                    <text
                                        x=node.x.to_string()
                                        y=label_y.to_string()
                                        text-anchor="middle"
                                        fill="#e2e8f0"
                                        font-size="11"
                                        font-family="system-ui, sans-serif"
                                        style="pointer-events: none; user-select: none;"
                                    >
                                        {label}
                                    </text>
                                </g>
                            }
                        })
                        .collect::<Vec<_>>();

                    // Tooltip render
                    let tt = tooltip.get();
                    let tooltip_view = if tt.visible {
                        if let Some(ref nid) = tt.node_id {
                            if let Some(node) = nodes.iter().find(|n| n.id == *nid) {
                                let tt_left = format!("{}px", tt.x + 12.0);
                                let tt_top = format!("{}px", tt.y - 10.0);
                                let desc = node.node_type.description();
                                let status_label = node.status.label();
                                let status_color = node.status.stroke_color();
                                let node_label = node.label.clone();
                                view! {
                                    <div
                                        class="topology-tooltip"
                                        style=format!("left:{};top:{}", tt_left, tt_top)
                                    >
                                        <div class="tt-label">{node_label}</div>
                                        <div class="tt-type">{desc}</div>
                                        <div class="tt-status" style=format!("color:{}", status_color)>
                                            {status_label}
                                        </div>
                                    </div>
                                }.into_any()
                            } else {
                                ().into_any()
                            }
                        } else {
                            ().into_any()
                        }
                    } else {
                        ().into_any()
                    };

                    view! {
                        <>
                            <div class="topology-stats">
                                <div class="metric-card">
                                    <span class="metric-label">"N\u{00f3}s Totais"</span>
                                    <span class="metric-value">{total_nodes}</span>
                                </div>
                                <div class="metric-card">
                                    <span class="metric-label">"Online"</span>
                                    <span class="metric-value success">{online_nodes}</span>
                                </div>
                                <div class="metric-card">
                                    <span class="metric-label">"Conex\u{00f5}es"</span>
                                    <span class="metric-value">{total_edges}</span>
                                </div>
                                <div class="metric-card">
                                    <span class="metric-label">"Peso M\u{00e9}dio"</span>
                                    <span class="metric-value muted">{avg_weight}</span>
                                </div>
                            </div>
                            <div class="topology-controls">
                                <button on:click=on_reset_click title="Resetar layout para padrão">
                                    "\u{21bb} Resetar Layout"
                                </button>
                            </div>
                            <svg
                                id="topology-svg"
                                class="topology-svg"
                                viewBox="0 0 800 500"
                                xmlns="http://www.w3.org/2000/svg"
                                on:mousemove=on_svg_mousemove
                                on:mouseup=on_svg_mouseup
                                on:wheel=on_svg_wheel
                                style="touch-action: none;"
                            >
                                <g transform=transform_str>
                                    {edge_views}
                                    {node_views}
                                </g>
                            </svg>
                            {tooltip_view}
                        </>
                    }.into_any()
                } else {
                    view! { <div class="topology-empty">"Sem dados de topologia"</div> }.into_any()
                }
            }}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_layout_produces_unique_positions() {
        let mut nodes = vec![
            TopologyNode {
                id: "a".to_string(),
                label: "A".to_string(),
                node_type: NodeType::Agent,
                status: NodeStatus::Online,
                x: 0.0,
                y: 0.0,
            },
            TopologyNode {
                id: "b".to_string(),
                label: "B".to_string(),
                node_type: NodeType::Orchestrator,
                status: NodeStatus::Online,
                x: 0.0,
                y: 0.0,
            },
            TopologyNode {
                id: "c".to_string(),
                label: "C".to_string(),
                node_type: NodeType::Gateway,
                status: NodeStatus::Online,
                x: 0.0,
                y: 0.0,
            },
        ];
        compute_layout(&mut nodes);
        for i in 0..nodes.len() {
            for j in (i + 1)..nodes.len() {
                let dx = (nodes[i].x - nodes[j].x).abs();
                let dy = (nodes[i].y - nodes[j].y).abs();
                assert!(dx > 0.01 || dy > 0.01, "Nodes {} and {} overlap", i, j);
            }
        }
    }

    #[test]
    fn test_find_node_pos_returns_none_for_missing() {
        let nodes = vec![TopologyNode {
            id: "x".to_string(),
            label: "X".to_string(),
            node_type: NodeType::Agent,
            status: NodeStatus::Online,
            x: 10.0,
            y: 20.0,
        }];
        assert!(find_node_pos(&nodes, "missing").is_none());
        assert_eq!(find_node_pos(&nodes, "x"), Some((10.0, 20.0)));
    }

    #[test]
    fn test_node_type_css_classes() {
        assert_eq!(NodeType::Agent.css_class(), "node-agent");
        assert_eq!(NodeType::Orchestrator.css_class(), "node-orchestrator");
        assert_eq!(NodeType::Gateway.css_class(), "node-gateway");
    }

    #[test]
    fn test_edge_dash_arrays() {
        assert_eq!(EdgeType::Control.dash_array(), "none");
        assert_eq!(EdgeType::Data.dash_array(), "8,4");
        assert_eq!(EdgeType::Heartbeat.dash_array(), "3,3");
    }

    #[test]
    fn test_edge_is_animated() {
        assert!(!EdgeType::Control.is_animated());
        assert!(EdgeType::Data.is_animated());
        assert!(EdgeType::Heartbeat.is_animated());
    }

    #[test]
    fn test_node_status_labels() {
        assert_eq!(NodeStatus::Online.label(), "Online");
        assert_eq!(NodeStatus::Degraded.label(), "Degradado");
        assert_eq!(NodeStatus::Offline.label(), "Offline");
    }

    #[test]
    fn test_node_type_descriptions() {
        assert_eq!(NodeType::Agent.description(), "Agente executor de tarefas");
        assert_eq!(NodeType::Orchestrator.description(), "Orquestrador de workflow");
        assert_eq!(NodeType::Gateway.description(), "Gateway de comunicação");
    }

    #[test]
    fn test_svg_transform_default() {
        let t = SvgTransform::default();
        assert_eq!(t.scale, 1.0);
        assert_eq!(t.translate_x, 0.0);
        assert_eq!(t.translate_y, 0.0);
    }

    #[test]
    fn test_drag_state_default() {
        let d = DragState::default();
        assert!(!d.active);
        assert!(d.node_id.is_none());
    }

    #[test]
    fn test_tooltip_state_default() {
        let t = TooltipState::default();
        assert!(!t.visible);
        assert!(t.node_id.is_none());
    }
}