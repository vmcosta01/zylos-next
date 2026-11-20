//! FederationDashboard — painel de federação com HLC, VectorClock e resolução de conflitos
//!
//! Inspirado em: Ruflo (federation/HLC), Paperclip (conflict resolution).
//! Visualiza estado de sincronização entre nós federados, relógios híbridos lógicos (HLC),
//! vetores de clock e conflitos pendentes/resolvidos.
//!
//! ## Related Documents
//! - Implementation Plan: [IMPLEMENTATION_PLAN_DETAILED.md](../../docs/IMPLEMENTATION_PLAN_DETAILED.md) §5.1 Task 4.8

use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::{Deserialize, Serialize};

/// Estado de um nó federado
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeStatus {
    Synced,
    Syncing,
    Drifted,
    Offline,
    Conflict,
}

impl NodeStatus {
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Synced => "✅",
            Self::Syncing => "🔄",
            Self::Drifted => "⏱️",
            Self::Offline => "❌",
            Self::Conflict => "⚠️",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Synced => "Sincronizado",
            Self::Syncing => "Sincronizando",
            Self::Drifted => "Desviado",
            Self::Offline => "Offline",
            Self::Conflict => "Conflito",
        }
    }

    pub fn css_class(&self) -> &'static str {
        match self {
            Self::Synced => "node-synced",
            Self::Syncing => "node-syncing",
            Self::Drifted => "node-drifted",
            Self::Offline => "node-offline",
            Self::Conflict => "node-conflict",
        }
    }
}

/// Um nó na federação
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FederatedNode {
    pub id: String,
    pub name: String,
    pub region: String,
    pub status: NodeStatus,
    pub hlc_wall_ms: u64,
    pub hlc_logical: u64,
    pub vector_clock: Vec<u64>,
    pub last_sync_at: String,
    pub latency_ms: u64,
    pub pending_conflicts: usize,
}

/// Evento HLC na timeline
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HlcEvent {
    pub id: String,
    pub node_id: String,
    pub wall_ms: u64,
    pub logical: u64,
    pub event_type: String,
    pub description: String,
}

/// Conflito detectado entre nós
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FederationConflict {
    pub id: String,
    pub nodes_involved: Vec<String>,
    pub resource: String,
    pub detected_at: String,
    pub resolved: bool,
    pub resolution_strategy: Option<String>,
    pub description: String,
}

/// Dados completos do dashboard de federação
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationData {
    pub nodes: Vec<FederatedNode>,
    pub recent_events: Vec<HlcEvent>,
    pub conflicts: Vec<FederationConflict>,
    pub total_syncs_24h: u64,
    pub avg_latency_ms: u64,
    pub max_drift_ms: u64,
}

