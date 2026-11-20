//! SkinThemes — sistema de temas visuais com 5+ skins para o cockpit
//!
//! Inspirado em: OpenMausBot (theme switcher), Paperclip (design tokens).
//! Permite trocar entre temas pré-definidos que alteram cores, fontes e espaçamento
//! via CSS custom properties injetadas dinamicamente.
//!
//! ## Temas Disponíveis
//! - Default (dark azul/roxo)
//! - Ocean (azul profundo/teal)
//! - Forest (verde/esmeralda)
//! - Sunset (laranja/âmbar)
//! - Rose (rosa/fúcsia)
//! - Monochrome (cinza neutro)
//! - Cyberpunk (neon verde/preto)
//!
//! ## Related Documents
//! - Implementation Plan: [IMPLEMENTATION_PLAN_DETAILED.md](../../docs/IMPLEMENTATION_PLAN_DETAILED.md) §5.1 Task 4.2

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

/// Definição de um tema/skin
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkinTheme {
    pub id: String,
    pub name: String,
    pub description: String,
    pub preview_color: String,
    pub css_variables: Vec<(String, String)>,
}

impl SkinTheme {
    /// Aplica o tema ao document.documentElement via CSS custom properties
    pub fn apply(&self) {
        if let Some(window) = web_sys::window() {
            if let Some(document) = window.document() {
                if let Some(root) = document.document_element() {
                    let style_value: String = self
                        .css_variables
                        .iter()
                        .map(|(var_name, value)| format!("--{}: {};", var_name, value))
                        .collect();
                    let _ = root.set_attribute("style", &style_value);
                }
            }
        }
    }
}

/// Retorna todos os temas disponíveis
pub fn available_themes() -> Vec<SkinTheme> {
    vec![
        SkinTheme {
            id: "default".into(),
            name: "Default".into(),
            description: "Tema padrão escuro com acentos azul/roxo".into(),
            preview_color: "#6366F1".into(),
            css_variables: vec![
                ("foreground".into(), "#E2E8F0".into()),
                ("background".into(), "#0F172A".into()),
                ("card".into(), "#1E293B".into()),
                ("accent".into(), "#6366F1".into()),
                ("muted-foreground".into(), "#94A3B8".into()),
                ("border".into(), "#334155".into()),
                ("destructive".into(), "#EF4444".into()),
                ("success".into(), "#10B981".into()),
            ],
        },
        SkinTheme {
            id: "ocean".into(),
            name: "Ocean".into(),
            description: "Azul profundo com acentos teal".into(),
            preview_color: "#0EA5E9".into(),
            css_variables: vec![
                ("foreground".into(), "#E0F2FE".into()),
                ("background".into(), "#0C1929".into()),
                ("card".into(), "#162032".into()),
                ("accent".into(), "#0EA5E9".into()),
                ("muted-foreground".into(), "#7DD3FC".into()),
                ("border".into(), "#1E3A5F".into()),
                ("destructive".into(), "#F43F5E".into()),
                ("success".into(), "#14B8A6".into()),
            ],
        },
        SkinTheme {
            id: "forest".into(),
            name: "Forest".into(),
            description: "Verde esmeralda com tons naturais".into(),
            preview_color: "#10B981".into(),
            css_variables: vec![
                ("foreground".into(), "#ECFDF5".into()),
                ("background".into(), "#052E16".into()),
                ("card".into(), "#064E3B".into()),
                ("accent".into(), "#10B981".into()),
                ("muted-foreground".into(), "#6EE7B7".into()),
                ("border".into(), "#065F46".into()),
                ("destructive".into(), "#DC2626".into()),
                ("success".into(), "#22C55E".into()),
            ],
        },
        SkinTheme {
            id: "sunset".into(),
            name: "Sunset".into(),
            description: "Laranja quente com acentos âmbar".into(),
            preview_color: "#F59E0B".into(),
            css_variables: vec![
                ("foreground".into(), "#FFF7ED".into()),
                ("background".into(), "#1C0F00".into()),
                ("card".into(), "#2D1B00".into()),
                ("accent".into(), "#F59E0B".into()),
                ("muted-foreground".into(), "#FCD34D".into()),
                ("border".into(), "#451A03".into()),
                ("destructive".into(), "#EF4444".into()),
                ("success".into(), "#84CC16".into()),
            ],
        },
        SkinTheme {
            id: "rose".into(),
            name: "Rose".into(),
            description: "Rosa fúcsia com toques suaves".into(),
            preview_color: "#EC4899".into(),
            css_variables: vec![
                ("foreground".into(), "#FDF2F8".into()),
                ("background".into(), "#1A0011".into()),
                ("card".into(), "#2D0020".into()),
                ("accent".into(), "#EC4899".into()),
                ("muted-foreground".into(), "#F9A8D4".into()),
                ("border".into(), "#4A0033".into()),
                ("destructive".into(), "#F43F5E".into()),
                ("success".into(), "#10B981".into()),
            ],
        },
        SkinTheme {
            id: "monochrome".into(),
            name: "Monochrome".into(),
            description: "Cinza neutro minimalista".into(),
            preview_color: "#A3A3A3".into(),
            css_variables: vec![
                ("foreground".into(), "#F5F5F5".into()),
                ("background".into(), "#171717".into()),
                ("card".into(), "#262626".into()),
                ("accent".into(), "#A3A3A3".into()),
                ("muted-foreground".into(), "#737373".into()),
                ("border".into(), "#404040".into()),
                ("destructive".into(), "#DC2626".into()),
                ("success".into(), "#65A30D".into()),
            ],
        },
        SkinTheme {
            id: "cyberpunk".into(),
            name: "Cyberpunk".into(),
            description: "Neon verde sobre preto intenso".into(),
            preview_color: "#00FF41".into(),
            css_variables: vec![
                ("foreground".into(), "#00FF41".into()),
                ("background".into(), "#000000".into()),
                ("card".into(), "#0A0A0A".into()),
                ("accent".into(), "#00FF41".into()),
                ("muted-foreground".into(), "#008F11".into()),
                ("border".into(), "#003B00".into()),
                ("destructive".into(), "#FF0040".into()),
                ("success".into(), "#00FF41".into()),
            ],
        },
    ]
}

