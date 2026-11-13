//! PhoneRelayPanel — UI para pareamento e relay de dispositivos móveis
//!
//! Exibe QR code placeholder, lista de dispositivos pareados com status,
//! e controles de pair/unpair. Usa mock data; sem chrono nem dependências WASM pesadas.
//!
//! ## CSS Classes
//! - `phone-relay-panel` — container raiz
//! - `qr-placeholder` — área do QR code estilizada
//! - `device-list` — lista de dispositivos
//! - `device-card` — card individual de dispositivo
//! - `device-paired` — modificador: dispositivo pareado e online
//! - `device-offline` — modificador: dispositivo offline
//! - `relay-screen` — preview da tela remota (placeholder)
//! - `pair-btn` — botão de pareamento/despareamento

use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::{Deserialize, Serialize};
use wasm_bindgen::closure::Closure;

/// Fetch JSON genérico via gloo-net (fallback para mock se falhar).
async fn fetch_json<T: for<'de> Deserialize<'de>>(url: &str) -> Result<T, String> {
    use gloo_net::http::Request;
    let resp = Request::get(url)
        .send()
        .await
        .map_err(|e| format!("{:?}", e))?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json::<T>().await.map_err(|e| format!("{:?}", e))
}

/// Status de um dispositivo pareado
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceStatus {
    Online,
    Offline,
    Pairing,
}

impl DeviceStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Online => "Online",
            Self::Offline => "Offline",
            Self::Pairing => "Pareando...",
        }
    }

    pub fn indicator(&self) -> &'static str {
        match self {
            Self::Online => "\u{1f7e2}",   // 🟢
            Self::Offline => "\u{1f534}",  // 🔴
            Self::Pairing => "\u{1f7e1}",  // 🟡
        }
    }
}

/// Plataforma do dispositivo
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Android,
    Ios,
}

impl Platform {
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Android => "\u{1f916}", // 🤖
            Self::Ios => "\u{1f4f1}",     // 📱
        }
    }
}

/// Dispositivo pareado
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PairedDevice {
    pub id: String,
    pub name: String,
    pub platform: Platform,
    /// Timestamp ISO-8601 como string (sem chrono)
    pub paired_at: String,
    /// Último contato como string relativa
    pub last_seen: String,
    pub status: DeviceStatus,
}

/// Sessão ativa de relay de tela
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RelaySession {
    pub device_id: String,
    pub screen_url: String,
    pub control_active: bool,
}

/// Métricas de qualidade do relay (recebidas via WS)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QualityMetrics {
    pub fps: f64,
    pub latency_ms: u64,
    pub bitrate_kbps: u64,
}

impl Default for QualityMetrics {
    fn default() -> Self {
        Self {
            fps: 0.0,
            latency_ms: 0,
            bitrate_kbps: 0,
        }
    }
}

/// Dados mock para demonstração
fn mock_devices() -> Vec<PairedDevice> {
    vec![
        PairedDevice {
            id: "dev-001".to_string(),
            name: "Pixel 8 Pro".to_string(),
            platform: Platform::Android,
            paired_at: "2026-11-10T14:30:00Z".to_string(),
            last_seen: "2 min atrás".to_string(),
            status: DeviceStatus::Online,
        },
        PairedDevice {
            id: "dev-002".to_string(),
            name: "iPhone 15".to_string(),
            platform: Platform::Ios,
            paired_at: "2026-11-09T09:15:00Z".to_string(),
            last_seen: "3h atrás".to_string(),
            status: DeviceStatus::Offline,
        },
        PairedDevice {
            id: "dev-003".to_string(),
            name: "Galaxy S24".to_string(),
            platform: Platform::Android,
            paired_at: "2026-11-11T18:00:00Z".to_string(),
            last_seen: "agora".to_string(),
            status: DeviceStatus::Pairing,
        },
    ]
}

