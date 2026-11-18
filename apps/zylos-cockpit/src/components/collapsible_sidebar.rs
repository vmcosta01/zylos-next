//! CollapsibleSidebar — Sidebar agrupada e colapsável (DESIGN_SYSTEM.md §10.4).
//!
//! Grupos: Core, Orquestração, Comunicação, Configuração.
//! Persistência: localStorage `zylos-sidebar-collapsed` (sidebar inteira)
//! e `zylos-sidebar-groups` (grupos individuais).
//! Animação: CSS transition via var(--spring-ui-response) (spring real na Fase 3).

use leptos::prelude::*;

use crate::tabs::Tab;

/// Grupos da sidebar conforme DESIGN_SYSTEM.md §10.4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SidebarGroup {
    Core,
    Orquestracao,
    Comunicacao,
    Configuracao,
}

impl SidebarGroup {
    pub const ALL: [SidebarGroup; 4] = [
        SidebarGroup::Core,
        SidebarGroup::Orquestracao,
        SidebarGroup::Comunicacao,
        SidebarGroup::Configuracao,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Core => "CORE",
            Self::Orquestracao => "ORQUESTRAÇÃO",
            Self::Comunicacao => "COMUNICAÇÃO",
            Self::Configuracao => "CONFIGURAÇÃO",
        }
    }

    pub fn tabs(self) -> &'static [Tab] {
        match self {
            Self::Core => &[Tab::Agents, Tab::Tasks, Tab::Memory, Tab::Goals],
            Self::Orquestracao => &[Tab::SwarmDag, Tab::Sentinel, Tab::Soar],
            Self::Comunicacao => &[Tab::Channels, Tab::PhoneRelay, Tab::Chat, Tab::Threads],
            Self::Configuracao => &[Tab::Personalities, Tab::Skins, Tab::Integrations],
        }
    }

    pub fn storage_key(self) -> &'static str {
        match self {
            Self::Core => "core",
            Self::Orquestracao => "orquestracao",
            Self::Comunicacao => "comunicacao",
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
    // Estado global: sidebar colapsada (mostra só ícones).
    let collapsed = RwSignal::new(ls_get_bool("zylos-sidebar-collapsed", false));

    // Estado por grupo: true = expandido, false = colapsado.
    let group_states = StoredValue::new({
        let mut map = std::collections::HashMap::new();
        for g in SidebarGroup::ALL {
            let key = format!("zylos-sidebar-group-{}", g.storage_key());
            map.insert(g, ls_get_bool(&key, true));
        }
        map
    });

    let toggle_sidebar = move |_| {
        let new_val = !collapsed.get();
        collapsed.set(new_val);
        ls_set_bool("zylos-sidebar-collapsed", new_val);
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
            class=move || {
                if collapsed.get() {
                    "collapsible-sidebar sidebar-collapsed material-translucent"
                } else {
                    "collapsible-sidebar sidebar-expanded material-translucent"
                }
            }
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
                                        let tab_label = StoredValue::new(tab.i18n_key());
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
                                                            tab_label.with_value(|k| k.to_string())
                                                        } else {
                                                            String::new()
                                                        }
                                                    }
                                                >
                                                    <span class="item-icon">{tab_icon(tab)}</span>
                                                    <span class="item-label">{tab_label.with_value(|k| k.to_string())}</span>
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

/// Ícone simples por aba (placeholder até Fase 7 com ícones SVG reais).
fn tab_icon(tab: Tab) -> &'static str {
    match tab {
        Tab::Agents => "👤",
        Tab::Tasks => "✓",
        Tab::Memory => "🧠",
        Tab::Goals => "🎯",
        Tab::SwarmDag => "⚡",
        Tab::Sentinel => "🛡",
        Tab::Soar => "🔄",
        Tab::Channels => "📢",
        Tab::PhoneRelay => "📱",
        Tab::Chat => "💬",
        Tab::Threads => "🧵",
        Tab::Personalities => "🎭",
        Tab::Skins => "🎨",
        Tab::Integrations => "⚙️",
        _ => "•",
    }
}
