//! VncCanvas — stream VNC via WebSocket com screenshot fallback (ADR-003).
//!
//! Conecta a `ws://gateway/api/v1/ws/computer-view/{session_id}` e renderiza frames
//! binários (JPEG) em um `<canvas>`. Se o WS falhar ou ficar 3s sem frames,
//! ativa fallback automático via GET `/api/v1/computer-view/sessions/:id/screenshot`.
//!
//! ## CSS Classes
//! - `vnc-container` — wrapper relativo
//! - `vnc-canvas` — elemento canvas
//! - `vnc-fallback` — overlay de screenshot estática
//! - `vnc-overlay` — controles sobre o canvas
//! - `vnc-status-dot` — indicador de conexão
//! - `vnc-controls` — barra de botões
//! - `vnc-badge` — badge "Controlando" / "Visualizando"

use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    BinaryType, Blob, CanvasRenderingContext2d, CloseEvent, ErrorEvent, HtmlCanvasElement,
    ImageBitmap, MessageEvent, WebSocket,
};

use crate::gateway_http::{delete_json, get_json, post_json};

/// Estado da conexão VNC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VncState {
    Connecting,
    Connected,
    Fallback,
    Disconnected,
}

/// Resposta do endpoint de sessões.
#[derive(Debug, Clone, Deserialize)]
struct SessionsResponse {
    sessions: Vec<SessionInfo>,
}

#[derive(Debug, Clone, Deserialize)]
struct SessionInfo {
    id: String,
}

/// Request body para take-control.
#[derive(Serialize)]
struct TakeControlReq {
    user_id: String,
    duration_minutes: Option<u64>,
    reason: Option<String>,
}

/// Request body para release-control.
#[derive(Serialize)]
struct ReleaseControlReq {
    user_id: String,
}

/// Baixa o screenshot com `Authorization` e põe um `blob:` local em `target` (libera o anterior).
fn show_screenshot(target: RwSignal<Option<String>>, path: String) {
    spawn_local(async move {
        let Some(base) = crate::gateway_config::gateway_base() else {
            return;
        };
        let url = crate::gateway_config::join_url(&base, &path);
        let Ok(resp) = crate::auth::authed(gloo_net::http::Request::get(&url))
            .send()
            .await
        else {
            return;
        };
        if !resp.ok() {
            return;
        }
        let Ok(bytes) = resp.binary().await else {
            return;
        };
        let array = js_sys::Uint8Array::from(bytes.as_slice());
        let parts = js_sys::Array::of1(&array);
        let Ok(blob) = Blob::new_with_u8_array_sequence(&parts) else {
            return;
        };
        let Ok(object_url) = web_sys::Url::create_object_url_with_blob(&blob) else {
            return;
        };
        if let Some(old) = target.get_untracked().filter(|u| u.starts_with("blob:")) {
            let _ = web_sys::Url::revoke_object_url(&old);
        }
        target.set(Some(object_url));
    });
}

