//! ThreadManager — threads de conversa persistidas no gateway.
//!
//! Inspirado em: OpenBot (`ThreadTabs.tsx`).
//! Dados reais de `/api/v1/threads` (SQLite no gateway):
//! - `GET /threads` lista; `POST /threads` cria;
//! - `PATCH /threads/:id` renomeia (`name`) ou reordena (`position`);
//! - `DELETE /threads/:id` apaga.
//!
//! Sem threads → estado vazio; erro → mensagem de [`crate::ui_error::report`].
//!
//! ## Related Documents
//! - Implementation Plan: [IMPLEMENTATION_PLAN_DETAILED.md](../../docs/IMPLEMENTATION_PLAN_DETAILED.md) §4.1

use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::{Deserialize, Serialize};

/// Uma thread como devolvida pelo gateway.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Thread {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub bot_id: String,
    #[serde(default)]
    pub position: i64,
    #[serde(default)]
    pub message_count: usize,
    #[serde(default)]
    pub last_message_at: Option<String>,
    /// Space dono da conversa (gateways antigos não mandam: vale o Geral).
    #[serde(default = "default_space")]
    pub space_id: String,
}

fn default_space() -> String {
    crate::spaces::GENERAL_ID.to_string()
}

#[derive(Debug, Clone, Deserialize)]
struct ThreadsResponse {
    threads: Vec<Thread>,
}

/// Ordena por `position` (estável) — a ordem exibida.
pub fn sort_threads(mut threads: Vec<Thread>) -> Vec<Thread> {
    threads.sort_by_key(|t| t.position);
    threads
}

/// Para mover `id` uma casa (`up` = para cima), devolve os dois PATCHes de
/// posição a enviar: `[(id, nova_pos), (vizinho, nova_pos)]`.
pub fn swap_positions(threads: &[Thread], id: &str, up: bool) -> Option<[(String, i64); 2]> {
    let i = threads.iter().position(|t| t.id == id)?;
    let j = if up { i.checked_sub(1)? } else { i + 1 };
    let other = threads.get(j)?;
    let me = &threads[i];
    // Usa os índices como posições para não depender de posições repetidas.
    Some([(me.id.clone(), j as i64), (other.id.clone(), i as i64)])
}

/// Nome válido para criar/renomear (não vazio após trim).
pub fn clean_name(s: &str) -> Option<String> {
    let t = s.trim();
    (!t.is_empty()).then(|| t.to_string())
}

