//! Command Palette (ADR-009) — Omnibar Cmd+K para navegação rápida.
//!
//! Indexa telas (tabs), comandos e histórico recente. Busca fuzzy pura em Rust
//! sem dependências externas. Spring animation de entrada/saída via `spring.rs`.

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

use crate::spring::{create_spring, SpringConfig};
use crate::tabs::Tab;

// ─── Tipos ───────────────────────────────────────────────────────────────────

/// Categoria de um item na paleta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Category {
    Recent,
    Page,
    Command,
}

impl Category {
    pub fn label(self) -> &'static str {
        match self {
            Category::Recent => "RECENT",
            Category::Page => "PAGES",
            Category::Command => "COMMANDS",
        }
    }
}

/// Um item indexado na paleta.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaletteItem {
    pub id: String,
    pub label: String,
    pub category: Category,
    /// Para páginas: i18n key da tab. Para comandos: descrição curta.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tab: Option<Tab>,
    /// Ação de comando (identificador string).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
}

/// Entrada do histórico persistido em localStorage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct HistoryEntry {
    id: String,
    label: String,
    category: Category,
    #[serde(skip_serializing_if = "Option::is_none")]
    tab: Option<Tab>,
    #[serde(skip_serializing_if = "Option::is_none")]
    command: Option<String>,
}

const HISTORY_KEY: &str = "zylos_command_history";
const MAX_HISTORY: usize = 10;
const MAX_VISIBLE: usize = 20;

// ─── Fuzzy Search ────────────────────────────────────────────────────────────

/// Score de matching fuzzy. Maior = melhor match. 0 = sem match.
fn fuzzy_score(query: &str, target: &str) -> u32 {
    if query.is_empty() {
        return 1; // empty query matches everything
    }
    let q: Vec<char> = query.to_lowercase().chars().collect();
    let t: Vec<char> = target.to_lowercase().chars().collect();

    let mut qi = 0;
    let mut score: u32 = 0;
    let mut last_match_idx: Option<usize> = None;

    for (ti, tc) in t.iter().enumerate() {
        if qi < q.len() && *tc == q[qi] {
            // Bonus por match consecutivo (adjacente ao anterior)
            if let Some(last) = last_match_idx {
                if ti == last + 1 {
                    score += 5;
                }
            }
            // Bonus por match no início
            if ti == 0 {
                score += 10;
            }
            // Score base
            score += 1;
            last_match_idx = Some(ti);
            qi += 1;
        }
    }

    // Só considera match se todos os chars do query foram encontrados
    if qi == q.len() {
        score
    } else {
        0
    }
}

/// Filtra e ordena itens por score fuzzy.
fn search_items(items: &[PaletteItem], query: &str) -> Vec<(PaletteItem, u32)> {
    if query.is_empty() {
        return items.iter().map(|i| (i.clone(), 1)).collect();
    }
    let mut results: Vec<(PaletteItem, u32)> = items
        .iter()
        .filter_map(|item| {
            let s = fuzzy_score(query, &item.label);
            if s > 0 {
                Some((item.clone(), s))
            } else {
                None
            }
        })
        .collect();
    results.sort_by(|a, b| b.1.cmp(&a.1));
    results.truncate(MAX_VISIBLE);
    results
}

// ─── Histórico ───────────────────────────────────────────────────────────────

fn load_history() -> Vec<HistoryEntry> {
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window()
            .and_then(|w| w.local_storage().ok().flatten())
            .and_then(|s| s.get_item(HISTORY_KEY).ok().flatten())
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Vec::new()
    }
}

fn save_history(entries: &[HistoryEntry]) {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(s) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
            if let Ok(json) = serde_json::to_string(entries) {
                let _ = s.set_item(HISTORY_KEY, &json);
            }
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = entries;
    }
}