#[component]
pub fn SkinThemesPanel() -> impl IntoView {
    let themes = available_themes();
    let active_theme_id = RwSignal::new("default".to_string());

    // Carregar tema salvo do localStorage ao montar
    Effect::new(move |_| {
        if let Some(window) = web_sys::window() {
            if let Some(storage) = window.local_storage().ok().flatten() {
                if let Ok(Some(saved)) = storage.get_item("zylos-skin-theme") {
                    active_theme_id.set(saved);
                }
            }
        }
    });

    let apply_theme = move |theme: &SkinTheme| {
        theme.apply();
        active_theme_id.set(theme.id.clone());

        // Persistir escolha no localStorage
        if let Some(window) = web_sys::window() {
            if let Some(storage) = window.local_storage().ok().flatten() {
                let _ = storage.set_item("zylos-skin-theme", &theme.id);
            }
        }
    };

    view! {
    <div class="skin-themes-panel">
    <div class="skin-themes-header">
    <h3 class="skin-themes-title"><i class="ph ph-palette" aria-hidden="true"></i>" Temas Visuais"</h3>
    <span class="skin-themes-subtitle">"Escolha um tema para personalizar o cockpit"</span>
    </div>

    <div class="skin-themes-grid">
    <For
    each=move || themes.clone()
    key=|t| t.id.clone()
    children=move |theme| {
    let tid = theme.id.clone();
    let tname = theme.name.clone();
    let tdesc = theme.description.clone();
    let tcolor = theme.preview_color.clone();
    let active_id = active_theme_id;
    let is_active = move || active_id.get() == tid;

    let card_css = if is_active() {
    "skin-card skin-card-active".to_string()
    } else {
    "skin-card".to_string()
    };

    let theme_clone = theme.clone();

    view! {
    <div
    class=card_css
    on:click=move |_| apply_theme(&theme_clone)
    >
    <div class="skin-preview" style=format!("background-color: {}", tcolor)></div>
    <div class="skin-info">
    <span class="skin-name">{tname.clone()}</span>
    <span class="skin-desc">{tdesc.clone()}</span>
    </div>
    {move || {
    if is_active() {
    view! { <span class="skin-active-badge"><i class="ph ph-check" aria-hidden="true"></i>" Ativo"</span> }.into_any()
    } else {
    view! { <div></div> }.into_any()
    }
    }}
    </div>
    }
    }
    />
    </div>
    </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_available_themes_count() {
        let themes = available_themes();
        assert!(themes.len() >= 5, "Must have at least 5 themes");
        assert_eq!(themes.len(), 7, "Currently 7 themes defined");
    }

    #[test]
    fn test_all_themes_have_unique_ids() {
        let themes = available_themes();
        let ids: Vec<&str> = themes.iter().map(|t| t.id.as_str()).collect();
        let unique: std::collections::HashSet<&str> = ids.iter().copied().collect();
        assert_eq!(ids.len(), unique.len(), "All theme IDs must be unique");
    }

    #[test]
    fn test_all_themes_have_required_variables() {
        let required_vars = [
            "foreground",
            "background",
            "card",
            "accent",
            "muted-foreground",
            "border",
        ];
        for theme in available_themes() {
            for var in &required_vars {
                assert!(
                    theme.css_variables.iter().any(|(k, _)| k == *var),
                    "Theme '{}' missing required CSS variable '{}'",
                    theme.id,
                    var
                );
            }
        }
    }

    #[test]
    fn test_all_themes_have_preview_color() {
        for theme in available_themes() {
            assert!(
                theme.preview_color.starts_with('#'),
                "Theme '{}' preview_color must be hex",
                theme.id
            );
            assert!(
                theme.preview_color.len() == 7 || theme.preview_color.len() == 4,
                "Theme '{}' preview_color must be valid hex (#RGB or #RRGGBB)",
                theme.id
            );
        }
    }

    #[test]
    fn test_theme_serialization() {
        let theme = available_themes()[0].clone();
        let json = serde_json::to_string(&theme).unwrap();
        assert!(json.contains("default"));
        assert!(json.contains("foreground"));
    }
}