#[component]
pub fn FederationDashboard() -> impl IntoView {
    let data = RwSignal::new(None::<FederationData>);
    let loading = RwSignal::new(true);
    let expanded_conflict = RwSignal::new(None::<String>);

    Effect::new(move |_| {
        spawn_local(async move {
            loading.set(true);
            if let Ok(d) = fetch_federation_data().await {
                data.set(Some(d));
            }
            loading.set(false);
        });
    });

    view! {
        <div class="federation-dashboard">
            <div class="fed-header">
                <h3 class="fed-title"><i class="ph ph-globe-hemisphere-west" aria-hidden="true"></i>" Federation Dashboard"</h3>
                <span class="fed-subtitle">"HLC · VectorClock · Conflict Resolution"</span>
            </div>

            {move || {
                if loading.get() {
                    view! {
                        <div class="fed-loading">
                            <span class="spinner"></span>
                            <span>"Carregando dados de federação..."</span>
                        </div>
                    }.into_any()
                } else if let Some(ref d) = data.get() {
                    let nodes = d.nodes.clone();
                    let events = d.recent_events.clone();
                    let conflicts = d.conflicts.clone();
                    let total_syncs = d.total_syncs_24h;
                    let avg_lat = d.avg_latency_ms;
                    let max_drift = d.max_drift_ms;
                    let exp_conflict = expanded_conflict;

                    view! {
                        // Summary metrics
                        <div class="fed-summary">
                            <div class="fed-metric-card">
                                <span class="fed-metric-label">"Nós Ativos"</span>
                                <span class="fed-metric-value">{nodes.len()}</span>
                            </div>
                            <div class="fed-metric-card">
                                <span class="fed-metric-label">"Syncs (24h)"</span>
                                <span class="fed-metric-value">{total_syncs}</span>
                            </div>
                            <div class="fed-metric-card">
                                <span class="fed-metric-label">"Latência Média"</span>
                                <span class="fed-metric-value">{format!("{}ms", avg_lat)}</span>
                            </div>
                            <div class="fed-metric-card">
                                <span class="fed-metric-label">"Max Drift"</span>
                                <span class="fed-metric-value">{format!("{}ms", max_drift)}</span>
                            </div>
                            <div class="fed-metric-card">
                                <span class="fed-metric-label">"Conflitos"</span>
                                <span class="fed-metric-value fed-conflict-count">{conflicts.len()}</span>
                            </div>
                        </div>

                        // Nodes grid
                        <div class="fed-section">
                            <h4 class="fed-section-title"><i class="ph ph-broadcast" aria-hidden="true"></i>" Nós Federados"</h4>
                            <div class="fed-nodes-grid">
                                <For
                                    each=move || nodes.clone()
                                    key=|n| n.id.clone()
                                    children=move |node| {
                                        let nname = node.name.clone();
                                        let nregion = node.region.clone();
                                        let nstatus = node.status.clone();
                                        let nhlc_w = node.hlc_wall_ms;
                                        let nhlc_l = node.hlc_logical;
                                        let nlat = node.latency_ms;
                                        let npending = node.pending_conflicts;
                                        let nvc = node.vector_clock.clone();
                                        let css = format!("fed-node-card {}", nstatus.css_class());
                                        let icon = nstatus.icon().to_string();
                                        let label = nstatus.label().to_string();
                                        let vc_str = nvc.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(",");
                                        let hlc_display = format!("{}.{:06}", nhlc_w, nhlc_l);

                                        view! {
                                            <div class=css>
                                                <div class="fed-node-header">
                                                    <span class="fed-node-icon">{icon.clone()}</span>
                                                    <span class="fed-node-name">{nname.clone()}</span>
                                                    <span class="fed-node-region">{nregion.clone()}</span>
                                                </div>
                                                <div class="fed-node-status">{label.clone()}</div>
                                                <div class="fed-node-hlc">
                                                    <span class="hlc-label">"HLC: "</span>
                                                    <span class="hlc-value">{hlc_display.clone()}</span>
                                                </div>
                                                <div class="fed-node-vc">
                                                    <span class="vc-label">"VC: ["</span>
                                                    <span class="vc-value">{vc_str.clone()}</span>
                                                    <span class="vc-label">"]"</span>
                                                </div>
                                                <div class="fed-node-meta">
                                                    <span>{format!("Latência: {}ms", nlat)}</span>
                                                    {if npending > 0 {
                                                        view! { <span class="fed-pending-badge">{format!("{} conflitos", npending)}</span> }.into_any()
                                                    } else {
                                                        view! { <div></div> }.into_any()
                                                    }}
                                                </div>
                                            </div>
                                        }
                                    }
                                />
                            </div>
                        </div>

                        // HLC Timeline
                        <div class="fed-section">
                            <h4 class="fed-section-title">"⏰ HLC Timeline (últimos eventos)"</h4>
                            <div class="fed-hlc-timeline">
                                <For
                                    each=move || events.clone()
                                    key=|e| e.id.clone()
                                    children=move |event| {
                                        let enode = event.node_id.clone();
                                        let etype = event.event_type.clone();
                                        let edesc = event.description.clone();
                                        let hlc_time = format!("{}.{:06}", event.wall_ms, event.logical);

                                        view! {
                                            <div class="fed-hlc-event">
                                                <div class="hlc-event-marker"></div>
                                                <div class="hlc-event-content">
                                                    <span class="hlc-event-node">{enode}</span>
                                                    <span class="hlc-event-type">{etype}</span>
                                                    <span class="hlc-event-time">{hlc_time}</span>
                                                    <p class="hlc-event-desc">{edesc}</p>
                                                </div>
                                            </div>
                                        }
                                    }
                                />
                            </div>
                        </div>

                        // Conflicts
                        <div class="fed-section">
                            <h4 class="fed-section-title"><i class="ph ph-sword" aria-hidden="true"></i>" Conflitos"</h4>
                            {if conflicts.is_empty() {
                                view! {
                                    <div class="fed-no-conflicts">"Nenhum conflito pendente"</div>
                                }.into_any()
                            } else {
                                view! {
                                    <div class="fed-conflicts-list">
                                        <For
                                            each=move || conflicts.clone()
                                            key=|c| c.id.clone()
                                            children=move |conflict| {
                                                let cid = conflict.id.clone();
                                                let cid_for_click = cid.clone();
                                                let cresource = conflict.resource.clone();
                                                let cdetected = conflict.detected_at.clone();
                                                let cresolved = conflict.resolved;
                                                let cdesc = conflict.description.clone();
                                                let cnodes = conflict.nodes_involved.clone();
                                                let cstrategy = conflict.resolution_strategy.clone();
                                                let exp = exp_conflict;
                                                let is_expanded = move || exp.get().as_deref() == Some(&cid);

                                                let card_css = if cresolved {
                                                    "fed-conflict-card fed-conflict-resolved".to_string()
                                                } else {
                                                    "fed-conflict-card fed-conflict-pending".to_string()
                                                };

                                                view! {
                                                    <div
                                                        class=card_css
                                                        on:click=move |_| {
                                                            let id = cid_for_click.clone();
                                                            exp.update(|current| {
                                                                if current.as_deref() == Some(&id) {
                                                                    *current = None;
                                                                } else {
                                                                    *current = Some(id);
                                                                }
                                                            });
                                                        }
                                                    >
                                                        <div class="fed-conflict-header">
                                                            <span class="fed-conflict-resource">{cresource.clone()}</span>
                                                            <span class="fed-conflict-status">
                                                                {if cresolved { "Resolvido" } else { "Pendente" }}
                                                            </span>
                                                        </div>
                                                        <div class="fed-conflict-nodes">
                                                            <For
                                                                each=move || cnodes.clone()
                                                                key=|n| n.clone()
                                                                children=move |n| {
                                                                    view! { <span class="fed-conflict-node-tag">{n.clone()}</span> }
                                                                }
                                                            />
                                                        </div>
                                                        <p class="fed-conflict-desc">{cdesc.clone()}</p>
                                                        {move || {
                                                            if is_expanded() {
                                                                view! {
                                                                    <div class="fed-conflict-details">
                                                                        <span class="fed-detail-label">"Detectado: "</span>
                                                                        <span>{cdetected.clone()}</span>
                                                                        {cstrategy.clone().map(|s| view! {
                                                                            <div>
                                                                                <span class="fed-detail-label">"Estratégia: "</span>
                                                                                <span>{s.clone()}</span>
                                                                            </div>
                                                                        })}
                                                                    </div>
                                                                }.into_any()
                                                            } else {
                                                                view! { <div></div> }.into_any()
                                                            }
                                                        }}
                                                    </div>
                                                }
                                            }
                                        />
                                    </div>
                                }.into_any()
                            }}
                        </div>
                    }.into_any()
                } else {
                    view! {
                        <div class="fed-empty">"Sem dados: a federação ainda não está ligada ao gateway."</div>
                    }.into_any()
                }
            }}
        </div>
    }
}

