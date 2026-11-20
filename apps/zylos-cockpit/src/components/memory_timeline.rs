//! Memória por Space — notas guardadas no gateway (`/api/v1/memory/entries`).
//!
//! Cada nota pertence a um Space; o Geral mostra as de todos. O Space ativo é lido do
//! `localStorage` ao montar (o módulo extras não recebe o contexto de Spaces; o cockpit
//! remonta o painel quando o Space muda).
//! Estados: Loading, Empty, Success, Error + validação do formulário.

use leptos::prelude::*;
use serde::Deserialize;
use wasm_bindgen_futures::spawn_local;

/// Nota como devolvida pelo gateway.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Note {
    pub key: String,
    pub space_id: String,
    #[serde(default)]
    pub title: String,
    pub content: String,
    #[serde(default)]
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
struct EntriesResp {
    entries: Vec<Note>,
}

/// Limite de caracteres da nota (igual ao do gateway).
const MAX_CONTENT: usize = 8_000;

/// Texto da nota pronto para enviar: sem espaços nas pontas, nunca vazio nem acima do limite.
pub fn clean_note(s: &str) -> Result<String, &'static str> {
    let t = s.trim();
    if t.is_empty() {
        Err("Escreva a nota antes de salvar.")
    } else if t.chars().count() > MAX_CONTENT {
        Err("A nota passa de 8.000 caracteres.")
    } else {
        Ok(t.to_string())
    }
}

/// `2026-11-19T10:30:00+00:00` → `2026-11-19 10:30`.
pub fn short_date(rfc3339: &str) -> String {
    rfc3339.get(..16).unwrap_or(rfc3339).replace('T', " ")
}

#[component]
pub fn MemoryTimeline() -> impl IntoView {
    let space = crate::spaces::stored_active();
    let notes = RwSignal::new(None::<Result<Vec<Note>, String>>);
    let title = RwSignal::new(String::new());
    let content = RwSignal::new(String::new());
    let form_error = RwSignal::new(None::<String>);
    let saving = RwSignal::new(false);

    let load = {
        let space = space.clone();
        move || {
            let space = space.clone();
            spawn_local(async move {
                let r = fetch_notes(&space)
                    .await
                    .map_err(|e| crate::ui_error::report("memória", &e));
                notes.set(Some(r));
            });
        }
    };
    {
        let load = load.clone();
        Effect::new(move |_| load());
    }

    let on_save = {
        let space = space.clone();
        let load = load.clone();
        move || {
            let text = match clean_note(&content.get_untracked()) {
                Ok(t) => t,
                Err(m) => {
                    form_error.set(Some(m.to_string()));
                    return;
                }
            };
            form_error.set(None);
            saving.set(true);
            let (space, load) = (space.clone(), load.clone());
            let note_title = title.get_untracked().trim().to_string();
            spawn_local(async move {
                match create_note(&space, &note_title, &text).await {
                    Ok(()) => {
                        content.set(String::new());
                        title.set(String::new());
                        load();
                    }
                    Err(e) => form_error.set(Some(crate::ui_error::report("memória", &e))),
                }
                saving.set(false);
            });
        }
    };

    let heading = if space == crate::spaces::GENERAL_ID {
        "Memória de todos os Spaces".to_string()
    } else {
        format!(
            "Memória do Space {}",
            crate::spaces::stored_space_name(&space)
        )
    };

    view! {
        <div class="memory-timeline">
            <h3 class="memory-heading">{heading}</h3>

            <div class="memory-form">
                <input
                    class="memory-title-input"
                    type="text"
                    aria-label="Título da nota (opcional)"
                    placeholder="Título (opcional)"
                    maxlength="120"
                    prop:value=move || title.get()
                    on:input=move |ev| title.set(event_target_value(&ev))
                />
                <textarea
                    class="memory-note-input"
                    aria-label="Nova nota"
                    placeholder="O que este Space deve lembrar?"
                    rows="3"
                    prop:value=move || content.get()
                    on:input=move |ev| content.set(event_target_value(&ev))
                />
                {move || form_error.get().map(|m| view! { <p class="spaces-error" role="alert">{m}</p> })}
                <button
                    class="cta-primary"
                    disabled=move || saving.get()
                    on:click={
                        let on_save = on_save.clone();
                        move |_| on_save()
                    }
                >"Salvar nota"</button>
            </div>

            {move || match notes.get() {
                None => view! {
                    <div class="skeleton-grid" role="status" aria-label="Carregando notas">
                        <div class="skeleton-card pulse"></div>
                        <div class="skeleton-card pulse"></div>
                    </div>
                }.into_any(),
                Some(Err(msg)) => {
                    let load = load.clone();
                    view! {
                        <div class="error-banner" role="alert">
                            <p>{msg}</p>
                            <button class="cta-retry" on:click=move |_| load()>"Tentar novamente"</button>
                        </div>
                    }.into_any()
                }
                Some(Ok(list)) if list.is_empty() => view! {
                    <div class="empty-state">
                        <p>"Nenhuma nota neste Space ainda."</p>
                        <p class="muted">"Escreva acima o que os agentes deste Space devem lembrar."</p>
                    </div>
                }.into_any(),
                Some(Ok(list)) => {
                    let load = load.clone();
                    view! {
                        <ul class="memory-notes">
                            {list.into_iter().map(|n| {
                                let load = load.clone();
                                let (key, note_space) = (n.key.clone(), n.space_id.clone());
                                view! {
                                    <li class="memory-note">
                                        <div class="memory-note-head">
                                            <strong>{if n.title.is_empty() { "Sem título".to_string() } else { n.title.clone() }}</strong>
                                            <span class="muted">{format!("{} · {}", crate::spaces::stored_space_name(&n.space_id), short_date(&n.updated_at))}</span>
                                            <button
                                                class="memory-note-delete"
                                                aria-label="Apagar nota"
                                                on:click=move |_| {
                                                    let (key, space, load) = (key.clone(), note_space.clone(), load.clone());
                                                    spawn_local(async move {
                                                        if let Err(e) = delete_note(&space, &key).await {
                                                            let _ = crate::ui_error::report("memória", &e);
                                                        }
                                                        load();
                                                    });
                                                }
                                            >"Apagar"</button>
                                        </div>
                                        <p class="memory-note-body">{n.content.clone()}</p>
                                    </li>
                                }
                            }).collect_view()}
                        </ul>
                    }.into_any()
                }
            }}
        </div>
    }
}