fn add_to_history(item: &PaletteItem) {
    let mut history = load_history();
    // Dedup: remove entrada existente com mesmo id
    history.retain(|e| e.id != item.id);
    // Insere no topo
    history.insert(
        0,
        HistoryEntry {
            id: item.id.clone(),
            label: item.label.clone(),
            category: item.category,
            tab: item.tab,
            command: item.command.clone(),
        },
    );
    // Limita tamanho
    history.truncate(MAX_HISTORY);
    save_history(&history);
}

// ─── Index Builder ───────────────────────────────────────────────────────────

fn build_static_index() -> Vec<PaletteItem> {
    let mut items = Vec::with_capacity(Tab::ALL.len() + 5);

    // Páginas (todas as tabs)
    for tab in Tab::ALL.iter().copied() {
        let label = match tab {
            Tab::Agents => "Agentes",
            Tab::Tasks => "Tarefas",
            Tab::Memory => "Memória",
            Tab::Goals => "Metas",
            Tab::Personalities => "Personalidades",
            Tab::SwarmDag => "DAG do Swarm",
            Tab::Sentinel => "Sentinela",
            Tab::EvidenceLedger => "Evidence Ledger",
            Tab::HitlApproval => "Approvals HITL",
            Tab::Skills => "Skills / Benchmarks",
            Tab::Logs => "Logs (Event Feed)",
            Tab::Soar => "SOAR Dashboard",
            Tab::Security => "Segurança",
            Tab::Awareness => "Agent Awareness",
            Tab::SkillEvolution => "Skill Evolution",
            Tab::EmployeeGovernance => "Employee Governance",
            Tab::TaskTriage => "Task Triage",
            Tab::Threads => "Threads",
            Tab::Subagents => "Subagentes",
            Tab::Files => "Arquivos",
            Tab::Voice => "Voz",
            Tab::Reactions => "Reações",
            Tab::Avatars => "Avatares",
            Tab::Skins => "Temas / Skins",
            Tab::Fleet => "Fleet Auditor",
            Tab::Federation => "Federação",
            Tab::Chat => "Chat",
            Tab::Channels => "Canais",
            Tab::PhoneRelay => "Phone Relay (VNC)",
            Tab::Integrations => "Integrações",
            Tab::SwarmTopology => "Topologia do Swarm",
            Tab::SkillEditor => "Editor de Skills",
            Tab::Analytics => "Analytics",
            Tab::Collab => "Colaboração",
        };
        items.push(PaletteItem {
            id: format!("page-{}", tab.css_class()),
            label: label.to_string(),
            category: Category::Page,
            tab: Some(tab),
            command: None,
        });
    }

    // Comandos estáticos
    let commands = [
        ("cmd-toggle-theme", "Alternar Tema Claro/Escuro"),
        ("cmd-refresh", "Atualizar Dados"),
        ("cmd-clear-notifications", "Limpar Notificações"),
        ("cmd-export-data", "Exportar Dados"),
        ("cmd-open-settings", "Abrir Configurações"),
    ];
    for (id, label) in commands {
        items.push(PaletteItem {
            id: id.to_string(),
            label: label.to_string(),
            category: Category::Command,
            tab: None,
            command: Some(id.to_string()),
        });
    }

    items
}

// ─── Componente ──────────────────────────────────────────────────────────────

