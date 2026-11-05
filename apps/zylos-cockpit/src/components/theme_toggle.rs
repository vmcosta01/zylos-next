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

/// Read initial theme from localStorage or default to Dark
fn load_theme() -> Theme {
    if let Some(window) = web_sys::window() {
        if let Ok(Some(storage)) = window.local_storage() {
            if let Ok(Some(value)) = storage.get_item("zylos-theme") {
                return match value.as_str() {
                    "light" => Theme::Light,
                    _ => Theme::Dark,
                };
            }
        }
    }
    Theme::Dark
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
    let label = move || theme_ctx.theme().get().label().to_string();

    view! {
        <button
            class="theme-toggle"
            on:click=move |_| theme_ctx_click.toggle()
            aria-label="Toggle theme"
            title="Toggle dark/light mode"
        >
            {label}
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
    }
}