/// Componente principal do painel Phone Relay
#[component]
pub fn PhoneRelayPanel() -> impl IntoView {
    let devices = RwSignal::new(mock_devices());
    let active_session = RwSignal::new(None::<RelaySession>);
    let pairing_code = RwSignal::new(Some("ZYLOS-7X4K-M9Q2".to_string()));
    let quality_metrics = RwSignal::new(QualityMetrics::default());

    // Carregar dispositivos do gateway (fallback para mock)
     Effect::new(move |_| {
     let devs_signal = devices;
     spawn_local(async move {
     match fetch_json::<serde_json::Value>("http://localhost:3000/api/v1/computer-view/sessions").await {
     Ok(val) => {
     if let Some(arr) = val.get("sessions").and_then(|v| v.as_array()) {
     // Mapeia sessões do gateway para PairedDevice
     let mapped: Vec<PairedDevice> = arr.iter().filter_map(|s| {
     let id = s.get("id")?.as_str()?.to_string();
     let bot_id = s.get("bot_id")?.as_str().unwrap_or("unknown").to_string();
     let state = s.get("state")?.as_str().unwrap_or("connecting");
     let status = match state {
     "active" | "user_control" => DeviceStatus::Online,
     "terminated" | "error" => DeviceStatus::Offline,
     _ => DeviceStatus::Pairing,
     };
     Some(PairedDevice {
     id,
     name: bot_id,
     platform: Platform::Android,
     paired_at: s.get("created_at").and_then(|v| v.as_str()).unwrap_or("").to_string(),
     last_seen: s.get("updated_at").and_then(|v| v.as_str()).unwrap_or("agora").to_string(),
     status,
     })
     }).collect();
     if !mapped.is_empty() {
     devs_signal.set(mapped);
     }
     }
     }
     Err(_) => {
     // Fallback: mantém dados mock
     }
     }
     });
     });

    view! {
        <div class="phone-relay-panel">
            <header class="panel-header">
                <h2>{"\u{1f4f2} Phone Relay"}</h2>
                <p class="panel-subtitle">{"Gerencie dispositivos pareados e relay de tela"}</p>
            </header>

            <section class="qr-section">
                <h3>{"Parear Novo Dispositivo"}</h3>
                <div class="qr-placeholder">
                    <div class="qr-inner">
                        <span class="qr-icon">{"\u{1f4f1}"}</span>
                        <code class="qr-code">{move || pairing_code.get().unwrap_or_else(|| "—".to_string())}</code>
                        <p class="qr-hint">{"Escaneie com o app Zylos Mobile"}</p>
                    </div>
                </div>
            </section>

            <section class="devices-section">
                <h3>{"Dispositivos Pareados"}</h3>
                <div class="device-list">
                    <For each=move || devices.get()
                         key=|d| d.id.clone()
                         children=move |device| {
                             let dev = device.clone();
                             let is_online = dev.status == DeviceStatus::Online;
                             let card_class = move || {
                                 if is_online {
                                     "device-card device-paired".to_string()
                                 } else {
                                     "device-card device-offline".to_string()
                                 }
                             };
                             let session_signal = active_session;
                             let devs_signal = devices;
                             let dev_id = dev.id.clone();
                                                         let dev_id_for_relay = dev_id.clone();
                                                         view! {
                                                             <div class=card_class>
                                                                 <div class="device-header">
                                                                     <span class="device-platform">{dev.platform.icon()}</span>
                                                                     <span class="device-name">{dev.name.clone()}</span>
                                                                     <span class="device-status-indicator">{dev.status.indicator()}</span>
                                                                 </div>
                                                                 <div class="device-meta">
                                                                     <span class="device-last-seen">{dev.last_seen.clone()}</span>
                                                                     <span class="device-status-label">{dev.status.label()}</span>
                                                                 </div>
                                                                 <div class="device-actions">
                                                                     <button
                                                                         class="pair-btn"
                                                                         on:click=move |_| {
                                                                             let id = dev_id.clone();
                                                 spawn_local(async move {
                                                     // Toggle unpair: remove da lista mock
                                                     let mut current = devs_signal.get();
                                                     current.retain(|d| d.id != id);
                                                     devs_signal.set(current);
                                                 });
                                             }
                                         >
                                             {"Desparear"}
                                         </button>
                                         {move || {
                                             if is_online {
                                                                                             let sid = dev_id_for_relay.clone();
                                                 view! {
                                                     <button
                                                         class="pair-btn relay-btn"
                                                         on:click=move |_| {
                                                             session_signal.set(Some(RelaySession {
                                                                 device_id: sid.clone(),
                                                                 screen_url: format!("ws://localhost:9876/relay/{}", sid),
                                                                 control_active: true,
                                                             }));
                                                         }
                                                     >
                                                         {"Iniciar Relay"}
                                                     </button>
                                                 }.into_any()
                                             } else {
                                              ().into_any()
                                              }
                                         }}
                                     </div>
                                 </div>
                             }
                         }
                    />
                </div>
            </section>

            <section class="relay-section">
                <h3>{"Relay de Tela"}</h3>
                {move || match &*active_session.read() {
                    Some(session) => {
                        let ws_url = session.screen_url.clone();
                        let dev_id = session.device_id.clone();
                        let metrics = quality_metrics;
                        let ctrl_active = session.control_active;
                        // Conectar WebSocket e renderizar frames no canvas
                        Effect::new(move |_| {
                            let url = ws_url.clone();
                            let m = metrics;
                            spawn_local(async move {
                                use wasm_bindgen::JsCast;
                                use web_sys::{WebSocket, MessageEvent};
                                let ws = match WebSocket::new(&url) {
                                    Ok(w) => w,
                                    Err(_) => return,
                                };
                                let ws_clone = ws.clone();
                                // Handler de mensagens: frame → canvas, quality → metrics
                                let on_msg = Closure::wrap(Box::new(move |e: MessageEvent| {
                                    if let Ok(text) = e.data().dyn_into::<js_sys::JsString>() {
                                        let s: String = text.into();
                                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&s) {
                                            match val.get("type").and_then(|t| t.as_str()) {
                                                Some("frame") => {
                                                    if let Some(data) = val.get("data").and_then(|d| d.as_str()) {
                                                        // Renderizar base64 no canvas via JS inline
                                                        let js_render = format!(
                                                            "(function(){{var c=document.getElementById('relay-canvas');if(!c)return;var ctx=c.getContext('2d');var img=new Image();img.onload=function(){{ctx.drawImage(img,0,0,c.width,c.height)}};img.src='data:image/jpeg;base64,{}'}})()",
                                                            data
                                                        );
                                                        let _ = js_sys::eval(&js_render);
                                                    }
                                                }
                                                Some("quality") => {
                                                    let fps = val.get("fps").and_then(|v| v.as_f64()).unwrap_or(0.0);
                                                    let latency = val.get("latency_ms").and_then(|v| v.as_u64()).unwrap_or(0);
                                                    let bitrate = val.get("bitrate_kbps").and_then(|v| v.as_u64()).unwrap_or(0);
                                                    m.set(QualityMetrics { fps, latency_ms: latency, bitrate_kbps: bitrate });
                                                }
                                                _ => {}
                                            }
                                        }
                                    }
                                }) as Box<dyn FnMut(MessageEvent)>);
                                ws_clone.set_onmessage(Some(on_msg.as_ref().unchecked_ref()));
                                on_msg.forget();
                                // Manter WS vivo até componente ser descartado
                                let _ = wasm_bindgen_futures::JsFuture::from(js_sys::Promise::new(&mut |resolve, _| {
                                    // Nunca resolve — mantém conexão aberta
                                    let _ = resolve;
                                })).await;
                            });
                        });
                        view! {
                            <div class="relay-screen">
                                <div class="relay-quality-bar">
                                    <span class="quality-fps">{move || format!("FPS: {:.1}", quality_metrics.get().fps)}</span>
                                    <span class="quality-latency">{move || format!("Latência: {}ms", quality_metrics.get().latency_ms)}</span>
                                    <span class="quality-bitrate">{move || format!("Bitrate: {}kbps", quality_metrics.get().bitrate_kbps)}</span>
                                </div>
                                <canvas id="relay-canvas" width="1280" height="720" class="relay-canvas"></canvas>
                                <div class="relay-info">
                                    <p>{format!("Conectado a {}", dev_id)}</p>
                                    <label class="relay-control-toggle">
                                        <input type="checkbox" checked=ctrl_active />
                                        {" Controle remoto ativo"}
                                    </label>
                                </div>
                            </div>
                        }.into_any()
                    }
                    None => view! {
                        <div class="relay-screen relay-inactive">
                            <p>{"Selecione um dispositivo online para iniciar o relay"}</p>
                        </div>
                    }.into_any(),
                }}
            </section>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_devices_count() {
        let devices = mock_devices();
        assert_eq!(devices.len(), 3);
    }

    #[test]
    fn test_device_status_labels() {
        assert_eq!(DeviceStatus::Online.label(), "Online");
        assert_eq!(DeviceStatus::Offline.label(), "Offline");
        assert_eq!(DeviceStatus::Pairing.label(), "Pareando...");
    }

    #[test]
    fn test_device_status_indicators() {
        assert_eq!(DeviceStatus::Online.indicator(), "\u{1f7e2}");
        assert_eq!(DeviceStatus::Offline.indicator(), "\u{1f534}");
        assert_eq!(DeviceStatus::Pairing.indicator(), "\u{1f7e1}");
    }

    #[test]
    fn test_platform_icons() {
        assert_eq!(Platform::Android.icon(), "\u{1f916}");
        assert_eq!(Platform::Ios.icon(), "\u{1f4f1}");
    }

    #[test]
    fn test_paired_device_partial_eq() {
        let d1 = PairedDevice {
            id: "dev-001".to_string(),
            name: "Test".to_string(),
            platform: Platform::Android,
            paired_at: "2026-01-01T00:00:00Z".to_string(),
            last_seen: "now".to_string(),
            status: DeviceStatus::Online,
        };
        let d2 = d1.clone();
        assert_eq!(d1, d2);
    }

    #[test]
    fn test_relay_session_partial_eq() {
        let s1 = RelaySession {
            device_id: "dev-001".to_string(),
            screen_url: "ws://localhost:9876/relay/dev-001".to_string(),
            control_active: true,
        };
        let s2 = s1.clone();
        assert_eq!(s1, s2);
    }

    #[test]
    fn test_device_serde_roundtrip() {
        let device = PairedDevice {
            id: "dev-serde".to_string(),
            name: "Serde Test".to_string(),
            platform: Platform::Ios,
            paired_at: "2026-11-11T00:00:00Z".to_string(),
            last_seen: "1h".to_string(),
            status: DeviceStatus::Offline,
        };
        let json = serde_json::to_string(&device).expect("serialize");
        let deserialized: PairedDevice = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(device, deserialized);
    }
}