//! CollapsibleSidebar — Sidebar agrupada e colapsável (DESIGN_SYSTEM.md §10.4).
//!
//! Grupos: Core, Orquestração, Comunicação, Configuração.
//! Persistência: localStorage `zylos-sidebar-collapsed` (sidebar inteira)
//! e `zylos-sidebar-groups` (grupos individuais).
//! Animação: Spring physics-based via `crate::spring` (ADR-007, Fase 3).

use leptos::prelude::*;

use crate::i18n::use_i18n;
use crate::spring::{create_spring, SpringConfig};
use crate::tabs::Tab;

/// Grupos da sidebar conforme DESIGN_SYSTEM.md §10.4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SidebarGroup {
    Core,
    Orquestracao,
    Seguranca,
    Comunicacao,
    Ferramentas,
    Configuracao,
}

impl SidebarGroup {
    pub const ALL: [SidebarGroup; 6] = [
        SidebarGroup::Core,
        SidebarGroup::Orquestracao,
        SidebarGroup::Seguranca,
        SidebarGroup::Comunicacao,
        SidebarGroup::Ferramentas,
        SidebarGroup::Configuracao,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Core => "CORE",
            Self::Orquestracao => "ORQUESTRAÇÃO",
            Self::Seguranca => "SEGURANÇA",
            Self::Comunicacao => "COMUNICAÇÃO",
            Self::Ferramentas => "FERRAMENTAS",
            Self::Configuracao => "CONFIGURAÇÃO",
        }
    }

    pub fn tabs(self) -> &'static [Tab] {
        match self {
            Self::Core => &[Tab::Agents, Tab::Tasks, Tab::Memory, Tab::Goals],
            Self::Orquestracao => &[Tab::SwarmDag, Tab::SwarmTopology, Tab::Sentinel, Tab::Soar, Tab::Subagents, Tab::TaskTriage],
            Self::Seguranca => &[Tab::Security, Tab::HitlApproval, Tab::EvidenceLedger, Tab::Awareness, Tab::EmployeeGovernance],
            Self::Comunicacao => &[Tab::Channels, Tab::PhoneRelay, Tab::Chat, Tab::Threads, Tab::Reactions],
            Self::Ferramentas => &[Tab::Skills, Tab::SkillEditor, Tab::SkillEvolution, Tab::Files, Tab::Analytics, Tab::Collab, Tab::Logs],
            Self::Configuracao => &[Tab::Personalities, Tab::Avatars, Tab::Skins, Tab::Fleet, Tab::Federation, Tab::Voice, Tab::Integrations],
        }
    }

    pub fn storage_key(self) -> &'static str {
        match self {
            Self::Core => "core",
            Self::Orquestracao => "orquestracao",
            Self::Seguranca => "seguranca",
            Self::Comunicacao => "comunicacao",
            Self::Ferramentas => "ferramentas",
            Self::Configuracao => "configuracao",
        }
    }
}

/// Lê um valor booleano do localStorage. Retorna `default` se ausente ou erro.
fn ls_get_bool(key: &str, default: bool) -> bool {
    web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item(key).ok().flatten())
        .map(|v| v == "true")
        .unwrap_or(default)
}

/// Escreve um valor booleano no localStorage.
fn ls_set_bool(key: &str, value: bool) {
    if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        let _ = storage.set_item(key, if value { "true" } else { "false" });
    }
}

