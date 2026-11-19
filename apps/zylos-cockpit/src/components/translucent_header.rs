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
    // Feature detection via CSS.supports() — evita problemas de tipo web-sys.
    Effect::new(move |_| {
        let supports = js_sys::eval(
            "typeof CSS !== 'undefined' && typeof CSS.supports === 'function' && (CSS.supports('backdrop-filter', 'blur(1px)') || CSS.supports('-webkit-backdrop-filter', 'blur(1px)'))"
        ).ok().and_then(|v| v.as_bool()).unwrap_or(false);
        
        if !supports {
            if let Some(window) = web_sys::window() {
                if let Some(doc) = window.document() {
                    if let Some(body) = doc.body() {
                        let _ = body.class_list().add_1("no-backdrop");
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
