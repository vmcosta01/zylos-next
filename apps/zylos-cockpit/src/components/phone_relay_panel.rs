//! PhoneRelayPanel — pareamento real de dispositivos móveis.
//!
//! Consome o gateway (`apps/zylos-gateway/src/relay_api.rs`):
//! - `POST   /api/v1/hermes/desktop/relay/pair-code` — código de 5 min + QR em SVG
//! - `GET    /api/v1/hermes/desktop/relay/devices` — dispositivos pareados
//! - `DELETE /api/v1/hermes/desktop/relay/tokens/revoke` — revoga dispositivo
//!
//! O relay de tela ainda não tem sessão no gateway: a área de preview mostra
//! estado vazio (nenhuma imagem simulada).
//!
//! ## CSS Classes
//! - `phone-relay-panel` — container raiz
//! - `qr-placeholder` — área do QR code
//! - `device-list` / `device-card` — lista e card de dispositivo
//! - `relay-screen relay-inactive` — preview sem sessão
//! - `pair-btn` — botões de ação

use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::{Deserialize, Serialize};

use crate::gateway_http::{delete_json, get_json, post_json};
use crate::ui_error::user_message;

const BASE: &str = "/api/v1/hermes/desktop/relay";

/// Dispositivo pareado, como devolvido pelo gateway.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PairedDevice {
    pub id: String,
    pub name: String,
    pub platform: String,
    /// Unix seconds.
    pub paired_at: u64,
    /// Unix seconds.
    pub last_seen: u64,
}

impl PairedDevice {
    /// Ícone da plataforma.
    pub fn icon(&self) -> &'static str {
        match self.platform.as_str() {
            "android" => "\u{1f916}",
            "ios" => "\u{1f4f1}",
            _ => "\u{1f4df}",
        }
    }
}

/// Código de pareamento emitido pelo gateway.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PairCode {
    pub code: String,
    pub expires_at: u64,
    pub payload: String,
    pub qr_svg: String,
}

#[derive(Deserialize)]
struct DevicesResponse {
    devices: Vec<PairedDevice>,
}

/// "há N min" a partir de dois instantes em segundos.
pub fn relative_time(then: u64, now: u64) -> String {
    let d = now.saturating_sub(then);
    match d {
        0..=59 => "agora".to_string(),
        60..=3599 => format!("há {} min", d / 60),
        3600..=86_399 => format!("há {} h", d / 3600),
        _ => format!("há {} dias", d / 86_400),
    }
}

/// Segundos restantes até `expires_at`.
pub fn remaining_secs(expires_at: u64, now: u64) -> u64 {
    expires_at.saturating_sub(now)
}

fn now_secs() -> u64 {
    (js_sys::Date::now() / 1000.0) as u64
}