#[component]
pub fn ThreadManager() -> impl IntoView {
    // None = carregando; Some(Ok) = dados; Some(Err) = mensagem amigável.
    let data = RwSignal::new(None::<Result<Vec<Thread>, String>>);
    let active = RwSignal::new(None::<String>);
    let editing = RwSignal::new(None::<String>);
    let draft = RwSignal::new(String::new());
    let action_error = RwSignal::new(None::<String>);

    let load = move || {
        spawn_local(async move {
            let r = fetch_threads()
                .await
                .map_err(|e| crate::ui_error::report("threads", &e));
            data.set(Some(r));
        });
    };
    Effect::new(move |_| load());

    // Executa uma ação remota e recarrega a lista; erro vai para o banner.
    let act =
        move |fut: std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>>>>| {
            spawn_local(async move {
                match fut.await {
                    Ok(()) => action_error.set(None),
                    Err(e) => action_error.set(Some(crate::ui_error::report("threads", &e))),
                }
                load();
            });
        };

    let on_create = move |_| {
        act(Box::pin(async {
            create_thread("Nova conversa").await.map(|_| ())
        }))
    };

    view! {
        <div class="thread-manager">
            <div class="thread-manager-header">
                <h3 class="thread-manager-title"><i class="ph ph-chats-circle" aria-hidden="true"></i>" Threads"</h3>
                <button class="thread-create-btn" on:click=on_create title="Nova thread">"+"</button>
            </div>

            {move || action_error.get().map(|m| view! {
                <div class="error-banner" role="alert"><p>{m}</p></div>
            })}

            {move || match data.get() {
                None => view! { <div class="thread-loading">"Carregando threads..."</div> }.into_any(),
                Some(Err(msg)) => view! {
                    <div class="error-banner" role="alert">
                        <p>{msg}</p>
                        <button class="cta-retry" on:click=move |_| load()>"Tentar novamente"</button>
                    </div>
                }.into_any(),
                Some(Ok(list)) if list.is_empty() => view! {
                    <div class="thread-empty">"Nenhuma thread ainda. Use + para criar."</div>
                }.into_any(),
                Some(Ok(list)) => {
                    let list = sort_threads(list);
                    let snapshot = list.clone();
                    view! {
                        <div class="thread-list">
                            {list.into_iter().map(|thread| {
                                let id = thread.id.clone();
                                let snap_up = snapshot.clone();
                                let snap_down = snapshot.clone();
                                let (id_sel, id_up, id_down, id_del, id_ren, id_edit, id_save) = (
                                    id.clone(), id.clone(), id.clone(), id.clone(),
                                    id.clone(), id.clone(), id.clone(),
                                );
                                let name = thread.name.clone();
                                let name_ren = name.clone();
                                let count = thread.message_count;
                                let is_active = move || active.get().as_deref() == Some(id_sel.as_str());
                                let save = move || {
                                    let id = id_save.clone();
                                    editing.set(None);
                                    if let Some(n) = clean_name(&draft.get_untracked()) {
                                        act(Box::pin(async move { rename_thread(&id, &n).await }));
                                    }
                                };
                                let save_key = save.clone();
                                let move_by = move |snap: Vec<Thread>, id: String, up: bool| {
                                    if let Some([a, b]) = swap_positions(&snap, &id, up) {
                                        act(Box::pin(async move {
                                            set_position(&a.0, a.1).await?;
                                            set_position(&b.0, b.1).await
                                        }));
                                    }
                                };
                                view! {
                                    <div
                                        class=move || if is_active() { "thread-item thread-active" } else { "thread-item" }
                                        on:click=move |_| active.set(Some(id.clone()))
                                    >
                                        {move || if editing.get().as_deref() == Some(id_edit.as_str()) {
                                            let save = save.clone();
                                            let save_key = save_key.clone();
                                            view! {
                                                <input
                                                    class="thread-rename-input"
                                                    prop:value=move || draft.get()
                                                    on:input=move |ev| draft.set(event_target_value(&ev))
                                                    on:keydown=move |ev| {
                                                        if ev.key() == "Enter" { save_key() }
                                                        if ev.key() == "Escape" { editing.set(None) }
                                                    }
                                                    on:blur=move |_| save()
                                                />
                                            }.into_any()
                                        } else {
                                            view! { <span class="thread-name">{name.clone()}</span> }.into_any()
                                        }}
                                        <span class="thread-meta">
                                            <span class="thread-count">{count}</span>
                                        </span>
                                        <div class="thread-actions">
                                            <button class="thread-action-btn" title="Renomear"
                                                on:click=move |ev| {
                                                    ev.stop_propagation();
                                                    draft.set(name_ren.clone());
                                                    editing.set(Some(id_ren.clone()));
                                                }
                                            ><i class="ph ph-pencil-simple" aria-hidden="true"></i></button>
                                            <button class="thread-action-btn" title="Mover para cima"
                                                on:click=move |ev| { ev.stop_propagation(); move_by(snap_up.clone(), id_up.clone(), true) }
                                            >"↑"</button>
                                            <button class="thread-action-btn" title="Mover para baixo"
                                                on:click=move |ev| { ev.stop_propagation(); move_by(snap_down.clone(), id_down.clone(), false) }
                                            >"↓"</button>
                                            <button class="thread-action-btn thread-delete-btn" title="Apagar thread"
                                                on:click=move |ev| {
                                                    ev.stop_propagation();
                                                    let id = id_del.clone();
                                                    if active.get_untracked().as_deref() == Some(id.as_str()) {
                                                        active.set(None);
                                                    }
                                                    act(Box::pin(async move { delete_thread(&id).await }));
                                                }
                                            ><i class="ph ph-x" aria-hidden="true"></i></button>
                                        </div>
                                    </div>
                                }
                            }).collect_view()}
                        </div>
                    }.into_any()
                }
            }}
        </div>
    }
}

