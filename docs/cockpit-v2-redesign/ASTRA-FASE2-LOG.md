[2m│[0m [1mapps\zylos-cockpit\src\components\translucent_header.rs[0m
[2m│[0m [38;2;139;134;130m@@ -0,0 +1,90 @@[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! TranslucentHeader — Header translúcido com backdrop-filter (ADR-010).[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//![0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! Especificação: DESIGN_SYSTEM.md §7 (Translucent Materials), §10.3 (TranslucentHeader).[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! Estados obrigatórios: APP_FLOW.md §2 (Layout Shell).[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//![0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! - backdrop-filter: blur(20px) saturate(180%) via var(--blur-header)[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! - background: var(--color-bg-translucent)[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! - sticky top, z-index var(--z-header)[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! - fallback opaco quando prefers-reduced-transparency: reduce (CSS)[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! - feature detection JS: .no-backdrop → fundo opaco[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+use leptos::prelude::*;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+use crate::components::{AuthHeaderButton, ThemeToggle};[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+use crate::streaming::StreamContext;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+/// Status do gateway derivado do StreamContext.[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+#[derive(Debug, Clone, Copy, PartialEq, Eq)][0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+pub enum GatewayStatus {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    Healthy,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    Degraded,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    Disconnected,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+}[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+impl GatewayStatus {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    pub fn css_class(self) -> &'static str {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        match self {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            Self::Healthy => "status-dot connected",[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            Self::Degraded => "status-dot warning",[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            Self::Disconnected => "status-dot error",[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        }[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    }[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    pub fn label(self) -> &'static str {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        match self {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            Self::Healthy => "Online",[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            Self::Degraded => "Degradado",[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            Self::Disconnected => "Offline",[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        }[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    }[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+}[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+/// Header translúcido do Cockpit v2.0.[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+///[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+/// Props:[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+/// - `version`: versão do gateway (ex: "2.0.0")[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+/// - `gateway_status`: status atual da conexão SSE[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+#[component][0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+pub fn TranslucentHeader([0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    version: String,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    gateway_status: GatewayStatus,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+) -> impl IntoView {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    // Feature detection: adiciona .no-backdrop se backdrop-filter não suportado.[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    Effect::new(move |_| {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        if let Some(window) = web_sys::window() {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            if let Ok(style) = window.get_computed_style(&{[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                let el = window.document().and_then(|d| d.body()).unwrap_or_default();[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                el.into()[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            }) {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                if let Some(s) = style {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                    let bf = s.get_property_value("backdrop-filter").unwrap_or_default();[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                    if bf.is_empty() || bf == "none" {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                        if let Some(doc) = window.document() {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                            if let Some(body) = doc.body() {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                let _ = body.class_list().add_1("no-backdrop");[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                            }[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                        }[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                    }[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                }[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            }[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        }[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    });[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    view! {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        <header class="translucent-header material-translucent" role="banner">[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            <div class="header-left">[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                <h1 class="header-title">"Zylos Cockpit"</h1>[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                <span class="header-status" aria-live="polite">[0m
[2m│[0m [38;2;139;134;130m… 12 linha(s) de diff omitida(s) em 1 arquivo(s)/seção(ões) adicional(is)[0m