/// Componente principal do painel Phone Relay
#[component]
pub fn PhoneRelayPanel() -> impl IntoView {
    let devices = RwSignal::new(Vec::<PairedDevice>::new());
    let loading = RwSignal::new(true);
    let error = RwSignal::new(None::<String>);
    let pair_code = RwSignal::new(None::<PairCode>);
    let now = RwSignal::new(now_secs());

    let show_err = move |e: String| {
        web_sys::console::warn_1(&format!("relay: {e}").into());
        error.set(Some(user_message(&e)));
    };

    let reload = move || {
        spawn_local(async move {
            match get_json::<DevicesResponse>(&format!("{BASE}/devices")).await {
                Ok(r) => {
                    error.set(None);
                    devices.set(r.devices);
                }
                Err(e) => show_err(e),
            }
            loading.set(false);
        });
    };
    reload();

    // Relógio para contagem regressiva do código e atualização periódica da lista.
    let ticks = StoredValue::new(0u32);
    let interval = set_interval_with_handle(
        move || {
            now.set(now_secs());
            let t = ticks.get_value() + 1;
            ticks.set_value(t);
            if t % 10 == 0 {
                reload();
            }
        },
        std::time::Duration::from_secs(1),
    );
    on_cleanup(move || {
        if let Ok(h) = interval {
            h.clear();
        }
    });

    let new_code = move |_| {
        spawn_local(async move {
            match post_json::<_, PairCode>(&format!("{BASE}/pair-code"), &serde_json::json!({}))
                .await
            {
                Ok(c) => {
                    error.set(None);
                    pair_code.set(Some(c));
                }
                Err(e) => show_err(e),
            }
        });
    };

    let revoke = move |id: String| {
        spawn_local(async move {
            let body = serde_json::json!({ "device_id": id });
            match delete_json::<_, serde_json::Value>(&format!("{BASE}/tokens/revoke"), &body)
                .await
            {
                Ok(_) => devices.update(|d| d.retain(|x| x.id != id)),
                Err(e) => show_err(e),
            }
        });
    };

    view! {
        <div class="phone-relay-panel">
            <header class="panel-header">
                <h2>{"\u{1f4f2} Phone Relay"}</h2>
                <p class="panel-subtitle">{"Gerencie dispositivos pareados e relay de tela"}</p>
            </header>
            {move || error.get().map(|e| view! { <div class="relay-error" role="alert">{e}</div> })}

            <section class="qr-section">
                <h3>{"Parear Novo Dispositivo"}</h3>
                {move || {
                    let code = pair_code.get().filter(|c| remaining_secs(c.expires_at, now.get()) > 0);
                    match code {
                        Some(c) => {
                            let left = remaining_secs(c.expires_at, now.get());
                            view! {
                                <div class="qr-placeholder">
                                    <div class="qr-inner">
                                        <div class="qr-svg" inner_html=c.qr_svg.clone()></div>
                                        <code class="qr-code">{c.code.clone()}</code>
                                        <p class="qr-hint">
                                            {format!("Escaneie com o app Zylos Mobile — expira em {}:{:02}", left / 60, left % 60)}
                                        </p>
                                    </div>
                                </div>
                            }.into_any()
                        }
                        None => view! {
                            <div class="qr-placeholder">
                                <div class="qr-inner">
                                    <p class="qr-hint">{"Gere um código para parear um celular (válido por 5 minutos)."}</p>
                                    <button class="pair-btn" on:click=new_code>{"Gerar código de pareamento"}</button>
                                </div>
                            </div>
                        }.into_any(),
                    }
                }}
            </section>

            <section class="devices-section">
                <h3>{"Dispositivos Pareados"}</h3>
                {move || {
                    if loading.get() {
                        view! { <p class="relay-loading">{"Carregando dispositivos..."}</p> }.into_any()
                    } else if devices.with(|d| d.is_empty()) {
                        view! { <p class="relay-empty">{"Nenhum dispositivo pareado."}</p> }.into_any()
                    } else {
                        ().into_any()
                    }
                }}
                <div class="device-list">
                    <For each=move || devices.get()
                         key=|d| d.id.clone()
                         children=move |dev| {
                             let id = dev.id.clone();
                             let last_seen = dev.last_seen;
                             let paired_at = dev.paired_at;
                             view! {
                                 <div class="device-card">
                                     <div class="device-header">
                                         <span class="device-platform">{dev.icon()}</span>
                                         <span class="device-name">{dev.name.clone()}</span>
                                     </div>
                                     <div class="device-meta">
                                         <span class="device-last-seen">
                                             {move || format!("Último contato: {}", relative_time(last_seen, now.get()))}
                                         </span>
                                         <span class="device-status-label">
                                             {move || format!("Pareado {}", relative_time(paired_at, now.get()))}
                                         </span>
                                     </div>
                                     <div class="device-actions">
                                         <button class="pair-btn" on:click=move |_| revoke(id.clone())>
                                             {"Revogar"}
                                         </button>
                                     </div>
                                 </div>
                             }
                         }
                    />
                </div>
            </section>

            <section class="relay-section">
                <h3>{"Relay de Tela"}</h3>
                <div class="relay-screen relay-inactive">
                    <p>{"Nenhuma sessão de relay ativa."}</p>
                </div>
            </section>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_devices_payload() {
        let raw = r#"{"total":1,"devices":[{"id":"dev-1","name":"Pixel","platform":"android","paired_at":10,"last_seen":20}]}"#;
        let r: DevicesResponse = serde_json::from_str(raw).unwrap();
        assert_eq!(r.devices.len(), 1);
        assert_eq!(r.devices[0].icon(), "\u{1f916}");
    }

    #[test]
    fn test_parse_pair_code_payload() {
        let raw = r#"{"code":"ABCD-EFGH","expires_at":300,"payload":"zylos-relay://pair?code=ABCD-EFGH","qr_svg":"<svg/>"}"#;
        let c: PairCode = serde_json::from_str(raw).unwrap();
        assert_eq!(c.code, "ABCD-EFGH");
        assert_eq!(remaining_secs(c.expires_at, 100), 200);
        assert_eq!(remaining_secs(c.expires_at, 400), 0);
    }

    #[test]
    fn test_relative_time() {
        assert_eq!(relative_time(100, 120), "agora");
        assert_eq!(relative_time(0, 180), "há 3 min");
        assert_eq!(relative_time(0, 7200), "há 2 h");
        assert_eq!(relative_time(0, 3 * 86_400), "há 3 dias");
        assert_eq!(relative_time(500, 100), "agora");
    }

    #[test]
    fn test_platform_icons() {
        let mut d = PairedDevice {
            id: "x".into(),
            name: "n".into(),
            platform: "ios".into(),
            paired_at: 0,
            last_seen: 0,
        };
        assert_eq!(d.icon(), "\u{1f4f1}");
        d.platform = "other".into();
        assert_eq!(d.icon(), "\u{1f4df}");
    }
}