// === API Function ===

async fn fetch_federation_data() -> Result<FederationData, String> {
    // O gateway ainda não expõe federação/HLC. Antes isto devolvia nós, eventos e conflitos de
    // exemplo que pareciam reais; agora falha e a tela diz a verdade.
    // ATALHO: sem backend de federação; gatilho de upgrade: existir `GET /api/v1/federation`.
    Err("federação indisponível no gateway".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_status_variants() {
        let statuses = vec![
            NodeStatus::Synced,
            NodeStatus::Syncing,
            NodeStatus::Drifted,
            NodeStatus::Offline,
            NodeStatus::Conflict,
        ];
        for s in &statuses {
            assert!(!s.icon().is_empty());
            assert!(!s.label().is_empty());
            assert!(!s.css_class().is_empty());
        }
    }

    #[test]
    fn test_federated_node_serialization() {
        let node = FederatedNode {
            id: "test-node".into(),
            name: "Test Node".into(),
            region: "us-east-1".into(),
            status: NodeStatus::Synced,
            hlc_wall_ms: 1731340800000,
            hlc_logical: 42,
            vector_clock: vec![100, 98, 95],
            last_sync_at: "2026-11-11T15:00:00Z".into(),
            latency_ms: 12,
            pending_conflicts: 0,
        };
        let json = serde_json::to_string(&node).unwrap();
        assert!(json.contains("test-node"));
        assert!(json.contains("synced"));
    }

    #[test]
    fn test_conflict_serialization() {
        let conflict = FederationConflict {
            id: "c1".into(),
            nodes_involved: vec!["n1".into(), "n2".into()],
            resource: "test/resource".into(),
            detected_at: "2026-11-11T10:00:00Z".into(),
            resolved: false,
            resolution_strategy: None,
            description: "Test conflict".into(),
        };
        let json = serde_json::to_string(&conflict).unwrap();
        assert!(json.contains("c1"));
        assert!(json.contains("test/resource"));
    }

    #[test]
    fn test_hlc_event_serialization() {
        let event = HlcEvent {
            id: "e1".into(),
            node_id: "node-1".into(),
            wall_ms: 1731340800000,
            logical: 42,
            event_type: "sync_complete".into(),
            description: "Sync done".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("sync_complete"));
        assert!(json.contains("42"));
    }
}