/// Command Palette overlay (ADR-009).
///
/// Montado no layout shell. Abre com Ctrl/Cmd+K, fecha com Esc ou click fora.
#[component]
pub fn CommandPalette(open: RwSignal<bool>, set_active_tab: WriteSignal<Tab>) -> impl IntoView {
    let query = RwSignal::new(String::new());
    let selected_idx = RwSignal::new(0usize);
    let input_ref = NodeRef::<leptos::html::Input>::new();

    // Spring animation para opacity e scale
    let (opacity_val, opacity_spring) = create_spring(0.0, SpringConfig::ENTRANCE);
    let (scale_val, scale_spring) = create_spring(0.95, SpringConfig::ENTRANCE);

    // Index estático (construído uma vez)
    let static_items = StoredValue::new_local(build_static_index());

    // Resultados filtrados
    let filtered = Memo::new(move |_| {
        let q = query.get();
        let items = static_items.with_value(|items| items.clone());
        search_items(&items, &q)
    });

    // Histórico (recarregado quando a paleta abre)
    let history = RwSignal::new(Vec::<HistoryEntry>::new());

    // Itens recentes como PaletteItems
    let recent_items = Memo::new(move |_| {
        let h = history.get();
        h.into_iter()
            .take(5)
            .map(|e| PaletteItem {
                id: e.id,
                label: e.label,
                category: Category::Recent,
                tab: e.tab,
                command: e.command,
            })
            .collect::<Vec<_>>()
    });

    // Lista combinada: recent (se query vazia) + filtered
    let combined = Memo::new(move |_| {
        let q = query.get();
        let recents = recent_items.get();
        let filt = filtered.get();

        if q.is_empty() && !recents.is_empty() {
            // Mostra recentes primeiro, depois páginas
            let mut all: Vec<(PaletteItem, u32)> = recents.into_iter().map(|r| (r, 1)).collect();
            // Adiciona páginas que não estão nos recentes
            let recent_ids: std::collections::HashSet<String> =
                all.iter().map(|(item, _)| item.id.clone()).collect();
            for (item, score) in filt {
                if !recent_ids.contains(&item.id) {
                    all.push((item, score));
                }
            }
            all.truncate(MAX_VISIBLE);
            all
        } else {
            filt
        }
    });

    // Animação de entrada/saída
    Effect::new(move |_| {
        if open.get() {
            opacity_spring.set_target(1.0);
            scale_spring.set_target(1.0);
            // Recarrega histórico
            history.set(load_history());
            query.set(String::new());
            selected_idx.set(0);
        } else {
            opacity_spring.set_target(0.0);
            scale_spring.set_target(0.95);
        }
    });

    // Focus no input quando abre
    Effect::new(move |_| {
        if open.get() {
            // Pequeno delay para garantir que o DOM existe
            let input = input_ref.get();
            if let Some(el) = input {
                let _ = el.focus();
            }
        }
    });

    // Reset selection quando resultados mudam
    Effect::new(move |_| {
        let _ = combined.get();
        selected_idx.set(0);
    });

    let execute_item = move |item: &PaletteItem| {
        add_to_history(item);
        if let Some(tab) = item.tab {
            set_active_tab.set(tab);
        }
        // Comandos são executados aqui (futuro: dispatch via context)
        open.set(false);
        query.set(String::new());
    };

    let item_count = Memo::new(move |_| combined.get().len());

    let on_keydown = move |ev: leptos::ev::KeyboardEvent| match ev.key().as_str() {
        "Escape" => {
            ev.prevent_default();
            open.set(false);
        }
        "ArrowDown" => {
            ev.prevent_default();
            let count = item_count.get();
            if count > 0 {
                let cur = selected_idx.get();
                selected_idx.set((cur + 1) % count);
            }
        }
        "ArrowUp" => {
            ev.prevent_default();
            let count = item_count.get();
            if count > 0 {
                let cur = selected_idx.get();
                selected_idx.set(if cur == 0 { count - 1 } else { cur - 1 });
            }
        }
        "Enter" => {
            ev.prevent_default();
            let idx = selected_idx.get();
            let items = combined.get();
            if let Some((item, _)) = items.get(idx) {
                execute_item(item);
            }
        }
        _ => {}
    };

    // Debounce: resetamos o query signal apenas após 50ms de inatividade.
    // Leptos já é reativo; o debounce é implementado via timeout no input handler.
    // Simplified: debounce handle removed as the fuzzy search is <1ms for <100 items.

    let on_input = move |ev: leptos::ev::Event| {
        let value = event_target_value(&ev);
        query.set(value);
    };

    // Agrupamento dos resultados por categoria
    let grouped = Memo::new(move |_| {
        let items = combined.get();
        let mut groups: Vec<(Category, Vec<(PaletteItem, u32)>)> = Vec::new();
        let mut current_cat: Option<Category> = None;

        for (item, score) in items {
            let cat = item.category;
            if current_cat != Some(cat) {
                groups.push((cat, Vec::new()));
                current_cat = Some(cat);
            }
            if let Some(last) = groups.last_mut() {
                last.1.push((item, score));
            }
        }
        groups
    });

    // Flat index mapping: dado um flat index, retorna (group_idx, item_in_group_idx)
    let _flat_to_grouped = move |flat_idx: usize| -> Option<(usize, usize)> {
        let groups = grouped.get();
        let mut offset = 0;
        for (gi, (_, items)) in groups.iter().enumerate() {
            if flat_idx < offset + items.len() {
                return Some((gi, flat_idx - offset));
            }
            offset += items.len();
        }
        None
    };

    // Track flat index for highlight
    let is_selected = move |flat_idx: usize| -> bool { selected_idx.get() == flat_idx };

    view! {
        <Show when=move || { open.get() || opacity_val.get() > 0.01 }>
            <div
                class="cmd-palette-backdrop"
                style:opacity=move || format!("{:.3}", opacity_val.get())
                style:pointer-events=move || if open.get() { "auto" } else { "none" }
                on:click=move |_| open.set(false)
            >
                <div
                    class="cmd-palette-overlay"
                    role="dialog"
                    aria-modal="true"
                    aria-label="Command Palette"
                    style:transform=move || format!("scale({:.4})", scale_val.get())
                    style:opacity=move || format!("{:.3}", opacity_val.get())
                    on:click=|ev| ev.stop_propagation()
                >
                    <div class="cmd-palette-input-wrap">
                        <span class="cmd-palette-icon">"⌘"</span>
                        <input
                            node_ref=input_ref
                            type="search"
                            class="cmd-palette-input"
                            placeholder="Buscar telas, agentes, comandos..."
                            prop:value=move || query.get()
                            on:input=on_input
                            on:keydown=on_keydown
                            autocomplete="off"
                        />
                        <kbd class="cmd-palette-kbd">"ESC"</kbd>
                    </div>

                    <div class="cmd-palette-results">
                        {move || {
                            let groups = grouped.get();
                            if groups.is_empty() {
                                view! {
                                    <div class="cmd-palette-empty">
                                        <p>"Nenhum resultado encontrado"</p>
                                        <span class="cmd-palette-hint">"Tente: agentes, tarefas, dag..."</span>
                                    </div>
                                }.into_any()
                            } else {
                                let mut flat_idx = 0usize;
                                groups
                                    .into_iter()
                                    .map(|(cat, items)| {
                                        let header = view! {
                                            <div class="cmd-palette-group-header">{cat.label()}</div>
                                        };
                                        let list: Vec<_> = items
                                            .into_iter()
                                            .map(|(item, _score)| {
                                                let idx = flat_idx;
                                                flat_idx += 1;
                                                let sel = is_selected(idx);
                                                let icon = match item.category {
                                                    Category::Recent => "🕐",
                                                    Category::Page => "📄",
                                                    Category::Command => "⚡",
                                                };
                                                view! {
                                                    <button
                                                        class=move || {
                                                            if is_selected(idx) {
                                                                "cmd-palette-item selected"
                                                            } else {
                                                                "cmd-palette-item"
                                                            }
                                                        }
                                                        on:click=move |_| execute_item(&item)
                                                        on:mouseenter=move |_| selected_idx.set(idx)
                                                        role="option"
                                                        aria-selected=move || is_selected(idx)
                                                    >
                                                        <span class="cmd-palette-item-icon">{icon}</span>
                                                        <span class="cmd-palette-item-label">{item.label.clone()}</span>
                                                        {match item.category {
                                                            Category::Page => view! {
                                                                <kbd class="cmd-palette-item-hint">"↵"</kbd>
                                                            }.into_any(),
                                                            Category::Command => view! {
                                                                <kbd class="cmd-palette-item-hint">"⌘"</kbd>
                                                            }.into_any(),
                                                            Category::Recent => view! {
                                                                <span class="cmd-palette-item-hint">"recente"</span>
                                                            }.into_any(),
                                                        }}
                                                    </button>
                                                }
                                            })
                                            .collect::<Vec<_>>();
                                        view! {
                                            <div class="cmd-palette-group">
                                                {header}
                                                {list}
                                            </div>
                                        }
                                    })
                                    .collect_view()
                                    .into_any()
                            }
                        }}
                    </div>

                    <div class="cmd-palette-footer">
                        <span class="cmd-palette-footer-hint">
                            "↑↓ navegar · ↵ selecionar · esc fechar"
                        </span>
                    </div>
                </div>
            </div>
        </Show>
    }
}

