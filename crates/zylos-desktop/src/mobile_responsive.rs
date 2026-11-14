//! # MobileResponsive — Responsive layout utilities for cockpit UI (T8.6.4)
//!
//! Provides breakpoint definitions, responsive CSS generation, and viewport
//! state tracking for the Leptos WASM cockpit. Ensures the dashboard is
//! usable on mobile phones, tablets, and desktops without separate builds.
//!
//! ## Breakpoints
//! - **Mobile**: < 640px (single column, stacked panels)
//! - **Tablet**: 640–1024px (2-column adaptive)
//! - **Desktop**: > 1024px (full sidebar + multi-panel)
//!
//! ## Integration
//! The cockpit WASM reads `ViewportState` via IPC on resize events.
//! CSS media queries are injected into the webview stylesheet at startup.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Standard responsive breakpoints in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Breakpoint {
    /// Mobile: single column, stacked layout (< 640px).
    Mobile,
    /// Tablet: 2-column adaptive layout (640–1024px).
    Tablet,
    /// Desktop: full sidebar + multi-panel (> 1024px).
    #[default]
    Desktop,
}

impl Breakpoint {
    /// Minimum width in pixels for this breakpoint.
    pub fn min_width_px(self) -> u32 {
        match self {
            Self::Mobile => 0,
            Self::Tablet => 640,
            Self::Desktop => 1024,
        }
    }

    /// Maximum width in pixels for this breakpoint (exclusive).
    /// Returns `None` for Desktop (no upper bound).
    pub fn max_width_px(self) -> Option<u32> {
        match self {
            Self::Mobile => Some(639),
            Self::Tablet => Some(1023),
            Self::Desktop => None,
        }
    }

    /// Determine breakpoint from a given viewport width.
    pub fn from_width(width: u32) -> Self {
        if width < 640 {
            Self::Mobile
        } else if width < 1024 {
            Self::Tablet
        } else {
            Self::Desktop
        }
    }

    /// CSS media query string for this breakpoint.
    pub fn media_query(self) -> String {
        match self {
            Self::Mobile => "(max-width: 639px)".into(),
            Self::Tablet => "(min-width: 640px) and (max-width: 1023px)".into(),
            Self::Desktop => "(min-width: 1024px)".into(),
        }
    }

    /// All breakpoints in ascending order.
    pub fn all() -> &'static [Breakpoint] {
        &[Breakpoint::Mobile, Breakpoint::Tablet, Breakpoint::Desktop]
    }
}

impl fmt::Display for Breakpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mobile => write!(f, "mobile"),
            Self::Tablet => write!(f, "tablet"),
            Self::Desktop => write!(f, "desktop"),
        }
    }
}

/// Current viewport state tracked by the desktop shell.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ViewportState {
    /// Current viewport width in pixels.
    pub width: u32,
    /// Current viewport height in pixels.
    pub height: u32,
    /// Active breakpoint derived from width.
    pub breakpoint: Breakpoint,
    /// Device pixel ratio (e.g., 2.0 for Retina).
    pub dpr: f64,
    /// Whether the device is in portrait orientation.
    pub is_portrait: bool,
    /// Safe area insets for notched devices (top, right, bottom, left).
    pub safe_area: SafeAreaInsets,
}

impl Default for ViewportState {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 860,
            breakpoint: Breakpoint::Desktop,
            dpr: 1.0,
            is_portrait: false,
            safe_area: SafeAreaInsets::default(),
        }
    }
}

