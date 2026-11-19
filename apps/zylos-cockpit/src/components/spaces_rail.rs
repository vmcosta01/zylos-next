//! Rail de Spaces na sidebar (DESIGN_SYSTEM v2.1 §5.1, APP_FLOW v2.1 §2).
//!
//! Lista os Spaces, escolhe o ativo, cria novos e edita quais agentes têm acesso. O Space ativo
//! filtra o Inbox, as aprovações no chat e os bots (ver `crate::spaces`).

use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::spaces::{create_space, delete_space, set_agents, use_spaces, SpacesCtx};

/// Nome de agente limpo para a lista de acesso (vazio e duplicado não entram).
pub(crate) fn add_agent(current: &[String], raw: &str) -> Option<Vec<String>> {
    let name = raw.trim();
    if name.is_empty() || name.chars().count() > 64 || current.iter().any(|a| a == name) {
        return None;
    }
    let mut next = current.to_vec();
    next.push(name.to_string());
    Some(next)
}

#[component]
pub fn SpacesRail() -> impl IntoView {
    let Some(ctx) = use_spaces() else {
        return ().into_any();
    };
    view! { <SpacesRailInner ctx=ctx /> }.into_any()
}

#[component]
fn SpacesRailInner(ctx: SpacesCtx) -> impl IntoView {
    let (creating, set_creating) = signal(false);
    let (new_name, set_new_name) = signal(String::new());
    let (agent_input, set_agent_input) = signal(String::new());
    let (error, set_error) = signal(None::<String>);

    let submit_new = move || {
        let name = new_name.get_untracked();
        if name.trim().is_empty() {
            return;
        }
        spawn_local(async move {
            match create_space(&name).await {
                Ok(sp) => {
                    set_error.set(None);
                    set_creating.set(false);
                    set_new_name.set(String::new());
                    ctx.reload();
                    ctx.select(&sp.id);
                }
                Err(e) => set_error.set(Some(e)),
            }
        });
    };

    let save_agents = move |next: Vec<String>| {
        let Some(sp) = ctx.active() else { return };
        spawn_local(async move {
            match set_agents(&sp.id, &next).await {
                Ok(()) => {
                    set_error.set(None);
                    ctx.reload();
                }
                Err(e) => set_error.set(Some(e)),
            }
        });
    };

    view! {
        <section class="spaces-rail" aria-label="Spaces">
            <div class="spaces-head">
                <span class="group-label">"SPACES"</span>
                <button
                    class="spaces-add"
                    aria-label="Novo Space"
                    title="Novo Space"
                    on:click=move |_| set_creating.update(|c| *c = !*c)
                >
                    <i class="ph ph-plus" aria-hidden="true"></i>
                </button>
            </div>

            {move || creating.get().then(|| view! {
                <form class="spaces-new" on:submit=move |ev| { ev.prevent_default(); submit_new(); }>
                    <input
                        type="text"
                        maxlength="60"
                        placeholder="Nome do Space"
                        aria-label="Nome do novo Space"
                        prop:value=move || new_name.get()
                        on:input=move |ev| set_new_name.set(event_target_value(&ev))
                    />
                    <button type="submit" class="cta-secondary">"Criar"</button>
                </form>
            })}

            <ul class="spaces-list" role="list">
                {move || ctx.list.get().into_iter().map(|sp| {
                    let id = sp.id.clone();
                    let id_click = sp.id.clone();
                    let is_active = Memo::new(move |_| ctx.active_id.get() == id);
                    view! {
                        <li>
                            <button
                                class=move || if is_active.get() { "spaces-item active" } else { "spaces-item" }
                                aria-current=move || is_active.get().to_string()
                                on:click=move |_| ctx.select(&id_click)
                            >
                                <i class="ph ph-folder-simple" aria-hidden="true"></i>
                                <span class="item-label">{sp.name.clone()}</span>
                            </button>
                        </li>
                    }
                }).collect_view()}
            </ul>

            {move || ctx.active().filter(|s| !s.open_access).map(|sp| {
                let agents = sp.agents.clone();
                let agents_add = sp.agents.clone();
                let id_del = sp.id.clone();
                view! {
                    <div class="spaces-access">
                        <p class="spaces-sub">"Agentes com acesso"</p>
                        {if agents.is_empty() {
                            view! { <p class="spaces-hint">"Nenhum ainda: este Space aparece vazio no Inbox e no chat."</p> }.into_any()
                        } else {
                            view! {
                                <ul class="spaces-chips">
                                    {agents.iter().map(|a| {
                                        let a = a.clone();
                                        let rest: Vec<String> = agents.iter().filter(|x| **x != a).cloned().collect();
                                        view! {
                                            <li class="spaces-chip">
                                                <span>{a.clone()}</span>
                                                <button
                                                    aria-label=format!("Remover {}", a)
                                                    on:click=move |_| save_agents(rest.clone())
                                                ><i class="ph ph-x" aria-hidden="true"></i></button>
                                            </li>
                                        }
                                    }).collect::<Vec<_>>()}
                                </ul>
                            }.into_any()
                        }}
                        <form class="spaces-new" on:submit=move |ev| {
                            ev.prevent_default();
                            if let Some(next) = add_agent(&agents_add, &agent_input.get_untracked()) {
                                set_agent_input.set(String::new());
                                save_agents(next);
                            }
                        }>
                            <input
                                type="text"
                                maxlength="64"
                                placeholder="Nome do agente"
                                aria-label="Adicionar agente ao Space"
                                prop:value=move || agent_input.get()
                                on:input=move |ev| set_agent_input.set(event_target_value(&ev))
                            />
                            <button type="submit" class="cta-secondary">"Adicionar"</button>
                        </form>
                        <button
                            class="spaces-delete"
                            on:click=move |_| {
                                let id = id_del.clone();
                                let ok = web_sys::window()
                                    .and_then(|w| w.confirm_with_message("Apagar este Space? As conversas e aprovações não são apagadas.").ok())
                                    .unwrap_or(false);
                                if ok {
                                    spawn_local(async move {
                                        match delete_space(&id).await {
                                            Ok(()) => ctx.reload(),
                                            Err(e) => set_error.set(Some(e)),
                                        }
                                    });
                                }
                            }
                        >"Apagar Space"</button>
                    </div>
                }
            })}

            {move || error.get().map(|e| view! { <p class="spaces-error" role="alert">{e}</p> })}
        </section>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adicionar_agente_ignora_vazio_e_duplicado() {
        let cur = vec!["chief".to_string()];
        assert_eq!(
            add_agent(&cur, "  dev "),
            Some(vec!["chief".into(), "dev".into()])
        );
        assert_eq!(add_agent(&cur, "chief"), None);
        assert_eq!(add_agent(&cur, "   "), None);
        assert_eq!(add_agent(&cur, &"x".repeat(65)), None);
    }
}