/// Componente VNC stream com fallback automático.
#[component]
pub fn VncStream(
    /// ID da sessão de computer-view. Se None, tenta descobrir a primeira ativa.
    #[prop(optional)]
    session_id: Option<String>,
) -> impl IntoView {
    let state = RwSignal::new(VncState::Connecting);
    let fps = RwSignal::new(0.0_f64);
    let controlling = RwSignal::new(false);
    let error_msg = RwSignal::new(None::<String>);
    let fallback_url = RwSignal::new(None::<String>);
    let active_session = RwSignal::new(session_id.clone());
    let has_frame = StoredValue::new(false);

    // Descobrir sessão ativa se não foi fornecida.
    if session_id.is_none() {
        spawn_local(async move {
            match get_json::<SessionsResponse>("/api/v1/computer-view/sessions").await {
                Ok(resp) => {
                    if let Some(s) = resp.sessions.first() {
                        active_session.set(Some(s.id.clone()));
                    } else {
                        state.set(VncState::Disconnected);
                        error_msg.set(Some("Nenhuma sessão ativa.".into()));
                    }
                }
                Err(e) => {
                    state.set(VncState::Disconnected);
                    error_msg.set(Some(format!("Erro ao listar sessões: {e}")));
                }
            }
        });
    }

    // Efeito principal: conecta WS quando temos session_id.
    Effect::new(move |_| {
        let sid = active_session.get()?;
        let ws_url = {
            let base = crate::gateway_config::gateway_base().unwrap_or_default();
            let ws_base = base
                .replace("http://", "ws://")
                .replace("https://", "wss://");
            // O gateway monta o relay sob /api/v1 (antes o cockpit pedia /ws/... e recebia 404)
            // e o navegador só consegue autenticar WebSocket por ?access_token=.
            crate::auth::authed_url(&format!("{ws_base}/api/v1/ws/computer-view/{sid}"))
        };

        // Caminho no gateway: o screenshot é baixado com Authorization e exibido como blob local
        // (o token não vai na URL, onde acabaria em log e histórico).
        let screenshot_path = format!("/api/v1/computer-view/sessions/{sid}/screenshot");

        // Reset state.
        state.set(VncState::Connecting);
        has_frame.set_value(false);
        error_msg.set(None);

        let ws = match WebSocket::new(&ws_url) {
            Ok(w) => w,
            Err(_) => {
                state.set(VncState::Fallback);
                return None;
            }
        };
        ws.set_binary_type(BinaryType::Blob);

        // Track frame timing for FPS.
        let frame_times: StoredValue<Vec<f64>> = StoredValue::new(Vec::new());
        // Sem nenhum frame em 3 s (ADR-003), cai no screenshot estático; o WebSocket segue aberto
        // e, se um frame chegar depois, o canvas volta a aparecer.
        {
            let (state_t, has_frame_t, fallback_t, shot_t) =
                (state, has_frame, fallback_url, screenshot_path.clone());
            let cb = Closure::once(move || {
                if !has_frame_t.get_value() && state_t.get_untracked() == VncState::Connected {
                    state_t.set(VncState::Fallback);
                    show_screenshot(fallback_t, shot_t);
                }
            });
            if let Some(w) = web_sys::window() {
                let _ = w.set_timeout_with_callback_and_timeout_and_arguments_0(
                    cb.as_ref().unchecked_ref(),
                    3000,
                );
            }
            cb.forget();
        }

        // onopen
        let state_clone = state;
        let on_open = Closure::wrap(Box::new(move |_: JsValue| {
            state_clone.set(VncState::Connected);
        }) as Box<dyn FnMut(JsValue)>);
        ws.set_onopen(Some(on_open.as_ref().unchecked_ref()));
        on_open.forget();

        // onmessage — renderiza frame JPEG no canvas.
        let fps_sig = fps;
        let frame_times_clone = frame_times;
        let has_frame_clone = has_frame;
        let state_msg = state;
        let on_message = Closure::wrap(Box::new(move |e: MessageEvent| {
            // Frame é imagem: binário (JPEG) ou o texto JSON do relay `{"type":"frame","data":<base64>}`.
            // Outras mensagens de texto (qualidade, erro) não são imagem.
            let data = e.data();
            let blob: Option<Blob> = if data.is_instance_of::<Blob>() {
                data.dyn_into::<Blob>().ok()
            } else {
                data.as_string()
                    .and_then(|t| frame_base64(&t))
                    .and_then(|b64| blob_from_base64(&b64))
            };
            let Some(blob) = blob else {
                return;
            };
            has_frame_clone.set_value(true);
            // Frame depois do fallback de 3 s: volta do screenshot para o stream.
            if state_msg.get_untracked() != VncState::Connected {
                state_msg.set(VncState::Connected);
            }

            // Calcular FPS.
            let now = js_sys::Date::now();
            frame_times_clone.update_value(|times| {
                times.push(now);
                // Manter apenas últimos 60 timestamps.
                while times.len() > 60 {
                    times.remove(0);
                }
                if times.len() >= 2 {
                    let elapsed = (times.last().unwrap() - times.first().unwrap()) / 1000.0;
                    if elapsed > 0.0 {
                        fps_sig.set((times.len() - 1) as f64 / elapsed);
                    }
                }
            });

            // Renderizar blob como imagem no canvas.
            {
                let promise =
                    web_sys::window().and_then(|w| w.create_image_bitmap_with_blob(&blob).ok());
                if let Some(promise) = promise {
                    wasm_bindgen_futures::spawn_local(async move {
                        if let Ok(bitmap) = wasm_bindgen_futures::JsFuture::from(promise).await {
                            if let Some(window) = web_sys::window() {
                                if let Some(document) = window.document() {
                                    if let Some(canvas) =
                                        document.get_element_by_id("vnc-canvas-el")
                                    {
                                        if let Ok(canvas) = canvas.dyn_into::<HtmlCanvasElement>() {
                                            if let Some(ctx) =
                                                canvas.get_context("2d").ok().flatten()
                                            {
                                                if let Ok(ctx) =
                                                    ctx.dyn_into::<CanvasRenderingContext2d>()
                                                {
                                                    if let Ok(bmp) =
                                                        bitmap.dyn_into::<ImageBitmap>()
                                                    {
                                                        canvas.set_width(bmp.width());
                                                        canvas.set_height(bmp.height());
                                                        let _ = ctx.draw_image_with_image_bitmap(
                                                            &bmp, 0.0, 0.0,
                                                        );
                                                        bmp.close();
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    });
                }
            }
        }) as Box<dyn FnMut(MessageEvent)>);
        ws.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        on_message.forget();

        // onerror
        let state_err = state;
        let fallback_url_err = fallback_url;
        let screenshot_path_err = screenshot_path.clone();
        let error_msg_err = error_msg;
        let on_error = Closure::wrap(Box::new(move |_: ErrorEvent| {
            state_err.set(VncState::Fallback);
            show_screenshot(fallback_url_err, screenshot_path_err.clone());
            error_msg_err.set(Some("WebSocket error".into()));
        }) as Box<dyn FnMut(ErrorEvent)>);
        ws.set_onerror(Some(on_error.as_ref().unchecked_ref()));
        on_error.forget();

        // onclose
        let state_close = state;
        let fallback_url_close = fallback_url;
        let screenshot_path_close = screenshot_path.clone();
        let on_close = Closure::wrap(Box::new(move |_: CloseEvent| {
            state_close.set(VncState::Disconnected);
            show_screenshot(fallback_url_close, screenshot_path_close.clone());
        }) as Box<dyn FnMut(CloseEvent)>);
        ws.set_onclose(Some(on_close.as_ref().unchecked_ref()));
        on_close.forget();

        // Cleanup: fechar WS quando o componente é desmontado.
        let ws_cleanup = ws.clone();
        on_cleanup(move || {
            let _ = ws_cleanup.close();
        });

        Some(ws)
    });

    // Auto-retry do fallback a cada 5s.
    let retry_interval = set_interval_with_handle(
        move || {
            if state.get() == VncState::Fallback || state.get() == VncState::Disconnected {
                // Forçar re-execução do Effect reconectando.
                let current = active_session.get();
                active_session.set(None);
                active_session.set(current);
            }
        },
        std::time::Duration::from_secs(5),
    );
    on_cleanup(move || {
        if let Ok(h) = retry_interval {
            h.clear();
        }
    });

    // Note: auto-release after 5 min of inactivity simplified — timeout handle
    // not stored to avoid leptos::reactive::owner::TimeoutHandle which doesn't exist
    // in Leptos 0.7. Server-side lease expiration handles this instead.

    // Manual screenshot download.
    let capture_screenshot = move |_| {
        if let Some(window) = web_sys::window() {
            if let Some(document) = window.document() {
                if let Some(canvas) = document.get_element_by_id("vnc-canvas-el") {
                    if let Ok(canvas) = canvas.dyn_into::<HtmlCanvasElement>() {
                        if let Ok(data_url) = canvas.to_data_url_with_type("image/png") {
                            // Criar link temporário para download.
                            if let Ok(link) = document.create_element("a") {
                                let _ = link.set_attribute("href", &data_url);
                                let _ = link.set_attribute("download", "vnc-screenshot.png");
                                if let Ok(el) = link.dyn_into::<web_sys::HtmlElement>() {
                                    el.click();
                                }
                            }
                        }
                    }
                }
            }
        }
    };

    view! {
        <div class="vnc-container">
            // Status indicator.
            <div class="vnc-status-bar">
                <span class="vnc-status-dot" class:connected=move || state.get() == VncState::Connected
                      class:connecting=move || state.get() == VncState::Connecting
                      class:fallback=move || state.get() == VncState::Fallback
                      class:disconnected=move || state.get() == VncState::Disconnected></span>
                <span class="vnc-status-text">
                    {move || match state.get() {
                        VncState::Connecting => "Conectando...".to_string(),
                        VncState::Connected if fps.get() > 0.0 => format!("Ao vivo ({:.0} FPS)", fps.get()),
                        VncState::Connected => "Conectado, aguardando imagem".to_string(),
                        VncState::Fallback => "Screenshot fallback".to_string(),
                        VncState::Disconnected => "Desconectado".to_string(),
                    }}
                </span>
                {move || controlling.get().then(|| view! {
                    <span class="vnc-badge vnc-badge-controlling"><i class="ph ph-game-controller" aria-hidden="true"></i>" Controlando"</span>
                })}
            </div>

            // Canvas (sempre presente, visível quando conectado).
            <canvas id="vnc-canvas-el" class="vnc-canvas"
                    style:display=move || if state.get() == VncState::Connected { "" } else { "none" }>
            </canvas>

            // Fallback screenshot.
            {move || {
                if state.get() == VncState::Fallback || state.get() == VncState::Disconnected {
                    let url = fallback_url.get().unwrap_or_default();
                    view! {
                        <div class="vnc-fallback">
                            {move || {
                                if !url.is_empty() {
                                    view! {
                                        <img class="vnc-fallback-img" src=url.clone() alt="Screenshot fallback" />
                                    }.into_any()
                                } else {
                                    view! {
                                        <div class="vnc-fallback-placeholder">
                                            <p>{"Nenhum frame disponível."}</p>
                                        </div>
                                    }.into_any()
                                }
                            }}
                            <div class="vnc-fallback-overlay">
                                <p>{"Conexão perdida — tentando reconectar..."}</p>
                            </div>
                        </div>
                    }.into_any()
                } else if state.get() == VncState::Connecting {
                    view! {
                        <div class="vnc-loading">
                            <div class="spinner"></div>
                            <p>{"Conectando ao stream..."}</p>
                        </div>
                    }.into_any()
                } else {
                    ().into_any()
                }
            }}

            // Error message.
            {move || error_msg.get().map(|e| view! {
                <div class="vnc-error" role="alert">{e}</div>
            })}

            // Controls bar.
            <div class="vnc-controls">
                <Show when=move || controlling.get()
                      fallback=move || view! {
                          <button class="cta-primary" aria-label="Assumir controle do computador remoto" on:click=move |_: web_sys::MouseEvent| {
                              let sid = active_session.get().unwrap_or_default();
                              let ctrl = controlling;
                              let err = error_msg;
                              spawn_local(async move {
                                  let req = TakeControlReq {
                                      user_id: "cockpit-user".into(),
                                      duration_minutes: Some(5),
                                      reason: Some("Manual control from Cockpit".into()),
                                  };
                                  match post_json::<_, serde_json::Value>(
                                      &format!("/api/v1/computer-view/sessions/{sid}/control"),
                                      &req,
                                  ).await {
                                      Ok(_) => ctrl.set(true),
                                      Err(e) => err.set(Some(erro_de_controle(&e))),
                                  }
                              });
                          }>
                              {"Assumir Controle"}
                          </button>
                      }>
                    <button class="cta-secondary" aria-label="Liberar controle do computador remoto" on:click=move |_: web_sys::MouseEvent| {
                        let sid = active_session.get().unwrap_or_default();
                        let ctrl = controlling;
                        let err = error_msg;
                        spawn_local(async move {
                            let req = ReleaseControlReq {
                                user_id: "cockpit-user".into(),
                            };
                            match delete_json::<_, serde_json::Value>(
                                &format!("/api/v1/computer-view/sessions/{sid}/control"),
                                &req,
                            ).await {
                                Ok(_) => ctrl.set(false),
                                Err(e) => err.set(Some(format!("Release failed: {e}"))),
                            }
                        });
                    }>
                        {"Liberar Controle"}
                    </button>
                </Show>
                <button class="cta-secondary" aria-label="Capturar screenshot da sessão remota" on:click=capture_screenshot>
                    <i class="ph ph-camera" aria-hidden="true"></i>" Capturar Tela"
                </button>
            </div>
        </div>
    }
}

/// Texto para a tela quando "Assumir controle" falha. O gateway responde 409 se a sessão ainda não
/// está ativa (nenhum frame chegou) ou se outra pessoa já está no controle.
fn erro_de_controle(e: &str) -> String {
    if e.contains("409") {
        "Não deu para assumir o controle: a sessão ainda não está ativa (nenhum quadro chegou) ou outra pessoa já está no controle."
            .to_string()
    } else {
        format!("Não deu para assumir o controle: {e}")
    }
}

/// Mensagem de texto do relay do gateway: `{"type":"frame","data":"<base64>",...}`.
#[derive(Deserialize)]
struct TextoFrame {
    #[serde(rename = "type")]
    tipo: String,
    #[serde(default)]
    data: String,
}

/// O base64 da imagem, se o texto for um frame do relay; `None` para qualidade, erro, lixo.
fn frame_base64(texto: &str) -> Option<String> {
    let m: TextoFrame = serde_json::from_str(texto).ok()?;
    (m.tipo == "frame" && !m.data.is_empty()).then_some(m.data)
}

/// Decodifica o base64 (via `atob`) num `Blob`; o navegador reconhece PNG/JPEG pelos bytes.
fn blob_from_base64(b64: &str) -> Option<Blob> {
    let bin = web_sys::window()?.atob(b64).ok()?;
    let bytes = js_sys::Uint8Array::new_with_length(bin.len() as u32);
    for (i, c) in bin.chars().enumerate() {
        bytes.set_index(i as u32, c as u8);
    }
    let partes = js_sys::Array::of1(&bytes);
    Blob::new_with_u8_array_sequence(&partes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn erro_de_controle_explica_o_409() {
        assert!(erro_de_controle("HTTP 409").contains("ainda não está ativa"));
        assert_eq!(
            erro_de_controle("HTTP 500"),
            "Não deu para assumir o controle: HTTP 500"
        );
    }

    #[test]
    fn frame_de_texto_do_relay_vira_base64() {
        let t = r#"{"type":"frame","data":"iVBORw0KGgo=","ts":1,"seq":2}"#;
        assert_eq!(frame_base64(t).as_deref(), Some("iVBORw0KGgo="));
    }

    #[test]
    fn mensagens_que_nao_sao_imagem_nao_viram_frame() {
        assert_eq!(
            frame_base64(r#"{"type":"quality","fps":15.0,"latency_ms":8,"bitrate_kbps":1}"#),
            None
        );
        assert_eq!(frame_base64(r#"{"type":"error","message":"x"}"#), None);
        assert_eq!(frame_base64(r#"{"type":"frame","data":""}"#), None);
        assert_eq!(frame_base64("não é json"), None);
    }

    #[test]
    fn test_vnc_state_equality() {
        assert_eq!(VncState::Connecting, VncState::Connecting);
        assert_ne!(VncState::Connected, VncState::Fallback);
    }

    #[test]
    fn test_take_control_req_serialization() {
        let req = TakeControlReq {
            user_id: "user-1".into(),
            duration_minutes: Some(5),
            reason: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("user-1"));
        assert!(json.contains("5"));
    }

    #[test]
    fn test_release_control_req_serialization() {
        let req = ReleaseControlReq {
            user_id: "user-1".into(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("user-1"));
    }
}