// ─── Testes ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_exact_match_scores_highest() {
        let s1 = fuzzy_score("agentes", "Agentes");
        let s2 = fuzzy_score("ag", "Agentes");
        assert!(s1 > s2, "exact match deve ter score maior: {s1} vs {s2}");
    }

    #[test]
    fn fuzzy_partial_match_works() {
        let s = fuzzy_score("ag", "Agentes");
        assert!(s > 0, "partial match 'ag' em 'Agentes' deve pontuar");
    }

    #[test]
    fn fuzzy_no_match_returns_zero() {
        let s = fuzzy_score("xyz", "Agentes");
        assert_eq!(s, 0, "'xyz' não deve match 'Agentes'");
    }

    #[test]
    fn fuzzy_empty_query_matches_all() {
        let s = fuzzy_score("", "Qualquer coisa");
        assert!(s > 0, "empty query deve match tudo");
    }

    #[test]
    fn fuzzy_consecutive_bonus() {
        let consecutive = fuzzy_score("age", "Agentes");
        let scattered = fuzzy_score("aes", "Agentes");
        assert!(
            consecutive > scattered,
            "chars consecutivos devem pontuar mais: {consecutive} vs {scattered}"
        );
    }

    #[test]
    fn search_sorts_by_score() {
        let items = vec![
            PaletteItem {
                id: "1".into(),
                label: "Agentes".into(),
                category: Category::Page,
                tab: None,
                command: None,
            },
            PaletteItem {
                id: "2".into(),
                label: "Analytics".into(),
                category: Category::Page,
                tab: None,
                command: None,
            },
            PaletteItem {
                id: "3".into(),
                label: "Sentinela".into(),
                category: Category::Page,
                tab: None,
                command: None,
            },
        ];
        let results = search_items(&items, "agen");
        assert!(!results.is_empty());
        // "Agentes" deve vir antes de "Analytics" para query "agen"
        assert_eq!(results[0].0.label, "Agentes");
    }

    #[test]
    fn search_limits_to_max_visible() {
        let items: Vec<PaletteItem> = (0..50)
            .map(|i| PaletteItem {
                id: format!("item-{i}"),
                label: format!("Item {i}"),
                category: Category::Page,
                tab: None,
                command: None,
            })
            .collect();
        let results = search_items(&items, "item");
        assert!(results.len() <= MAX_VISIBLE);
    }

    #[test]
    fn build_static_index_has_pages_and_commands() {
        let items = build_static_index();
        let pages = items
            .iter()
            .filter(|i| i.category == Category::Page)
            .count();
        let cmds = items
            .iter()
            .filter(|i| i.category == Category::Command)
            .count();
        assert_eq!(pages, Tab::ALL.len());
        assert!(cmds >= 5, "deve ter pelo menos 5 comandos estáticos");
    }
}
