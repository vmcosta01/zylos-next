//! Zylos Cockpit — Theme Toggle Component
//!
//! Dark/Light mode switcher using CSS variables per DESIGN_SYSTEM.md.
//! Persists preference to localStorage for cross-session consistency.

use leptos::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Dark,
    Light,
}

impl Theme {
    pub fn css_class(&self) -> &'static str {
        match self {
            Theme::Dark => "theme-dark",
            Theme::Light => "theme-light",
        }
    }

    /// Valor do atributo `data-theme` lido pelo CSS.
    pub fn data_theme(&self) -> &'static str {
        match self {
            Theme::Dark => "dark",
            Theme::Light => "light",
        }
    }

    /// Ícone Phosphor do tema atual (sol no claro, lua no escuro).
    pub fn icon(&self) -> &'static str {
        match self {
            Theme::Dark => "moon",
            Theme::Light => "sun",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Theme::Dark => "🌙",
            Theme::Light => "☀️",
        }
    }

    pub fn toggle(&self) -> Theme {
        match self {
            Theme::Dark => Theme::Light,
            Theme::Light => Theme::Dark,
        }
    }
}

/// Tema do sistema (`prefers-color-scheme`); Claro se não der para perguntar ao navegador.
fn system_theme() -> Theme {
    #[cfg(target_arch = "wasm32")]
    {
        let dark = web_sys::window()
            .and_then(|w| w.match_media("(prefers-color-scheme: dark)").ok().flatten())
            .map(|m| m.matches())
            .unwrap_or(false);
        if dark {
            return Theme::Dark;
        }
    }
    Theme::Light
}

/// Lê o tema salvo; sem valor salvo, segue o sistema (DESIGN_SYSTEM v2.1 §1).
fn load_theme() -> Theme {
    if let Some(window) = web_sys::window() {
        if let Ok(Some(storage)) = window.local_storage() {
            if let Ok(Some(value)) = storage.get_item("zylos-theme") {
                return match value.as_str() {
                    "light" => Theme::Light,
                    "dark" => Theme::Dark,
                    _ => system_theme(),
                };
            }
        }
    }
    system_theme()
}

/// Persist theme to localStorage
fn save_theme(theme: Theme) {
    if let Some(window) = web_sys::window() {
        if let Ok(Some(storage)) = window.local_storage() {
            let value = match theme {
                Theme::Dark => "dark",
                Theme::Light => "light",
            };
            let _ = storage.set_item("zylos-theme", value);
        }
    }
}

/// Apply theme class to document body
fn apply_theme(theme: Theme) {
    if let Some(window) = web_sys::window() {
        if let Some(document) = window.document() {
            if let Some(body) = document.body() {
                // Remove old classes, add new
                let class_list = body.class_list();
                let _ = class_list.remove_1("theme-dark");
                let _ = class_list.remove_1("theme-light");
                let _ = class_list.add_1(theme.css_class());
            }
            // Os tokens de cor leem `data-theme` (design-tokens.css); só a classe no body não bastava.
            if let Some(root) = document.document_element() {
                let _ = root.set_attribute("data-theme", theme.data_theme());
            }
        }
    }
}

/// Global theme context
#[derive(Clone)]
pub struct ThemeContext {
    theme: RwSignal<Theme>,
}

impl ThemeContext {
    pub fn new() -> Self {
        let initial = load_theme();
        apply_theme(initial);
        Self {
            theme: RwSignal::new(initial),
        }
    }

    pub fn theme(&self) -> RwSignal<Theme> {
        self.theme
    }

    pub fn toggle(&self) {
        let next = self.theme.get().toggle();
        self.theme.set(next);
        save_theme(next);
        apply_theme(next);
    }
}

impl Default for ThemeContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Provide theme context to component tree
pub fn provide_theme() -> ThemeContext {
    let ctx = ThemeContext::new();
    provide_context(ctx.clone());
    ctx
}

/// Access theme from any component
pub fn use_theme() -> ThemeContext {
    expect_context::<ThemeContext>()
}

/// ThemeToggle button component
#[component]
pub fn ThemeToggle() -> impl IntoView {
    let theme_ctx = use_theme();
    let theme_ctx_click = theme_ctx.clone();
    let icon_class = move || format!("ph ph-{}", theme_ctx.theme().get().icon());

    view! {
        <button
            class="theme-toggle"
            on:click=move |_| theme_ctx_click.toggle()
            aria-label="Alternar tema claro/escuro"
            title="Alternar tema claro/escuro"
        >
            <i class=icon_class aria-hidden="true"></i>
        </button>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_toggle() {
        assert_eq!(Theme::Dark.toggle(), Theme::Light);
        assert_eq!(Theme::Light.toggle(), Theme::Dark);
    }

    #[test]
    fn test_theme_css_classes() {
        assert_eq!(Theme::Dark.css_class(), "theme-dark");
        assert_eq!(Theme::Light.css_class(), "theme-light");
    }

    #[test]
    fn test_theme_labels() {
        assert_eq!(Theme::Dark.label(), "🌙");
        assert_eq!(Theme::Light.label(), "☀️");
        assert_eq!(Theme::Light.icon(), "sun");
        assert_eq!(Theme::Dark.data_theme(), "dark");
    }
}