// === API Functions ===

fn threads_url(suffix: &str) -> Result<String, String> {
    let base = crate::gateway_config::gateway_base()
        .ok_or("Gateway não configurado (meta zylos-gateway-url)")?;
    Ok(crate::gateway_config::join_url(
        &base,
        &format!("/api/v1/threads{suffix}"),
    ))
}

async fn send(req: gloo_net::http::Request) -> Result<gloo_net::http::Response, String> {
    let resp = req.send().await.map_err(|e| format!("{e:?}"))?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    Ok(resp)
}

async fn fetch_threads() -> Result<Vec<Thread>, String> {
    use gloo_net::http::Request;
    // O Space ativo filtra a lista (o Geral mostra todas).
    let suffix = format!(
        "?space={}",
        crate::auth::url_encode(&crate::spaces::stored_active())
    );
    let req = crate::auth::authed(Request::get(&threads_url(&suffix)?))
        .build()
        .map_err(|e| format!("{e:?}"))?;
    let r: ThreadsResponse = send(req)
        .await?
        .json()
        .await
        .map_err(|e| format!("{e:?}"))?;
    Ok(r.threads)
}

async fn create_thread(name: &str) -> Result<Thread, String> {
    use gloo_net::http::Request;
    let req = crate::auth::authed(Request::post(&threads_url("")?))
        .json(&serde_json::json!({ "name": name, "space_id": crate::spaces::stored_active() }))
        .map_err(|e| format!("{e:?}"))?;
    send(req).await?.json().await.map_err(|e| format!("{e:?}"))
}

async fn patch_thread(id: &str, body: serde_json::Value) -> Result<(), String> {
    use gloo_net::http::Request;
    let url = threads_url(&format!("/{}", crate::auth::url_encode(id)))?;
    let req = crate::auth::authed(Request::patch(&url))
        .json(&body)
        .map_err(|e| format!("{e:?}"))?;
    send(req).await.map(|_| ())
}

async fn rename_thread(id: &str, name: &str) -> Result<(), String> {
    patch_thread(id, serde_json::json!({ "name": name })).await
}

async fn set_position(id: &str, position: i64) -> Result<(), String> {
    patch_thread(id, serde_json::json!({ "position": position })).await
}

async fn delete_thread(id: &str) -> Result<(), String> {
    use gloo_net::http::Request;
    let url = threads_url(&format!("/{}", crate::auth::url_encode(id)))?;
    let req = crate::auth::authed(Request::delete(&url))
        .build()
        .map_err(|e| format!("{e:?}"))?;
    send(req).await.map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(id: &str, pos: i64) -> Thread {
        Thread {
            id: id.into(),
            name: id.into(),
            bot_id: String::new(),
            position: pos,
            message_count: 0,
            last_message_at: None,
            space_id: "geral".into(),
        }
    }

    #[test]
    fn desserializa_resposta_do_gateway() {
        let r: ThreadsResponse = serde_json::from_str(
            r#"{"threads":[{"id":"a","name":"A","bot_id":"","position":1,
                "message_count":2,"last_message_at":null,"created_at":"x"}]}"#,
        )
        .unwrap();
        assert_eq!(r.threads[0].message_count, 2);
    }

    #[test]
    fn ordena_e_troca_posicoes() {
        let v = sort_threads(vec![t("b", 5), t("a", 1), t("c", 9)]);
        assert_eq!(v[0].id, "a");
        let [x, y] = swap_positions(&v, "b", true).unwrap();
        assert_eq!(x, ("b".to_string(), 0));
        assert_eq!(y, ("a".to_string(), 1));
        assert!(swap_positions(&v, "a", true).is_none());
        assert!(swap_positions(&v, "c", false).is_none());
    }

    #[test]
    fn nome_vazio_rejeitado() {
        assert_eq!(clean_name("  "), None);
        assert_eq!(clean_name(" x "), Some("x".into()));
    }
}
