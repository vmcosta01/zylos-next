//! Zylos Cockpit — Lazy Roster Component (Task 6.2.2)
//!
//! Virtualized agent list supporting 500+ agents with <100ms render,
//! fuzzy search, tenant grouping, and status filtering per Master Plan.

use leptos::prelude::*;
use std::collections::HashMap;

use crate::components::agent_card::{AgentCard, AgentInfo, AgentStatus};
use crate::components::empty_state::{self, EmptyState};

/// Filter state for roster
#[derive(Debug, Clone, PartialEq)]
pub struct RosterFilter {
    pub search: String,
    pub status: Option<AgentStatus>,
    pub tenant: Option<String>,
}

#[allow(clippy::derivable_impls)]
impl Default for RosterFilter {
    fn default() -> Self {
        Self {
            search: String::new(),
            status: None,
            tenant: None,
        }
    }
}

/// Simple fuzzy match: checks if all chars in query appear in order within target
fn fuzzy_match(query: &str, target: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let query_lower = query.to_lowercase();
    let target_lower = target.to_lowercase();
    let mut q_chars = query_lower.chars();
    let mut current = match q_chars.next() {
        Some(c) => c,
        None => return true,
    };
    for t_char in target_lower.chars() {
        if t_char == current {
            match q_chars.next() {
                Some(c) => current = c,
                None => return true,
            }
        }
    }
    false
}

/// Group agents by tenant field extracted from ID prefix or metadata
fn group_by_tenant(agents: &[AgentInfo]) -> HashMap<String, Vec<AgentInfo>> {
    let mut groups: HashMap<String, Vec<AgentInfo>> = HashMap::new();
    for agent in agents {
        // Extract tenant from ID prefix (e.g., "tenant-001/agent-001" → "tenant-001")
        // Fallback to "default" if no prefix
        let tenant = agent
            .id
            .split('/')
            .next()
            .filter(|s| s.contains('-') && **s != *agent.id.as_str())
            .unwrap_or("default")
            .to_string();
        groups.entry(tenant).or_default().push(agent.clone());
    }
    groups
}

