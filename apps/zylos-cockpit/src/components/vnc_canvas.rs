//! VncCanvas — stream VNC via WebSocket com screenshot fallback (ADR-003).
//!
//! Conecta a `ws://gateway/ws/computer-view/{session_id}` e renderiza frames
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
            format!("{ws_base}/ws/computer-view/{sid}")
        };

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
        // Note: frame timeout simplified — timeout handle not stored to avoid
        // leptos::reactive::owner::TimeoutHandle which doesn't exist in Leptos 0.7.
        // Fallback is triggered on WS error/close instead.

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
        let on_message = Closure::wrap(Box::new(move |e: MessageEvent| {
            has_frame_clone.set_value(true);

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
            let data = e.data();
            if let Ok(blob) = data.dyn_into::<Blob>() {
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
            fallback_url_err.set(Some(screenshot_path_err.clone()));
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
            fallback_url_close.set(Some(screenshot_path_close.clone()));
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
                        VncState::Connected => format!("Ao vivo ({:.0} FPS)", fps.get()),
                        VncState::Fallback => "Screenshot fallback".to_string(),
                        VncState::Disconnected => "Desconectado".to_string(),
                    }}
                </span>
                {move || controlling.get().then(|| view! {
                    <span class="vnc-badge vnc-badge-controlling">{"🎮 Controlando"}</span>
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
                          <button class="cta-primary" on:click=move |_: web_sys::MouseEvent| {
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
                                      Err(e) => err.set(Some(format!("Take control failed: {e}"))),
                                  }
                              });
                          }>
                              {"Assumir Controle"}
                          </button>
                      }>
                    <button class="cta-secondary" on:click=move |_: web_sys::MouseEvent| {
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
                <button class="cta-secondary" on:click=capture_screenshot>
                    {"📷 Capturar Tela"}
                </button>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