impl ViewportState {
    /// Create from dimensions, auto-detecting breakpoint and orientation.
    pub fn from_dimensions(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            breakpoint: Breakpoint::from_width(width),
            dpr: 1.0,
            is_portrait: height > width,
            safe_area: SafeAreaInsets::default(),
        }
    }

    /// Create with full parameters including DPR and safe area.
    pub fn new(width: u32, height: u32, dpr: f64, safe_area: SafeAreaInsets) -> Self {
        Self {
            width,
            height,
            breakpoint: Breakpoint::from_width(width),
            dpr,
            is_portrait: height > width,
            safe_area,
        }
    }

    /// Check if the viewport is considered mobile-sized.
    pub fn is_mobile(&self) -> bool {
        self.breakpoint == Breakpoint::Mobile
    }

    /// Check if the viewport is tablet-sized.
    pub fn is_tablet(&self) -> bool {
        self.breakpoint == Breakpoint::Tablet
    }

    /// Check if the viewport is desktop-sized.
    pub fn is_desktop(&self) -> bool {
        self.breakpoint == Breakpoint::Desktop
    }

    /// Logical width adjusted for DPR.
    pub fn logical_width(&self) -> f64 {
        self.width as f64 / self.dpr
    }

    /// Logical height adjusted for DPR.
    pub fn logical_height(&self) -> f64 {
        self.height as f64 / self.dpr
    }

    /// Usable width after subtracting safe area insets.
    pub fn usable_width(&self) -> u32 {
        self.width
            .saturating_sub(self.safe_area.left)
            .saturating_sub(self.safe_area.right)
    }

    /// Usable height after subtracting safe area insets.
    pub fn usable_height(&self) -> u32 {
        self.height
            .saturating_sub(self.safe_area.top)
            .saturating_sub(self.safe_area.bottom)
    }
}

/// Safe area insets for notched/rounded-corner devices.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub struct SafeAreaInsets {
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
    pub left: u32,
}

impl SafeAreaInsets {
    pub fn new(top: u32, right: u32, bottom: u32, left: u32) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    /// Total horizontal inset.
    pub fn horizontal(&self) -> u32 {
        self.left + self.right
    }

    /// Total vertical inset.
    pub fn vertical(&self) -> u32 {
        self.top + self.bottom
    }
}

/// Responsive layout configuration that adapts per breakpoint.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResponsiveLayout {
    /// Sidebar visibility per breakpoint.
    pub sidebar_visible: BreakpointConfig<bool>,
    /// Number of grid columns per breakpoint.
    pub grid_columns: BreakpointConfig<u32>,
    /// Panel stacking mode per breakpoint.
    pub stack_panels: BreakpointConfig<bool>,
    /// Font size scale factor per breakpoint.
    pub font_scale: BreakpointConfig<f64>,
    /// Touch target minimum size in px per breakpoint.
    pub touch_target_min: BreakpointConfig<u32>,
}

impl Default for ResponsiveLayout {
    fn default() -> Self {
        Self {
            sidebar_visible: BreakpointConfig {
                mobile: false,
                tablet: true,
                desktop: true,
            },
            grid_columns: BreakpointConfig {
                mobile: 1,
                tablet: 2,
                desktop: 3,
            },
            stack_panels: BreakpointConfig {
                mobile: true,
                tablet: false,
                desktop: false,
            },
            font_scale: BreakpointConfig {
                mobile: 0.9,
                tablet: 1.0,
                desktop: 1.0,
            },
            touch_target_min: BreakpointConfig {
                mobile: 44,
                tablet: 40,
                desktop: 32,
            },
        }
    }
}

impl ResponsiveLayout {
    /// Get the effective value for a given breakpoint.
    pub fn for_breakpoint<'a, T>(&self, config: &'a BreakpointConfig<T>, bp: Breakpoint) -> &'a T {
        match bp {
            Breakpoint::Mobile => &config.mobile,
            Breakpoint::Tablet => &config.tablet,
            Breakpoint::Desktop => &config.desktop,
        }
    }

    /// Generate CSS custom properties for the current viewport.
    pub fn css_variables(&self, viewport: &ViewportState) -> String {
        let bp = viewport.breakpoint;
        format!(
            "--sidebar-visible: {};\n\
             --grid-columns: {};\n\
             --stack-panels: {};\n\
             --font-scale: {};\n\
             --touch-target-min: {}px;\n\
             --safe-area-top: {}px;\n\
             --safe-area-bottom: {}px;\n\
             --safe-area-left: {}px;\n\
             --safe-area-right: {}px;",
            if *self.for_breakpoint(&self.sidebar_visible, bp) {
                "1"
            } else {
                "0"
            },
            self.for_breakpoint(&self.grid_columns, bp),
            if *self.for_breakpoint(&self.stack_panels, bp) {
                "1"
            } else {
                "0"
            },
            self.for_breakpoint(&self.font_scale, bp),
            self.for_breakpoint(&self.touch_target_min, bp),
            viewport.safe_area.top,
            viewport.safe_area.bottom,
            viewport.safe_area.left,
            viewport.safe_area.right,
        )
    }
}

/// Per-breakpoint configuration value.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BreakpointConfig<T> {
    pub mobile: T,
    pub tablet: T,
    pub desktop: T,
}

