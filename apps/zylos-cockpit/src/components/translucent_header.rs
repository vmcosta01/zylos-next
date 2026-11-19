//! TranslucentHeader — Header translúcido com backdrop-filter (ADR-010).
//!
//! Especificação: DESIGN_SYSTEM.md §7 (Translucent Materials), §10.3 (TranslucentHeader).
//! Estados obrigatórios: APP_FLOW.md §2 (Layout Shell).
//!
//! - backdrop-filter: blur(20px) saturate(180%) via var(--blur-header)
//! - background: var(--color-bg-translucent)
//! - sticky top, z-index var(--z-header)
//! - fallback opaco quando prefers-reduced-transparency: reduce (CSS)
//! - feature detection JS: .no-backdrop → fundo opaco

use leptos::prelude::*;
use wasm_bindgen::JsCast;

#[allow(unused_imports)]
use crate::components::{AuthHeaderButton, ThemeToggle};

/// Status do gateway derivado do StreamContext.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GatewayStatus {
    Healthy,
    Degraded,
    Disconnected,
}

impl GatewayStatus {
    pub fn css_class(self) -> &'static str {
        match self {
            Self::Healthy => "status-dot connected",
            Self::Degraded => "status-dot warning",
            Self::Disconnected => "status-dot error",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Healthy => "Online",
            Self::Degraded => "Degradado",
            Self::Disconnected => "Offline",
        }
    }
}

/// Header translúcido do Cockpit v2.0.
///
/// Props:
/// - `version`: versão do gateway (ex: "2.0.0")
/// - `gateway_status`: status atual da conexão SSE
#[component]
pub fn TranslucentHeader(version: String, gateway_status: GatewayStatus) -> impl IntoView {
    // Feature detection: cria elemento de teste para verificar se backdrop-filter é suportado.
    Effect::new(move |_| {
        if let Some(window) = web_sys::window() {
            if let Some(doc) = window.document() {
                if let Some(body) = doc.body() {
                    // Criar elemento de teste temporário e converter para HtmlElement
                    if let Ok(test_el) = doc.create_element("div") {
                        if let Ok(html_el) = test_el.dyn_into::<web_sys::HtmlElement>() {
                            let style = html_el.style();
                            // Tentar definir backdrop-filter
                            let _ = style.set_property("backdrop-filter", "blur(1px)");
                            // Verificar se o browser aceitou a propriedade
                            let computed_bf = style.get_property_value("backdrop-filter").unwrap_or_default();
                            let webkit_bf = style.get_property_value("-webkit-backdrop-filter").unwrap_or_default();
                            
                            // Se ambas estão vazias ou "none", browser não suporta
                            if (computed_bf.is_empty() || computed_bf == "none") 
                                && (webkit_bf.is_empty() || webkit_bf == "none") {
                                let _ = body.class_list().add_1("no-backdrop");
                            }
                        }
                    }
                }
            }
        }
    });

    view! {
        <header class="translucent-header material-translucent" role="banner">
            <div class="header-left">
                <h1 class="header-title">"Zylos Cockpit"</h1>
                <span class="header-status" aria-live="polite">
                    <span class=gateway_status.css_class()></span>
                    <span class="status-label">{gateway_status.label()}</span>
                </span>
            </div>
            <div class="header-right">
                <span class="version-badge">{version}</span>
                <ThemeToggle />
                <AuthHeaderButton />
            </div>
        </header>
    }
}
