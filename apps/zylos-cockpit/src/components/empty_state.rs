//! Zylos Cockpit — EmptyState Component (Fase 7)
//!
//! Reusable empty state with inline SVG illustration, title, description,
//! and optional CTA button per DESIGN_SYSTEM.md §6 states.

use leptos::prelude::*;

/// Inline SVG icons for empty states
pub mod icons {
    pub const ROBOT: &str = r#"<svg width="64" height="64" viewBox="0 0 64 64" fill="none" xmlns="http://www.w3.org/2000/svg"><rect x="16" y="20" width="32" height="28" rx="4" stroke="currentColor" stroke-width="2"/><circle cx="26" cy="32" r="3" fill="currentColor"/><circle cx="38" cy="32" r="3" fill="currentColor"/><path d="M26 42h12" stroke="currentColor" stroke-width="2" stroke-linecap="round"/><path d="M32 12v8" stroke="currentColor" stroke-width="2" stroke-linecap="round"/><circle cx="32" cy="10" r="3" stroke="currentColor" stroke-width="2"/><path d="M12 30h4M48 30h4" stroke="currentColor" stroke-width="2" stroke-linecap="round"/><path d="M20 48v6M44 48v6" stroke="currentColor" stroke-width="2" stroke-linecap="round"/></svg>"#;

    pub const DOCUMENT: &str = r#"<svg width="64" height="64" viewBox="0 0 64 64" fill="none" xmlns="http://www.w3.org/2000/svg"><path d="M18 8h20l12 12v36a4 4 0 01-4 4H18a4 4 0 01-4-4V12a4 4 0 014-4z" stroke="currentColor" stroke-width="2"/><path d="M38 8v12h12" stroke="currentColor" stroke-width="2"/><path d="M24 32h16M24 40h10" stroke="currentColor" stroke-width="2" stroke-linecap="round"/></svg>"#;

    pub const CHART: &str = r#"<svg width="64" height="64" viewBox="0 0 64 64" fill="none" xmlns="http://www.w3.org/2000/svg"><rect x="8" y="36" width="10" height="20" rx="2" stroke="currentColor" stroke-width="2"/><rect x="22" y="24" width="10" height="32" rx="2" stroke="currentColor" stroke-width="2"/><rect x="36" y="16" width="10" height="40" rx="2" stroke="currentColor" stroke-width="2"/><rect x="50" y="28" width="10" height="28" rx="2" stroke="currentColor" stroke-width="2"/><path d="M8 60h52" stroke="currentColor" stroke-width="2" stroke-linecap="round"/></svg>"#;

    pub const SEARCH: &str = r#"<svg width="64" height="64" viewBox="0 0 64 64" fill="none" xmlns="http://www.w3.org/2000/svg"><circle cx="28" cy="28" r="16" stroke="currentColor" stroke-width="2"/><path d="M40 40l14 14" stroke="currentColor" stroke-width="2" stroke-linecap="round"/><path d="M22 28h12M28 22v12" stroke="currentColor" stroke-width="2" stroke-linecap="round"/></svg>"#;
}

/// EmptyState component props
#[component]
pub fn EmptyState(
    /// SVG markup string (use icons::ROBOT, icons::DOCUMENT, etc.)
    #[prop(default = icons::ROBOT)]
    icon: &'static str,
    /// Title text
    title: String,
    /// Description text
    #[prop(default = String::new())]
    description: String,
    /// Optional CTA button label
    #[prop(optional)]
    cta_label: Option<String>,
    /// Optional CTA click handler
    #[prop(optional)]
    on_cta: Option<Callback<()>>,
) -> impl IntoView {
    let has_cta = cta_label.is_some();
    let label = cta_label.unwrap_or_default();

    view! {
        <div class="empty-state-v2">
            <div class="empty-state-icon" inner_html=icon></div>
            <h3 class="empty-state-title">{title}</h3>
            {if !description.is_empty() {
                view! { <p class="empty-state-description">{description}</p> }.into_any()
            } else {
                view! { <span></span> }.into_any()
            }}
            {if has_cta {
                let cb = on_cta;
                view! {
                    <button
                        class="cta-primary empty-state-cta"
                        on:click=move |_| { if let Some(ref c) = cb { c.run(()); } }
                    >{label.clone()}</button>
                }.into_any()
            } else {
                view! { <span></span> }.into_any()
            }}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_icons_are_valid_svg() {
        assert!(icons::ROBOT.contains("<svg"));
        assert!(icons::DOCUMENT.contains("<svg"));
        assert!(icons::CHART.contains("<svg"));
        assert!(icons::SEARCH.contains("<svg"));
    }

    #[test]
    fn test_icons_close_properly() {
        assert!(icons::ROBOT.contains("</svg>"));
        assert!(icons::DOCUMENT.contains("</svg>"));
        assert!(icons::CHART.contains("</svg>"));
        assert!(icons::SEARCH.contains("</svg>"));
    }
}