impl<T> BreakpointConfig<T> {
    pub fn uniform(value: T) -> Self
    where
        T: Clone,
    {
        Self {
            mobile: value.clone(),
            tablet: value.clone(),
            desktop: value,
        }
    }

    pub fn get(&self, bp: Breakpoint) -> &T {
        match bp {
            Breakpoint::Mobile => &self.mobile,
            Breakpoint::Tablet => &self.tablet,
            Breakpoint::Desktop => &self.desktop,
        }
    }
}

/// Generate complete responsive CSS media queries for the cockpit.
pub fn generate_responsive_css(layout: &ResponsiveLayout) -> String {
    let mut css = String::new();

    // Base styles (mobile-first)
    css.push_str("/* === Responsive Cockpit Styles === */\n\n");
    css.push_str(":root {\n");
    css.push_str(&format!(
        "  --grid-columns: {};\n",
        layout.grid_columns.mobile
    ));
    css.push_str(&format!("  --font-scale: {};\n", layout.font_scale.mobile));
    css.push_str(&format!(
        "  --touch-target-min: {}px;\n",
        layout.touch_target_min.mobile
    ));
    css.push_str("}\n\n");

    // Mobile breakpoint
    css.push_str(&format!("@media {} {{\n", Breakpoint::Mobile.media_query()));
    css.push_str("  .cockpit-sidebar { display: none; }\n");
    css.push_str("  .cockpit-grid { grid-template-columns: 1fr; }\n");
    css.push_str("  .cockpit-panel { width: 100%; }\n");
    css.push_str("  .cockpit-nav { position: fixed; bottom: 0; left: 0; right: 0; }\n");
    css.push_str("}\n\n");

    // Tablet breakpoint
    css.push_str(&format!("@media {} {{\n", Breakpoint::Tablet.media_query()));
    css.push_str(&format!(
        "  :root {{ --grid-columns: {}; --font-scale: {}; --touch-target-min: {}px; }}\n",
        layout.grid_columns.tablet, layout.font_scale.tablet, layout.touch_target_min.tablet
    ));
    if !layout.sidebar_visible.tablet {
        css.push_str("  .cockpit-sidebar { display: none; }\n");
    } else {
        css.push_str("  .cockpit-sidebar { display: flex; width: 220px; }\n");
    }
    css.push_str(&format!(
        "  .cockpit-grid {{ grid-template-columns: repeat({}, 1fr); }}\n",
        layout.grid_columns.tablet
    ));
    css.push_str("}\n\n");

    // Desktop breakpoint
    css.push_str(&format!(
        "@media {} {{\n",
        Breakpoint::Desktop.media_query()
    ));
    css.push_str(&format!(
        "  :root {{ --grid-columns: {}; --font-scale: {}; --touch-target-min: {}px; }}\n",
        layout.grid_columns.desktop, layout.font_scale.desktop, layout.touch_target_min.desktop
    ));
    css.push_str("  .cockpit-sidebar { display: flex; width: 260px; }\n");
    css.push_str(&format!(
        "  .cockpit-grid {{ grid-template-columns: repeat({}, 1fr); }}\n",
        layout.grid_columns.desktop
    ));
    css.push_str("}\n\n");

    // Orientation queries
    css.push_str("@media (orientation: portrait) {\n");
    css.push_str("  .cockpit-main { flex-direction: column; }\n");
    css.push_str("}\n\n");

    // Reduced motion preference
    css.push_str("@media (prefers-reduced-motion: reduce) {\n");
    css.push_str("  *, *::before, *::after { animation-duration: 0.01ms !important; transition-duration: 0.01ms !important; }\n");
    css.push_str("}\n\n");

    // Dark/light mode
    css.push_str("@media (prefers-color-scheme: dark) {\n");
    css.push_str("  :root { --bg-primary: #0a0a0a; --fg-primary: #fafafa; }\n");
    css.push_str("}\n");
    css.push_str("@media (prefers-color-scheme: light) {\n");
    css.push_str("  :root { --bg-primary: #ffffff; --fg-primary: #0a0a0a; }\n");
    css.push_str("}\n");

    css
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_breakpoint_from_width() {
        assert_eq!(Breakpoint::from_width(0), Breakpoint::Mobile);
        assert_eq!(Breakpoint::from_width(320), Breakpoint::Mobile);
        assert_eq!(Breakpoint::from_width(639), Breakpoint::Mobile);
        assert_eq!(Breakpoint::from_width(640), Breakpoint::Tablet);
        assert_eq!(Breakpoint::from_width(768), Breakpoint::Tablet);
        assert_eq!(Breakpoint::from_width(1023), Breakpoint::Tablet);
        assert_eq!(Breakpoint::from_width(1024), Breakpoint::Desktop);
        assert_eq!(Breakpoint::from_width(1920), Breakpoint::Desktop);
    }

    #[test]
    fn test_breakpoint_display() {
        assert_eq!(format!("{}", Breakpoint::Mobile), "mobile");
        assert_eq!(format!("{}", Breakpoint::Tablet), "tablet");
        assert_eq!(format!("{}", Breakpoint::Desktop), "desktop");
    }

    #[test]
    fn test_breakpoint_media_queries() {
        assert!(Breakpoint::Mobile
            .media_query()
            .contains("max-width: 639px"));
        assert!(Breakpoint::Tablet
            .media_query()
            .contains("min-width: 640px"));
        assert!(Breakpoint::Tablet
            .media_query()
            .contains("max-width: 1023px"));
        assert!(Breakpoint::Desktop
            .media_query()
            .contains("min-width: 1024px"));
    }

    #[test]
    fn test_breakpoint_min_max_width() {
        assert_eq!(Breakpoint::Mobile.min_width_px(), 0);
        assert_eq!(Breakpoint::Mobile.max_width_px(), Some(639));
        assert_eq!(Breakpoint::Tablet.min_width_px(), 640);
        assert_eq!(Breakpoint::Tablet.max_width_px(), Some(1023));
        assert_eq!(Breakpoint::Desktop.min_width_px(), 1024);
        assert_eq!(Breakpoint::Desktop.max_width_px(), None);
    }

    #[test]
    fn test_breakpoint_all_count() {
        assert_eq!(Breakpoint::all().len(), 3);
    }

    #[test]
    fn test_viewport_from_dimensions_mobile() {
        let vp = ViewportState::from_dimensions(375, 812);
        assert_eq!(vp.breakpoint, Breakpoint::Mobile);
        assert!(vp.is_mobile());
        assert!(vp.is_portrait);
    }

    #[test]
    fn test_viewport_from_dimensions_tablet() {
        let vp = ViewportState::from_dimensions(768, 1024);
        assert_eq!(vp.breakpoint, Breakpoint::Tablet);
        assert!(vp.is_tablet());
        assert!(vp.is_portrait);
    }

    #[test]
    fn test_viewport_from_dimensions_desktop() {
        let vp = ViewportState::from_dimensions(1920, 1080);
        assert_eq!(vp.breakpoint, Breakpoint::Desktop);
        assert!(vp.is_desktop());
        assert!(!vp.is_portrait);
    }

    #[test]
    fn test_viewport_default() {
        let vp = ViewportState::default();
        assert_eq!(vp.width, 1280);
        assert_eq!(vp.breakpoint, Breakpoint::Desktop);
    }

    #[test]
    fn test_viewport_with_dpr() {
        let vp = ViewportState::new(750, 1334, 2.0, SafeAreaInsets::default());
        assert!((vp.logical_width() - 375.0).abs() < f64::EPSILON);
        assert!((vp.logical_height() - 667.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_safe_area_insets() {
        let sa = SafeAreaInsets::new(44, 0, 34, 0);
        assert_eq!(sa.horizontal(), 0);
        assert_eq!(sa.vertical(), 78);
    }

    #[test]
    fn test_usable_dimensions_with_safe_area() {
        let vp = ViewportState::new(375, 812, 1.0, SafeAreaInsets::new(44, 0, 34, 0));
        assert_eq!(vp.usable_width(), 375);
        assert_eq!(vp.usable_height(), 734); // 812 - 44 - 34
    }

    #[test]
    fn test_responsive_layout_defaults() {
        let layout = ResponsiveLayout::default();
        assert!(!*layout.for_breakpoint(&layout.sidebar_visible, Breakpoint::Mobile));
        assert!(*layout.for_breakpoint(&layout.sidebar_visible, Breakpoint::Desktop));
        assert_eq!(
            *layout.for_breakpoint(&layout.grid_columns, Breakpoint::Mobile),
            1
        );
        assert_eq!(
            *layout.for_breakpoint(&layout.grid_columns, Breakpoint::Desktop),
            3
        );
        assert!(*layout.for_breakpoint(&layout.stack_panels, Breakpoint::Mobile));
        assert!(!*layout.for_breakpoint(&layout.stack_panels, Breakpoint::Desktop));
    }

    #[test]
    fn test_breakpoint_config_uniform() {
        let config = BreakpointConfig::uniform(42u32);
        assert_eq!(config.mobile, 42);
        assert_eq!(config.tablet, 42);
        assert_eq!(config.desktop, 42);
    }

    #[test]
    fn test_breakpoint_config_get() {
        let config = BreakpointConfig {
            mobile: "m",
            tablet: "t",
            desktop: "d",
        };
        assert_eq!(*config.get(Breakpoint::Mobile), "m");
        assert_eq!(*config.get(Breakpoint::Tablet), "t");
        assert_eq!(*config.get(Breakpoint::Desktop), "d");
    }

    #[test]
    fn test_css_variables_generation() {
        let layout = ResponsiveLayout::default();
        let vp = ViewportState::from_dimensions(375, 812);
        let css = layout.css_variables(&vp);

        assert!(css.contains("--sidebar-visible: 0"));
        assert!(css.contains("--grid-columns: 1"));
        assert!(css.contains("--stack-panels: 1"));
        assert!(css.contains("--touch-target-min: 44px"));
    }

    #[test]
    fn test_generate_responsive_css_contains_breakpoints() {
        let layout = ResponsiveLayout::default();
        let css = generate_responsive_css(&layout);

        assert!(css.contains("@media (max-width: 639px)"));
        assert!(css.contains("@media (min-width: 640px) and (max-width: 1023px)"));
        assert!(css.contains("@media (min-width: 1024px)"));
        assert!(css.contains(".cockpit-sidebar"));
        assert!(css.contains(".cockpit-grid"));
        assert!(css.contains("prefers-reduced-motion"));
        assert!(css.contains("prefers-color-scheme"));
    }

    #[test]
    fn test_generate_responsive_css_mobile_hides_sidebar() {
        let layout = ResponsiveLayout::default();
        let css = generate_responsive_css(&layout);

        // Mobile section should hide sidebar
        let mobile_section = css
            .find("@media (max-width: 639px)")
            .expect("mobile media query missing");
        let after_mobile = &css[mobile_section..];
        let end_brace = after_mobile.find('}').unwrap();
        let mobile_block = &after_mobile[..end_brace];
        assert!(mobile_block.contains("display: none"));
    }

    #[test]
    fn test_viewport_serialization_roundtrip() {
        let vp = ViewportState::new(768, 1024, 2.0, SafeAreaInsets::new(20, 0, 20, 0));

        let json = serde_json::to_string(&vp).unwrap();
        let restored: ViewportState = serde_json::from_str(&json).unwrap();
        assert_eq!(vp, restored);
    }

    #[test]
    fn test_safe_area_serialization() {
        let sa = SafeAreaInsets::new(44, 8, 34, 8);
        let json = serde_json::to_string(&sa).unwrap();
        let restored: SafeAreaInsets = serde_json::from_str(&json).unwrap();
        assert_eq!(sa, restored);
    }

    #[test]
    fn test_responsive_layout_serialization() {
        let layout = ResponsiveLayout::default();
        let json = serde_json::to_string(&layout).unwrap();
        let restored: ResponsiveLayout = serde_json::from_str(&json).unwrap();
        assert_eq!(layout, restored);
    }

    #[test]
    fn test_breakpoint_serialization() {
        for bp in Breakpoint::all() {
            let json = serde_json::to_string(bp).unwrap();
            let restored: Breakpoint = serde_json::from_str(&json).unwrap();
            assert_eq!(*bp, restored);
        }
    }

    #[test]
    fn test_viewport_landscape() {
        let vp = ViewportState::from_dimensions(1024, 768);
        assert!(!vp.is_portrait);
    }

    #[test]
    fn test_css_variables_desktop() {
        let layout = ResponsiveLayout::default();
        let vp = ViewportState::from_dimensions(1920, 1080);
        let css = layout.css_variables(&vp);

        assert!(css.contains("--sidebar-visible: 1"));
        assert!(css.contains("--grid-columns: 3"));
        assert!(css.contains("--stack-panels: 0"));
    }
}
