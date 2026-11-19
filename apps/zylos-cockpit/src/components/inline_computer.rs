//! Computador do agente dentro do chat (DESIGN_SYSTEM v2.1 §6.6, ADR-003).
//!
//! Lista as sessões de `GET /api/v1/computer-view/sessions` como cartões compactos no chat;
//! "Expandir" abre o stream (`VncStream`: WebSocket com screenshot de reserva e controle com lease).
//! Sem sessões, não mostra nada.

use leptos::prelude::*;
use serde::Deserialize;
use wasm_bindgen_futures::spawn_local;

use super::vnc_canvas::VncStream;

#[derive(Debug, Clone, PartialEq, Deserialize)]
struct SessionRow {
    id: String,
    bot_id: String,
    state: String,
}

#[derive(Debug, Clone, Deserialize)]
struct SessionsResp {
    sessions: Vec<SessionRow>,
}

/// Rótulo legível do estado da sessão.
fn state_label(state: &str) -> &'static str {
    match state {
        "connecting" => "Conectando",
        "active" | "live" => "Ao vivo",
        "controlled" | "human_control" => "Você está no controle",
        "paused" => "Pausado",
        "closed" | "terminated" => "Encerrado",
        _ => "Disponível",
    }
}

async fn fetch_sessions() -> Result<Vec<SessionRow>, String> {
    let base = crate::gateway_config::gateway_base().ok_or("Gateway não configurado")?;
    let url = crate::gateway_config::join_url(&base, "/api/v1/computer-view/sessions");
    let resp = crate::auth::authed(gloo_net::http::Request::get(&url))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let data: SessionsResp = resp.json().await.map_err(|e| e.to_string())?;
    Ok(data.sessions)
}

/// Cartões "Computador de <agente>" no chat. Atualiza a lista a cada 10 s.
#[component]
pub fn InlineComputers() -> impl IntoView {
    let (sessions, set_sessions) = signal(Vec::<SessionRow>::new());
    let (open_id, set_open_id) = signal(None::<String>);

    let reload = move || {
        spawn_local(async move {
            // Sem gateway ou com erro, o chat segue sem cartões de computador.
            if let Ok(list) = fetch_sessions().await {
                set_sessions.set(list);
            }
        });
    };
    reload();
    #[cfg(target_arch = "wasm32")]
    gloo_timers::callback::Interval::new(10_000, reload).forget();

    view! {
        <div class="chat-inline-computers">
            {move || {
                sessions
                    .get()
                    .into_iter()
                    .map(|s| {
                        let id = s.id.clone();
                        let id_toggle = s.id.clone();
                        let is_open = Memo::new(move |_| open_id.get().as_deref() == Some(id.as_str()));
                        let id_view = s.id.clone();
                        view! {
                            <section class="computer-mini" aria-label=format!("Computador de {}", s.bot_id)>
                                <header class="computer-mini-head">
                                    <i class="ph ph-device-mobile" aria-hidden="true"></i>
                                    <strong>{format!("Computador de {}", s.bot_id)}</strong>
                                    <span class="computer-mini-state">{state_label(&s.state)}</span>
                                    <button
                                        class="cta-secondary"
                                        aria-expanded=move || is_open.get().to_string()
                                        on:click=move |_| {
                                            let next = if open_id.get().as_deref() == Some(id_toggle.as_str()) {
                                                None
                                            } else {
                                                Some(id_toggle.clone())
                                            };
                                            set_open_id.set(next);
                                        }
                                    >
                                        <i class="ph ph-arrows-out-simple" aria-hidden="true"></i>
                                        {move || if is_open.get() { "Recolher" } else { "Expandir" }}
                                    </button>
                                </header>
                                {move || is_open.get().then(|| {
                                    let sid = id_view.clone();
                                    view! { <div class="computer-mini-body"><VncStream session_id=sid /></div> }
                                })}
                            </section>
                        }
                    })
                    .collect_view()
            }}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotulos_de_estado() {
        assert_eq!(state_label("connecting"), "Conectando");
        assert_eq!(state_label("active"), "Ao vivo");
        assert_eq!(state_label("controlled"), "Você está no controle");
        assert_eq!(state_label("qualquer"), "Disponível");
    }
}