/// LazyRoster component — virtualized, searchable, filterable agent list
#[component]
pub fn LazyRoster(agents: Vec<AgentInfo>, #[prop(optional)] max_visible: usize) -> impl IntoView {
    let max_visible = if max_visible == 0 { 20 } else { max_visible };
    let (filter, set_filter) = signal(RosterFilter::default());
    let (visible_count, set_visible_count) = signal(max_visible);

    // Derived filtered + grouped list
    let filtered_agents = Memo::new(move |_| {
        let f = filter.get();
        let mut result: Vec<AgentInfo> = agents
            .iter()
            .filter(|a| {
                // Status filter
                if let Some(ref status) = f.status {
                    if &a.status != status {
                        return false;
                    }
                }
                // Tenant filter
                if let Some(ref tenant) = f.tenant {
                    let agent_tenant =
                        a.id.split('/')
                            .next()
                            .filter(|s| s.contains('-') && **s != *a.id.as_str())
                            .unwrap_or("default");
                    if agent_tenant != tenant {
                        return false;
                    }
                }
                // Fuzzy search
                fuzzy_match(&f.search, &a.name) || fuzzy_match(&f.search, &a.id)
            })
            .cloned()
            .collect();
        result.sort_by(|a, b| a.name.cmp(&b.name));
        result
    });

    let grouped = Memo::new(move |_| {
        let visible = visible_count.get();
        let all = filtered_agents.get();
        let truncated = if all.len() > visible {
            &all[..visible]
        } else {
            &all[..]
        };
        group_by_tenant(truncated)
    });

    let total_count = Memo::new(move |_| filtered_agents.get().len());
    let has_more = Memo::new(move |_| total_count.get() > visible_count.get());

    let load_more = move |_| {
        let current = visible_count.get();
        set_visible_count.set(current + max_visible);
    };

    let on_search_input = move |ev: web_sys::Event| {
        use wasm_bindgen::JsCast;
        if let Some(input) = ev
            .target()
            .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
        {
            let value = input.value();
            set_filter.update(|f| f.search = value);
            set_visible_count.set(max_visible); // Reset pagination on search
        }
    };

    let on_status_filter = move |status: Option<AgentStatus>| {
        set_filter.update(|f| f.status = status);
        set_visible_count.set(max_visible);
    };

    view! {
        <div class="lazy-roster">
            <div class="roster-toolbar">
                <input
                    type="text"
                    class="roster-search"
                    aria-label="Buscar agentes"
                    placeholder="Buscar agentes..."
                    on:input=on_search_input
                />
                <div class="roster-filters">
                    <button
                        class="filter-btn"
                        on:click=move |_| on_status_filter(None)
                    >"Todos"</button>
                    <button
                        class="filter-btn"
                        on:click=move |_| on_status_filter(Some(AgentStatus::Running))
                    >"Running"</button>
                    <button
                        class="filter-btn"
                        on:click=move |_| on_status_filter(Some(AgentStatus::Idle))
                    >"Idle"</button>
                    <button
                        class="filter-btn"
                        on:click=move |_| on_status_filter(Some(AgentStatus::Blocked))
                    >"Blocked"</button>
                </div>
                <span class="roster-count">
                    {move || format!("{} agentes", total_count.get())}
                </span>
            </div>

            <div class="roster-groups">
                {move || {
                    let groups = grouped.get();
                    if groups.is_empty() {
                        view! {
                            <EmptyState
                                icon=empty_state::icons::SEARCH
                                title="Nenhum agente encontrado".to_string()
                                description="Tente ajustar os filtros ou buscar por outro termo.".to_string()
                            />
                        }.into_any()
                    } else {
                        let mut sorted_tenants: Vec<_> = groups.iter().collect();
                        sorted_tenants.sort_by_key(|(k, _)| (*k).clone());
                        sorted_tenants
                            .into_iter()
                            .map(|(tenant, agents)| {
                                let tenant_name = tenant.clone();
                                let agent_cards = agents.iter().map(|agent| {
                                    view! { <AgentCard agent=agent.clone() /> }
                                }).collect::<Vec<_>>();
                                view! {
                                    <div class="roster-group">
                                        <h3 class="group-title">{tenant_name}</h3>
                                        <div class="agent-grid">
                                            {agent_cards}
                                        </div>
                                    </div>
                                }
                            })
                            .collect::<Vec<_>>()
                            .into_any()
                    }
                }}
            </div>

            {move || {
                if has_more.get() {
                    view! {
                        <div class="roster-load-more">
                            <button class="cta-secondary" on:click=load_more>
                                "Carregar mais"
                            </button>
                        </div>
                    }.into_any()
                } else {
                    view! { <div></div> }.into_any()
                }
            }}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_agent(id: &str, name: &str, status: AgentStatus) -> AgentInfo {
        AgentInfo {
            id: id.to_string(),
            name: name.to_string(),
            status,
            task_count: 0,
            last_active: "now".to_string(),
            activity: Vec::new(),
        }
    }

    #[test]
    fn test_fuzzy_match_exact() {
        assert!(fuzzy_match("chief", "Chief of Staff"));
    }

    #[test]
    fn test_fuzzy_match_partial() {
        assert!(fuzzy_match("cos", "Chief of Staff"));
    }

    #[test]
    fn test_fuzzy_match_no_match() {
        assert!(!fuzzy_match("xyz", "Chief of Staff"));
    }

    #[test]
    fn test_fuzzy_match_empty_query() {
        assert!(fuzzy_match("", "anything"));
    }

    #[test]
    fn test_fuzzy_match_case_insensitive() {
        assert!(fuzzy_match("CHIEF", "chief of staff"));
    }

    #[test]
    fn test_group_by_tenant_with_prefix() {
        let agents = vec![
            make_agent("proj-a/agent-001", "A1", AgentStatus::Idle),
            make_agent("proj-a/agent-002", "A2", AgentStatus::Idle),
            make_agent("proj-b/agent-001", "B1", AgentStatus::Idle),
        ];
        let groups = group_by_tenant(&agents);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups.get("proj-a").unwrap().len(), 2);
        assert_eq!(groups.get("proj-b").unwrap().len(), 1);
    }

    #[test]
    fn test_group_by_tenant_without_prefix() {
        let agents = vec![
            make_agent("agent-001", "A1", AgentStatus::Idle),
            make_agent("agent-002", "A2", AgentStatus::Idle),
        ];
        let groups = group_by_tenant(&agents);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups.get("default").unwrap().len(), 2);
    }

    #[test]
    fn test_group_by_tenant_mixed() {
        let agents = vec![
            make_agent("proj-a/agent-001", "A1", AgentStatus::Idle),
            make_agent("agent-002", "A2", AgentStatus::Idle),
        ];
        let groups = group_by_tenant(&agents);
        assert_eq!(groups.len(), 2);
        assert!(groups.contains_key("proj-a"));
        assert!(groups.contains_key("default"));
    }
}