/// Sidebar colapsável com grupos de navegação.
#[component]
pub fn CollapsibleSidebar(
    active_tab: ReadSignal<Tab>,
    set_active_tab: WriteSignal<Tab>,
) -> impl IntoView {
    let i18n = use_i18n();

    // Estado global: sidebar colapsada (mostra só ícones).
    let collapsed = RwSignal::new(ls_get_bool("zylos-sidebar-collapsed", false));

    // Spring para animar width da sidebar (ADR-007, DESIGN_SYSTEM.md §8).
    let initial_width = if collapsed.get() { 64.0 } else { 280.0 };
    let (sidebar_width, width_spring) = create_spring(initial_width, SpringConfig::UI);

    // Estado por grupo: true = expandido, false = colapsado.
    let group_states = StoredValue::new({
        let mut map = std::collections::HashMap::new();
        for g in SidebarGroup::ALL {
            let key = format!("zylos-sidebar-group-{}", g.storage_key());
            map.insert(g, ls_get_bool(&key, true));
        }
        map
    });

    let toggle_sidebar = {
        let width_spring = width_spring.clone();
        move |_| {
            let new_val = !collapsed.get();
            collapsed.set(new_val);
            ls_set_bool("zylos-sidebar-collapsed", new_val);
            // Anima width via spring (interruptível, velocity handoff).
            width_spring.set_target(if new_val { 64.0 } else { 280.0 });
        }
    };

    let toggle_group = move |g: SidebarGroup| {
        let current = group_states.with_value(|m| m.get(&g).copied().unwrap_or(true));
        let new_val = !current;
        group_states.update_value(|m| {
            m.insert(g, new_val);
        });
        let key = format!("zylos-sidebar-group-{}", g.storage_key());
        ls_set_bool(&key, new_val);
    };

    view! {
        <nav
            class="collapsible-sidebar material-translucent"
            style=move || format!("width: {:.1}px; min-width: {:.1}px;", sidebar_width.get(), sidebar_width.get())
            role="navigation"
            aria-label="Navegação principal"
        >
            <div class="sidebar-toggle">
                <button
                    class="sidebar-toggle-btn"
                    on:click=toggle_sidebar
                    aria-label=move || if collapsed.get() { "Expandir sidebar" } else { "Recolher sidebar" }
                    title=move || if collapsed.get() { "Expandir" } else { "Recolher" }
                >
                    {move || if collapsed.get() { "☰" } else { "◀" }}
                </button>
            </div>

            <div class="sidebar-groups">
                {SidebarGroup::ALL.iter().map(|&group| {
                    let i18n = i18n;
                    let tabs = group.tabs();
                    let is_expanded = Signal::derive(move || {
                        group_states.with_value(|m| m.get(&group).copied().unwrap_or(true))
                    });

                    view! {
                        <div class="sidebar-group">
                            <button
                                class="sidebar-group-header"
                                on:click=move |_| toggle_group(group)
                                aria-expanded=move || is_expanded.get()
                            >
                                <span class="group-label">{group.label()}</span>
                                <span class=move || {
                                    if is_expanded.get() { "group-chevron expanded" } else { "group-chevron" }
                                }>"▾"</span>
                            </button>
                            <Show when=move || is_expanded.get() || collapsed.get()>
                                <ul class="sidebar-items" role="list">
                                    {tabs.iter().map(|&tab| {
                                        let is_active = Signal::derive(move || active_tab.get() == tab);
                                        view! {
                                            <li>
                                                <button
                                                    class=move || {
                                                        if is_active.get() {
                                                            format!("sidebar-item active {}", tab.css_class())
                                                        } else {
                                                            format!("sidebar-item {}", tab.css_class())
                                                        }
                                                    }
                                                    on:click=move |_| set_active_tab.set(tab)
                                                    aria-current=move || is_active.get().then_some("page")
                                                    title=move || {
                                                        if collapsed.get() {
                                                            i18n.t(tab.i18n_key())
                                                        } else {
                                                            String::new()
                                                        }
                                                    }
                                                >
                                                    <span class="item-icon"><i class=format!("ph ph-{}", tab_icon(tab)) aria-hidden="true"></i></span>
                                                    <span class="item-label">{move || i18n.t(tab.i18n_key())}</span>
                                                </button>
                                            </li>
                                        }
                                    }).collect_view()}
                                </ul>
                            </Show>
                        </div>
                    }
                }).collect_view()}
            </div>
        </nav>
    }
}

/// Ícone Phosphor por aba (nome sem o prefixo `ph-`). Novos nomes entram em
/// `scripts/build-icons.py`, que gera o subconjunto da fonte servido em `fonts/`.
fn tab_icon(tab: Tab) -> &'static str {
    match tab {
        Tab::Agents => "robot",
        Tab::Tasks => "check-square",
        Tab::Memory => "brain",
        Tab::Goals => "target",
        Tab::Personalities => "user-circle",
        Tab::SwarmDag => "graph",
        Tab::SwarmTopology => "share-network",
        Tab::Sentinel => "shield-check",
        Tab::Soar => "lightning",
        Tab::Subagents => "tree-structure",
        Tab::TaskTriage => "funnel",
        Tab::Security => "lock-key",
        Tab::HitlApproval => "hand-palm",
        Tab::EvidenceLedger => "books",
        Tab::Awareness => "eye",
        Tab::EmployeeGovernance => "scales",
        Tab::Channels => "megaphone",
        Tab::PhoneRelay => "device-mobile",
        Tab::Chat => "chats-circle",
        Tab::Threads => "chat-teardrop-text",
        Tab::Reactions => "smiley",
        Tab::Skills => "toolbox",
        Tab::SkillEditor => "pencil-simple",
        Tab::SkillEvolution => "git-branch",
        Tab::Files => "folder-simple",
        Tab::Analytics => "chart-line-up",
        Tab::Collab => "users-three",
        Tab::Logs => "scroll",
        Tab::Voice => "microphone",
        Tab::Avatars => "user-focus",
        Tab::Skins => "palette",
        Tab::Fleet => "buildings",
        Tab::Federation => "globe-hemisphere-west",
        Tab::Integrations => "plugs-connected",
    }
}
