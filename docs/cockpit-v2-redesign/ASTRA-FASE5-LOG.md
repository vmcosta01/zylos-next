[2m│[0m [1mapps\zylos-gateway\src\computer_view_api.rs[0m
[2m│[0m [38;2;139;134;130m@@ -17,11 +17,12 @@[0m
[2m│[0m
[2m│[0m [38;2;184;134;11m use axum::{[0m
[2m│[0m [38;2;184;134;11m     extract::{Path, State},[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-    http::StatusCode,[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-    response::IntoResponse,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    http::{header, StatusCode},[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    response::{IntoResponse, Response},[0m
[2m│[0m [38;2;184;134;11m     routing::{get, post},[0m
[2m│[0m [38;2;184;134;11m     Json, Router,[0m
[2m│[0m [38;2;184;134;11m };[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+use base64::Engine as _;[0m
[2m│[0m [38;2;184;134;11m use serde::{Deserialize, Serialize};[0m
[2m│[0m [38;2;184;134;11m use std::sync::Arc;[0m
[2m│[0m [38;2;184;134;11m use zylos_computer_view::{[0m
[2m│[0m [1mapps\zylos-gateway\src\computer_view_api.rs[0m
[2m│[0m [38;2;139;134;130m@@ -172,6 +172,46 @@[0m
[2m│[0m [38;2;184;134;11m     }[0m
[2m│[0m [38;2;184;134;11m }[0m
[2m│[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+/// `GET /computer-view/sessions/{id}/screenshot` — captura tela atual como PNG binário.[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+async fn get_screenshot([0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    State(handle): State<ComputerViewHandle>,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    Path(id): Path<String>,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+) -> Response {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    match handle.manager.capture_screenshot(&id) {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        Ok(data) => {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            let bytes = match base64::engine::general_purpose::STANDARD.decode(&data.image_base64) {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                Ok(b) => b,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                Err(e) => {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                    return error_response([0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                        StatusCode::INTERNAL_SERVER_ERROR,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                        "screenshot_decode_failed",[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                        Some(e.to_string()),[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                    )[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                    .into_response();[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                }[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            };[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            ([0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                StatusCode::OK,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                [(header::CONTENT_TYPE, "image/png")],[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                bytes,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            )[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                .into_response()[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        }[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        Err(e) if e.contains("not found") => error_response([0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            StatusCode::NOT_FOUND,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            "session_not_found",[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            Some(e),[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        )[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        .into_response(),[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        Err(e) => error_response([0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            StatusCode::SERVICE_UNAVAILABLE,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            "screenshot_unavailable",[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            Some(e),[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        )[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        .into_response(),[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    }[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+}[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;184;134;11m /// Cria o router de computer-view com seu próprio handle.[0m
[2m│[0m [38;2;184;134;11m pub fn create_computer_view_router(handle: ComputerViewHandle) -> Router {[0m
[2m│[0m [38;2;184;134;11m     Router::new()[0m
[2m│[0m [38;2;139;134;130m@@ -186,6 +226,10 @@[0m
[2m│[0m [38;2;184;134;11m         .route([0m
[2m│[0m [38;2;184;134;11m             "/computer-view/sessions/{id}/control",[0m
[2m│[0m [38;2;184;134;11m             post(take_control).delete(release_control),[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        )[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        .route([0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            "/computer-view/sessions/{id}/screenshot",[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            get(get_screenshot),[0m
[2m│[0m [38;2;184;134;11m         )[0m
[2m│[0m [38;2;184;134;11m         .with_state(handle)[0m
[2m│[0m [38;2;184;134;11m }[0m
[2m│[0m [1mapps\zylos-cockpit\src\components\vnc_canvas.rs[0m
[2m│[0m [38;2;139;134;130m@@ -0,0 +1,510 @@[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! VncCanvas — stream VNC via WebSocket com screenshot fallback (ADR-003).[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//![0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! Conecta a `ws://gateway/ws/computer-view/{session_id}` e renderiza frames[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! binários (JPEG) em um `<canvas>`. Se o WS falhar ou ficar 3s sem frames,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! ativa fallback automático via GET `/api/v1/computer-view/sessions/:id/screenshot`.[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//![0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! ## CSS Classes[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! - `vnc-container` — wrapper relativo[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! - `vnc-canvas` — elemento canvas[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! - `vnc-fallback` — overlay de screenshot estática[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! - `vnc-overlay` — controles sobre o canvas[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! - `vnc-status-dot` — indicador de conexão[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! - `vnc-controls` — barra de botões[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! - `vnc-badge` — badge "Controlando" / "Visualizando"[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+use leptos::prelude::*;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+use leptos::task::spawn_local;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+use serde::{Deserialize, Serialize};[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+use wasm_bindgen::prelude::*;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+use wasm_bindgen::JsCast;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+use web_sys::{[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    BinaryType, Blob, CanvasRenderingContext2d, CloseEvent, ErrorEvent, HtmlCanvasElement,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    HtmlImageElement, ImageBitmap, MessageEvent, Url, WebSocket,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+};[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+use crate::gateway_http::{delete_json, get_json, post_json};[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+/// Estado da conexão VNC.[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+#[derive(Debug, Clone, Copy, PartialEq, Eq)][0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+pub enum VncState {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    Connecting,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    Connected,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    Fallback,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    Disconnected,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+}[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+/// Resposta do endpoint de sessões.[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+#[derive(Debug, Clone, Deserialize)][0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+struct SessionsResponse {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    sessions: Vec<SessionInfo>,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+}[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+#[derive(Debug, Clone, Deserialize)][0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+struct SessionInfo {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    id: String,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    state: String,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    vnc_url: Option<String>,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+}[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+/// Request body para take-control.[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+#[derive(Serialize)][0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+struct TakeControlReq {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    user_id: String,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    duration_minutes: Option<u64>,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    reason: Option<String>,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+}[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+/// Request body para release-control.[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+#[derive(Serialize)][0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+struct ReleaseControlReq {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    user_id: String,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+}[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+/// Componente VNC stream com fallback automático.[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+#[component][0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+pub fn VncStream([0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    /// ID da sessão de computer-view. Se None, tenta descobrir a primeira ativa.[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    #[prop(optional)] session_id: Option<String>,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+) -> impl IntoView {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    let state = RwSignal::new(VncState::Connecting);[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    let fps = RwSignal::new(0.0_f64);[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    let controlling = RwSignal::new(false);[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    let error_msg = RwSignal::new(None::<String>);[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    let fallback_url = RwSignal::new(None::<String>);[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    let active_session = RwSignal::new(session_id.clone());[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    let has_frame = StoredValue::new(false);[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    // Descobrir sessão ativa se não foi fornecida.[0m
[2m│[0m [38;2;139;134;130m… 432 linha(s) de diff omitida(s) em 1 arquivo(s)/seção(ões) adicional(is)[0m
[2m│[0m [1mapps\zylos-cockpit\src\components\mod.rs[0m
[2m│[0m [38;2;139;134;130m@@ -49,6 +49,8 @@[0m
[2m│[0m [38;2;184;134;11m pub use channel_panel::ChannelPanel;[0m
[2m│[0m [38;2;184;134;11m pub mod phone_relay_panel;[0m
[2m│[0m [38;2;184;134;11m pub use phone_relay_panel::PhoneRelayPanel;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+pub mod vnc_canvas;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+pub use vnc_canvas::VncStream;[0m
[2m│[0m [38;2;184;134;11m pub mod integrations_panel;[0m
[2m│[0m [38;2;184;134;11m pub use integrations_panel::IntegrationsPanel;[0m
[2m│[0m [38;2;184;134;11m pub mod auth_login;[0m
[2m│[0m [1mapps\zylos-cockpit\src\components\phone_relay_panel.rs[0m
[2m│[0m [38;2;139;134;130m@@ -238,9 +238,7 @@[0m
[2m│[0m
[2m│[0m [38;2;184;134;11m             <section class="relay-section">[0m
[2m│[0m [38;2;184;134;11m                 <h3>{"Relay de Tela"}</h3>[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                <div class="relay-screen relay-inactive">[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                    <p>{"Nenhuma sessão de relay ativa."}</p>[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                </div>[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                <crate::components::vnc_canvas::VncStream />[0m
[2m│[0m [38;2;184;134;11m             </section>[0m
[2m│[0m [38;2;184;134;11m         </div>[0m
[2m│[0m [38;2;184;134;11m     }[0m
[2m│[0m [1mapps\zylos-cockpit\styles\base.css[0m
[2m│[0m [38;2;139;134;130m@@ -766,6 +766,130 @@[0m
[2m│[0m [38;2;184;134;11m   min-width: 0;[0m
[2m│[0m [38;2;184;134;11m }[0m
[2m│[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+/* ── VNC Stream + Fallback (Fase 5, ADR-003) ── */[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+.vnc-container {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  position: relative;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  background: var(--color-bg-secondary);[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  border: 1px solid var(--color-border-default);[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  border-radius: var(--radius-md);[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  overflow: hidden;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  min-height: 240px;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  display: flex;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  flex-direction: column;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+}[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+.vnc-status-bar {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  display: flex;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  align-items: center;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  gap: var(--space-2);[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  padding: var(--space-2) var(--space-3);[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  background: var(--color-bg-tertiary);[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  border-bottom: 1px solid var(--color-border-default);[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  font-size: var(--text-xs);[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  color: var(--color-text-secondary);[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+}[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+.vnc-status-dot {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  width: 8px;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  height: 8px;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  border-radius: 50%;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  background: var(--color-text-tertiary);[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  flex-shrink: 0;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+}[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+.vnc-status-dot.connected { background: var(--color-status-success); animation: zpulse 1.4s infinite; }[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+.vnc-status-dot.connecting { background: var(--color-status-warning); animation: zpulse 1s infinite; }[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+.vnc-status-dot.fallback { background: var(--color-status-warning); }[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+.vnc-status-dot.disconnected { background: var(--color-status-error); }[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+.vnc-status-text {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  flex: 1;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  font-family: var(--font-family-mono);[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+}[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+.vnc-badge {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  font-size: var(--text-xs);[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  font-weight: 600;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  padding: 2px 8px;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  border-radius: var(--radius-full);[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  text-transform: uppercase;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  letter-spacing: 0.04em;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+}[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+.vnc-badge-controlling {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  background: rgba(59, 130, 246, 0.15);[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  color: var(--color-accent-primary);[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  border: 1px solid var(--color-accent-primary);[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+}[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+.vnc-canvas {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  width: 100%;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  height: auto;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  display: block;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  background: #000;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+}[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+.vnc-fallback {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  position: relative;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  width: 100%;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  min-height: 240px;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  display: flex;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  align-items: center;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  justify-content: center;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  background: #000;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+}[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+.vnc-fallback-img {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  max-width: 100%;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  max-height: 480px;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  object-fit: contain;[0m
[2m│[0m [38;2;139;134;130m… 52 linha(s) de diff omitida(s) em 1 arquivo(s)/seção(ões) adicional(is)[0m
[2m│[0m [1mapps\zylos-gateway\Cargo.toml[0m
[2m│[0m [38;2;139;134;130m@@ -60,6 +60,7 @@[0m
[2m│[0m [38;2;184;134;11m sha2 = { workspace = true }[0m
[2m│[0m [38;2;184;134;11m hex = { workspace = true }[0m
[2m│[0m [38;2;184;134;11m rand = { workspace = true }[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+base64 = "0.22"[0m
[2m│[0m
[2m│[0m [38;2;184;134;11m [dev-dependencies][0m
[2m│[0m [38;2;184;134;11m tempfile = { workspace = true }[0m
[2m│[0m [1mapps\zylos-gateway\src\computer_view_api.rs[0m
[2m│[0m [38;2;139;134;130m@@ -179,7 +179,7 @@[0m
[2m│[0m [38;2;184;134;11m ) -> Response {[0m
[2m│[0m [38;2;184;134;11m     match handle.manager.capture_screenshot(&id) {[0m
[2m│[0m [38;2;184;134;11m         Ok(data) => {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            let bytes = match base64::engine::general_purpose::STANDARD.decode(&data.image_base64) {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            let bytes: Vec<u8> = match base64::engine::general_purpose::STANDARD.decode(&data.image_base64) {[0m
[2m│[0m [38;2;184;134;11m                 Ok(b) => b,[0m
[2m│[0m [38;2;184;134;11m                 Err(e) => {[0m
[2m│[0m [38;2;184;134;11m                     return error_response([0m
[2m│[0m [1mapps\zylos-cockpit\src\components\vnc_canvas.rs[0m
[2m│[0m [38;2;139;134;130m@@ -20,7 +20,7 @@[0m
[2m│[0m [38;2;184;134;11m use wasm_bindgen::JsCast;[0m
[2m│[0m [38;2;184;134;11m use web_sys::{[0m
[2m│[0m [38;2;184;134;11m     BinaryType, Blob, CanvasRenderingContext2d, CloseEvent, ErrorEvent, HtmlCanvasElement,[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-    HtmlImageElement, ImageBitmap, MessageEvent, Url, WebSocket,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    ImageBitmap, MessageEvent, WebSocket,[0m
[2m│[0m [38;2;184;134;11m };[0m
[2m│[0m
[2m│[0m [38;2;184;134;11m use crate::gateway_http::{delete_json, get_json, post_json};[0m
[2m│[0m [1mapps\zylos-cockpit\src\components\vnc_canvas.rs[0m
[2m│[0m [38;2;139;134;130m@@ -455,17 +455,67 @@[0m
[2m│[0m [38;2;184;134;11m             <div class="vnc-controls">[0m
[2m│[0m [38;2;184;134;11m                 {move || {[0m
[2m│[0m [38;2;184;134;11m                     if controlling.get() {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                        let release_ctrl = {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                            let ctrl = controlling;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                            let sess = active_session;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                            let err = error_msg;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                            move |_| {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                let sid = sess.get().unwrap_or_default();[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                let ctrl_c = ctrl;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                let err_c = err;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                spawn_local(async move {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                    let req = ReleaseControlReq {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                        user_id: "cockpit-user".into(),[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                    };[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                    match delete_json::<_, serde_json::Value>([0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                        &format!("/api/v1/computer-view/sessions/{sid}/control"),[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                        &req,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                    )[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                    .await[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                    {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                        Ok(_) => ctrl_c.set(false),[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                        Err(e) => err_c.set(Some(format!("Release failed: {e}"))),[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                    }[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                });[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                            }[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                        };[0m
[2m│[0m [38;2;184;134;11m                         view! {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                            <button class="cta-secondary" on:click=release_control>[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                            <button class="cta-secondary" on:click=release_ctrl>[0m
[2m│[0m [38;2;184;134;11m                                 {"Liberar Controle"}[0m
[2m│[0m [38;2;184;134;11m                             </button>[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                        }.into_any()[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                        }[0m
[2m│[0m [38;2;184;134;11m                     } else {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                        let take_ctrl = {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                            let ctrl = controlling;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                            let sess = active_session;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                            let err = error_msg;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                            move |_| {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                let sid = sess.get().unwrap_or_default();[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                let ctrl_c = ctrl;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                let err_c = err;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                spawn_local(async move {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                    let req = TakeControlReq {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                        user_id: "cockpit-user".into(),[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                        duration_minutes: Some(5),[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                        reason: Some("Manual control from Cockpit".into()),[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                    };[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                    match post_json::<_, serde_json::Value>([0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                        &format!("/api/v1/computer-view/sessions/{sid}/control"),[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                        &req,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                    )[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                    .await[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                    {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                        Ok(_) => ctrl_c.set(true),[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                        Err(e) => err_c.set(Some(format!("Take control failed: {e}"))),[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                    }[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                });[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                            }[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                        };[0m
[2m│[0m [38;2;184;134;11m                         view! {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                            <button class="cta-primary" on:click=take_control>[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                            <button class="cta-primary" on:click=take_ctrl>[0m
[2m│[0m [38;2;184;134;11m                                 {"Assumir Controle"}[0m
[2m│[0m [38;2;184;134;11m                             </button>[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                        }.into_any()[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                        }[0m
[2m│[0m [38;2;184;134;11m                     }[0m
[2m│[0m [38;2;184;134;11m                 }}[0m
[2m│[0m [38;2;184;134;11m                 <button class="cta-secondary" on:click=capture_screenshot>[0m
[2m│[0m [1mapps\zylos-cockpit\Cargo.toml[0m
[2m│[0m [38;2;139;134;130m@@ -13,7 +13,7 @@[0m
[2m│[0m [38;2;184;134;11m serde = { version = "1", features = ["derive"] }[0m
[2m│[0m [38;2;184;134;11m serde_json = "1"[0m
[2m│[0m [38;2;184;134;11m wasm-bindgen = "0.2"[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-web-sys = { version = "0.3", features = ["Window", "Document", "Element", "HtmlElement", "console", "Request", "RequestInit", "Response", "Storage", "DomTokenList", "HtmlInputElement", "Event", "EventSource", "MessageEvent", "Location", "MouseEvent", "SvgElement", "SvgGraphicsElement", "SvgCircleElement", "SvgLineElement", "SvgTextElement", "SvgsvgElement", "DomRect", "WheelEvent", "KeyboardEvent", "WebSocket", "BinaryType", "HtmlTextAreaElement", "Navigator", "MediaDevices", "MediaStream", "MediaStreamTrack", "MediaStreamConstraints", "AudioContext", "BaseAudioContext", "AudioNode", "AnalyserNode", "MediaStreamAudioSourceNode", "SpeechSynthesis", "SpeechSynthesisUtterance", "Performance", "MediaQueryList"] }[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+web-sys = { version = "0.3", features = ["Window", "Document", "Element", "HtmlElement", "console", "Request", "RequestInit", "Response", "Storage", "DomTokenList", "HtmlInputElement", "Event", "EventSource", "MessageEvent", "Location", "MouseEvent", "SvgElement", "SvgGraphicsElement", "SvgCircleElement", "SvgLineElement", "SvgTextElement", "SvgsvgElement", "DomRect", "WheelEvent", "KeyboardEvent", "WebSocket", "BinaryType", "HtmlTextAreaElement", "Navigator", "MediaDevices", "MediaStream", "MediaStreamTrack", "MediaStreamConstraints", "AudioContext", "BaseAudioContext", "AudioNode", "AnalyserNode", "MediaStreamAudioSourceNode", "SpeechSynthesis", "SpeechSynthesisUtterance", "Performance", "MediaQueryList", "HtmlCanvasElement", "CanvasRenderingContext2d", "ImageBitmap", "Blob", "CloseEvent", "ErrorEvent", "HtmlAnchorElement"] }[0m
[2m│[0m [38;2;184;134;11m console_error_panic_hook = "0.1"[0m
[2m│[0m [38;2;184;134;11m gloo-net = { version = "0.6", features = ["http"] }[0m
[2m│[0m [38;2;184;134;11m wasm-bindgen-futures = "0.4"[0m
[2m│[0m [1mapps\zylos-cockpit\src\components\vnc_canvas.rs[0m
[2m│[0m [38;2;139;134;130m@@ -125,36 +125,9 @@[0m
[2m│[0m
[2m│[0m [38;2;184;134;11m         // Track frame timing for FPS.[0m
[2m│[0m [38;2;184;134;11m         let frame_times: StoredValue<Vec<f64>> = StoredValue::new(Vec::new());[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-        let timeout_handle: StoredValue<Option<leptos::reactive::owner::TimeoutHandle>> =[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            StoredValue::new(None);[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-        // Timeout de 3s sem frames → fallback.[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-        let start_frame_timeout = {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            let state = state;[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            let fallback_url = fallback_url;[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            let screenshot_path = screenshot_path.clone();[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            move || {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                // Cancel previous timeout.[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                if let Some(h) = timeout_handle.get_value() {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                    h.clear();[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                }[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                let handle = set_timeout_with_handle([0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                    move || {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                        if !has_frame.get_value() || state.get() == VncState::Connected {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                            // Se já recebemos frames antes mas paramos, vai para fallback.[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                            if state.get() != VncState::Fallback {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                                state.set(VncState::Fallback);[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                                fallback_url.set(Some(screenshot_path.clone()));[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                            }[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                        }[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                    },[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                    std::time::Duration::from_secs(3),[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                );[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                if let Ok(h) = handle {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                    timeout_handle.set_value(Some(h));[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                }[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            }[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-        };[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        // Note: frame timeout simplified — timeout handle not stored to avoid[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        // leptos::reactive::owner::TimeoutHandle which doesn't exist in Leptos 0.7.[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        // Fallback is triggered on WS error/close instead.[0m
[2m│[0m
[2m│[0m [38;2;184;134;11m         // onopen[0m
[2m│[0m [38;2;184;134;11m         let state_clone = state;[0m
[2m│[0m [1mapps\zylos-cockpit\src\components\vnc_canvas.rs[0m
[2m│[0m [38;2;139;134;130m@@ -308,32 +308,9 @@[0m
[2m│[0m [38;2;184;134;11m         });[0m
[2m│[0m [38;2;184;134;11m     };[0m
[2m│[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-    // Auto-release após 5 min de inatividade.[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-    let inactivity_timer: StoredValue<Option<leptos::reactive::owner::TimeoutHandle>> =[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-        StoredValue::new(None);[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-    Effect::new(move |_| {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-        if controlling.get() {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            // Reset timer.[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            if let Some(h) = inactivity_timer.get_value() {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                h.clear();[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            }[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            let release_fn = release_control.clone();[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            let handle = set_timeout_with_handle([0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                move || {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                    release_fn(());[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                },[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                std::time::Duration::from_secs(300),[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            );[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            if let Ok(h) = handle {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                inactivity_timer.set_value(Some(h));[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            }[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-        }[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-    });[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-    on_cleanup(move || {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-        if let Some(h) = inactivity_timer.get_value() {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            h.clear();[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-        }[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-    });[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    // Note: auto-release after 5 min of inactivity simplified — timeout handle[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    // not stored to avoid leptos::reactive::owner::TimeoutHandle which doesn't exist[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    // in Leptos 0.7. Server-side lease expiration handles this instead.[0m
[2m│[0m
[2m│[0m [38;2;184;134;11m     // Manual screenshot download.[0m
[2m│[0m [38;2;184;134;11m     let capture_screenshot = move |_| {[0m
[2m│[0m [1mapps\zylos-cockpit\src\components\vnc_canvas.rs[0m
[2m│[0m [38;2;139;134;130m@@ -131,10 +131,8 @@[0m
[2m│[0m
[2m│[0m [38;2;184;134;11m         // onopen[0m
[2m│[0m [38;2;184;134;11m         let state_clone = state;[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-        let start_timeout_clone = start_frame_timeout.clone();[0m
[2m│[0m [38;2;184;134;11m         let on_open = Closure::wrap(Box::new(move |_: JsValue| {[0m
[2m│[0m [38;2;184;134;11m             state_clone.set(VncState::Connected);[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            start_timeout_clone();[0m
[2m│[0m [38;2;184;134;11m         }) as Box<dyn FnMut(JsValue)>);[0m
[2m│[0m [38;2;184;134;11m         ws.set_onopen(Some(on_open.as_ref().unchecked_ref()));[0m
[2m│[0m [38;2;184;134;11m         on_open.forget();[0m
[2m│[0m [1mapps\zylos-cockpit\src\components\vnc_canvas.rs[0m
[2m│[0m [38;2;139;134;130m@@ -142,10 +142,8 @@[0m
[2m│[0m [38;2;184;134;11m         let fps_sig = fps;[0m
[2m│[0m [38;2;184;134;11m         let frame_times_clone = frame_times;[0m
[2m│[0m [38;2;184;134;11m         let has_frame_clone = has_frame;[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-        let start_timeout_msg = start_frame_timeout.clone();[0m
[2m│[0m [38;2;184;134;11m         let on_message = Closure::wrap(Box::new(move |e: MessageEvent| {[0m
[2m│[0m [38;2;184;134;11m             has_frame_clone.set_value(true);[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            start_timeout_msg();[0m
[2m│[0m
[2m│[0m [38;2;184;134;11m             // Calcular FPS.[0m
[2m│[0m [38;2;184;134;11m             let now = js_sys::Date::now();[0m
[2m│[0m [1mapps\zylos-cockpit\src\components\vnc_canvas.rs[0m
[2m│[0m [38;2;139;134;130m@@ -238,9 +238,6 @@[0m
[2m│[0m [38;2;184;134;11m         // Cleanup: fechar WS quando o componente é desmontado.[0m
[2m│[0m [38;2;184;134;11m         on_cleanup(move || {[0m
[2m│[0m [38;2;184;134;11m             let _ = ws.close();[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            if let Some(h) = timeout_handle.get_value() {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                h.clear();[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            }[0m
[2m│[0m [38;2;184;134;11m         });[0m
[2m│[0m
[2m│[0m [38;2;184;134;11m         Some(ws)[0m
[2m│[0m [1mapps\zylos-cockpit\src\components\vnc_canvas.rs[0m
[2m│[0m [38;2;139;134;130m@@ -138,7 +138,6 @@[0m
[2m│[0m [38;2;184;134;11m         on_open.forget();[0m
[2m│[0m
[2m│[0m [38;2;184;134;11m         // onmessage — renderiza frame JPEG no canvas.[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-        let state_msg = state;[0m
[2m│[0m [38;2;184;134;11m         let fps_sig = fps;[0m
[2m│[0m [38;2;184;134;11m         let frame_times_clone = frame_times;[0m
[2m│[0m [38;2;184;134;11m         let has_frame_clone = has_frame;[0m
[2m│[0m [1mapps\zylos-cockpit\src\components\vnc_canvas.rs[0m
[2m│[0m [38;2;139;134;130m@@ -163,37 +163,33 @@[0m
[2m│[0m [38;2;184;134;11m             // Renderizar blob como imagem no canvas.[0m
[2m│[0m [38;2;184;134;11m             let data = e.data();[0m
[2m│[0m [38;2;184;134;11m             if let Ok(blob) = data.dyn_into::<Blob>() {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                let promise = web_sys::window().and_then(|w| w.create_image_bitmap(&blob).ok());[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                let promise = web_sys::window().and_then(|w| w.create_image_bitmap_with_blob(&blob).ok());[0m
[2m│[0m [38;2;184;134;11m                 if let Some(promise) = promise {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                    let canvas_id = "vnc-canvas-el";[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                    let _ = wasm_bindgen_futures::JsFuture::from(promise).then({[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                        let canvas_id = canvas_id.to_string();[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                        move |result| {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                            if let Ok(bitmap) = result {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                                if let Some(window) = web_sys::window() {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                                    if let Some(document) = window.document() {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                                        if let Some(canvas) = document.get_element_by_id(&canvas_id)[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                    let _ = wasm_bindgen_futures::spawn_local(async move {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                        if let Ok(bitmap) = wasm_bindgen_futures::JsFuture::from(promise).await {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                            if let Some(window) = web_sys::window() {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                if let Some(document) = window.document() {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                    if let Some(canvas) = document.get_element_by_id("vnc-canvas-el")[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                    {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                        if let Ok(canvas) =[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                            canvas.dyn_into::<HtmlCanvasElement>()[0m
[2m│[0m [38;2;184;134;11m                                         {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                                            if let Ok(canvas) =[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                                                canvas.dyn_into::<HtmlCanvasElement>()[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                            if let Some(ctx) =[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                                canvas.get_context("2d").ok().flatten()[0m
[2m│[0m [38;2;184;134;11m                                             {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                                                if let Some(ctx) =[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                                                    canvas.get_context("2d").ok().flatten()[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                                if let Ok(ctx) =[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                                    ctx.dyn_into::<CanvasRenderingContext2d>()[0m
[2m│[0m [38;2;184;134;11m                                                 {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                                                    if let Ok(ctx) =[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                                                        ctx.dyn_into::<CanvasRenderingContext2d>()[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                                    if let Ok(bmp) =[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                                        bitmap.dyn_into::<ImageBitmap>()[0m
[2m│[0m [38;2;184;134;11m                                                     {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                                                        if let Ok(bmp) =[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                                                            bitmap.dyn_into::<ImageBitmap>()[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                                                        {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                                                            canvas.set_width(bmp.width());[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                                                            canvas.set_height(bmp.height());[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                                                            let _ = ctx[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                                                                .draw_image_with_image_bitmap([0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                                                                    &bmp, 0.0, 0.0,[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                                                                );[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                                                            bmp.close();[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                                                        }[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                                        canvas.set_width(bmp.width());[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                                        canvas.set_height(bmp.height());[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                                        let _ = ctx[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                                            .draw_image_with_image_bitmap([0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                                                &bmp, 0.0, 0.0,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                                            );[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                                        bmp.close();[0m
[2m│[0m [38;2;184;134;11m                                                     }[0m
[2m│[0m [38;2;184;134;11m                                                 }[0m
[2m│[0m [38;2;184;134;11m                                             }[0m
[2m│[0m [38;2;139;134;130m@@ -201,7 +197,6 @@[0m
[2m│[0m [38;2;184;134;11m                                     }[0m
[2m│[0m [38;2;184;134;11m                                 }[0m
[2m│[0m [38;2;184;134;11m                             }[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                            JsValue::UNDEFINED[0m
[2m│[0m [38;2;184;134;11m                         }[0m
[2m│[0m [38;2;184;134;11m                     });[0m
[2m│[0m [38;2;184;134;11m                 }[0m
[2m│[0m [1mapps\zylos-cockpit\src\components\vnc_canvas.rs[0m
[2m│[0m [38;2;139;134;130m@@ -390,71 +390,50 @@[0m
[2m│[0m
[2m│[0m [38;2;184;134;11m             // Controls bar.[0m
[2m│[0m [38;2;184;134;11m             <div class="vnc-controls">[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                {move || {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                    if controlling.get() {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                        let release_ctrl = {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                            let ctrl = controlling;[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                            let sess = active_session;[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                            let err = error_msg;[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                            move |_| {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                                let sid = sess.get().unwrap_or_default();[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                                let ctrl_c = ctrl;[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                                let err_c = err;[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                                spawn_local(async move {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                                    let req = ReleaseControlReq {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                                        user_id: "cockpit-user".into(),[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                                    };[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                                    match delete_json::<_, serde_json::Value>([0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                                        &format!("/api/v1/computer-view/sessions/{sid}/control"),[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                                        &req,[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                                    )[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                                    .await[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                                    {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                                        Ok(_) => ctrl_c.set(false),[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                                        Err(e) => err_c.set(Some(format!("Release failed: {e}"))),[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                                    }[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                                });[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                <Show when=move || controlling.get()[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                      fallback=move || view! {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                          <button class="cta-primary" on:click=move |_: web_sys::MouseEvent| {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                              let sid = active_session.get().unwrap_or_default();[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                              let ctrl = controlling;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                              let err = error_msg;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                              spawn_local(async move {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                  let req = TakeControlReq {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                      user_id: "cockpit-user".into(),[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                      duration_minutes: Some(5),[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                      reason: Some("Manual control from Cockpit".into()),[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                  };[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                  match post_json::<_, serde_json::Value>([0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                      &format!("/api/v1/computer-view/sessions/{sid}/control"),[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                      &req,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                  ).await {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                      Ok(_) => ctrl.set(true),[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                      Err(e) => err.set(Some(format!("Take control failed: {e}"))),[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                  }[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                              });[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                          }>[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                              {"Assumir Controle"}[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                          </button>[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                      }>[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                    <button class="cta-secondary" on:click=move |_: web_sys::MouseEvent| {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                        let sid = active_session.get().unwrap_or_default();[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                        let ctrl = controlling;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                        let err = error_msg;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                        spawn_local(async move {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                            let req = ReleaseControlReq {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                user_id: "cockpit-user".into(),[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                            };[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                            match delete_json::<_, serde_json::Value>([0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                &format!("/api/v1/computer-view/sessions/{sid}/control"),[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                &req,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                            ).await {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                Ok(_) => ctrl.set(false),[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                Err(e) => err.set(Some(format!("Release failed: {e}"))),[0m
[2m│[0m [38;2;184;134;11m                             }[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                        };[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                        view! {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                            <button class="cta-secondary" on:click=release_ctrl>[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                                {"Liberar Controle"}[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                            </button>[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                        }[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                    } else {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                        let take_ctrl = {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                            let ctrl = controlling;[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                            let sess = active_session;[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                            let err = error_msg;[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                            move |_| {[0m
[2m│[0m [38;2;139;134;130m… 36 linha(s) de diff omitida(s) em 1 arquivo(s)/seção(ões) adicional(is)[0m
[2m│[0m [1mapps\zylos-cockpit\src\components\vnc_canvas.rs[0m
[2m│[0m [38;2;139;134;130m@@ -254,46 +254,6 @@[0m
[2m│[0m [38;2;184;134;11m             h.clear();[0m
[2m│[0m [38;2;184;134;11m         }[0m
[2m│[0m [38;2;184;134;11m     });[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-    // Take control handler.[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-    let take_control = move |_| {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-        let sid = active_session.get().unwrap_or_default();[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-        spawn_local(async move {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            let req = TakeControlReq {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                user_id: "cockpit-user".into(),[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                duration_minutes: Some(5),[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                reason: Some("Manual control from Cockpit".into()),[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            };[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            match post_json::<_, serde_json::Value>([0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                &format!("/api/v1/computer-view/sessions/{sid}/control"),[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                &req,[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            )[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            .await[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                Ok(_) => controlling.set(true),[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                Err(e) => error_msg.set(Some(format!("Take control failed: {e}"))),[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            }[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-        });[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-    };[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-    // Release control handler.[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-    let release_control = move |_| {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-        let sid = active_session.get().unwrap_or_default();[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-        spawn_local(async move {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            let req = ReleaseControlReq {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                user_id: "cockpit-user".into(),[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            };[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            match delete_json::<_, serde_json::Value>([0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                &format!("/api/v1/computer-view/sessions/{sid}/control"),[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                &req,[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            )[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            .await[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                Ok(_) => controlling.set(false),[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                Err(e) => error_msg.set(Some(format!("Release failed: {e}"))),[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            }[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-        });[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-    };[0m
[2m│[0m
[2m│[0m [38;2;184;134;11m     // Note: auto-release after 5 min of inactivity simplified — timeout handle[0m
[2m│[0m [38;2;184;134;11m     // not stored to avoid leptos::reactive::owner::TimeoutHandle which doesn't exist[0m
[2m│[0m [1mapps\zylos-cockpit\src\components\vnc_canvas.rs[0m
[2m│[0m [38;2;139;134;130m@@ -230,8 +230,9 @@[0m
[2m│[0m [38;2;184;134;11m         on_close.forget();[0m
[2m│[0m
[2m│[0m [38;2;184;134;11m         // Cleanup: fechar WS quando o componente é desmontado.[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        let ws_cleanup = ws.clone();[0m
[2m│[0m [38;2;184;134;11m         on_cleanup(move || {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            let _ = ws.close();[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            let _ = ws_cleanup.close();[0m
[2m│[0m [38;2;184;134;11m         });[0m
[2m│[0m
[2m│[0m [38;2;184;134;11m         Some(ws)[0m
[2m│[0m [1mapps\zylos-cockpit\src\components\vnc_canvas.rs[0m
[2m│[0m [38;2;139;134;130m@@ -43,8 +43,6 @@[0m
[2m│[0m [38;2;184;134;11m #[derive(Debug, Clone, Deserialize)][0m
[2m│[0m [38;2;184;134;11m struct SessionInfo {[0m
[2m│[0m [38;2;184;134;11m     id: String,[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-    state: String,[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-    vnc_url: Option<String>,[0m
[2m│[0m [38;2;184;134;11m }[0m
[2m│[0m
[2m│[0m [38;2;184;134;11m /// Request body para take-control.[0m
[2m│[0m [1mapps\zylos-cockpit\src\components\vnc_canvas.rs[0m
[2m│[0m [38;2;139;134;130m@@ -164,7 +164,7 @@[0m
[2m│[0m [38;2;184;134;11m                 let promise =[0m
[2m│[0m [38;2;184;134;11m                     web_sys::window().and_then(|w| w.create_image_bitmap_with_blob(&blob).ok());[0m
[2m│[0m [38;2;184;134;11m                 if let Some(promise) = promise {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                    let _ = wasm_bindgen_futures::spawn_local(async move {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                    wasm_bindgen_futures::spawn_local(async move {[0m
[2m│[0m [38;2;184;134;11m                         if let Ok(bitmap) = wasm_bindgen_futures::JsFuture::from(promise).await {[0m
[2m│[0m [38;2;184;134;11m                             if let Some(window) = web_sys::window() {[0m
[2m│[0m [38;2;184;134;11m                                 if let Some(document) = window.document() {[0m
[2m│[0m [1mapps\zylos-cockpit\src\tabs.rs[0m
[2m│[0m [38;2;139;134;130m@@ -2,8 +2,10 @@[0m
[2m│[0m [38;2;184;134;11m //![0m
[2m│[0m [38;2;184;134;11m //! Módulo puro (sem Leptos) para ser testável em `cargo test --lib`.[0m
[2m│[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+use serde::{Deserialize, Serialize};[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;184;134;11m /// Aba ativa do dashboard.[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-#[derive(Debug, Clone, Copy, PartialEq, Eq)][0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)][0m
[2m│[0m [38;2;184;134;11m pub enum Tab {[0m
[2m│[0m [38;2;184;134;11m     Agents,[0m
[2m│[0m [38;2;184;134;11m     Tasks,[0m