async fn fetch_notes(space: &str) -> Result<Vec<Note>, String> {
    let path = format!(
        "/api/v1/memory/entries?space={}",
        crate::auth::url_encode(space)
    );
    let r: EntriesResp = crate::gateway_http::get_json(&path).await?;
    Ok(r.entries)
}

async fn create_note(space: &str, title: &str, content: &str) -> Result<(), String> {
    let _: serde_json::Value = crate::gateway_http::post_json(
        "/api/v1/memory/entries",
        &serde_json::json!({ "space": space, "title": title, "content": content }),
    )
    .await?;
    Ok(())
}

async fn delete_note(space: &str, key: &str) -> Result<(), String> {
    let path = format!(
        "/api/v1/memory/entries/{}?space={}",
        crate::auth::url_encode(key),
        crate::auth::url_encode(space)
    );
    let resp = crate::auth::authed(gloo_net::http::Request::delete(
        &crate::gateway_http::api_url(&path)?,
    ))
    .send()
    .await
    .map_err(|e| format!("{e:?}"))?;
    if resp.ok() {
        Ok(())
    } else {
        Err(format!("HTTP {}", resp.status()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nota_vazia_ou_longa_demais_e_rejeitada() {
        assert!(clean_note("   ").is_err());
        assert!(clean_note(&"a".repeat(MAX_CONTENT + 1)).is_err());
        assert_eq!(clean_note("  prazo  ").unwrap(), "prazo");
    }

    #[test]
    fn data_curta() {
        assert_eq!(short_date("2026-11-19T10:30:45+00:00"), "2026-11-19 10:30");
        assert_eq!(short_date("hoje"), "hoje");
    }

    #[test]
    fn desserializa_notas_do_gateway() {
        let r: EntriesResp = serde_json::from_str(
            r#"{"entries":[{"key":"k","space_id":"ops","title":"","content":"x","updated_at":"2026-11-19T10:30:00+00:00"}]}"#,
        )
        .unwrap();
        assert_eq!(r.entries[0].space_id, "ops");
    }
}
