[2m│[0m [1mapps\zylos-cockpit\src\components\toast_container.rs[0m
[2m│[0m [38;2;139;134;130m@@ -0,0 +1,283 @@[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! Zylos Cockpit — Toast Notification System (Fase 7)[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//![0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! Global toast container with 4 variants, auto-dismiss, pause-on-hover,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! and spring entrance/exit animations per DESIGN_SYSTEM.md §7.2.[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+use leptos::prelude::*;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+use std::time::Duration;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+/// Toast variant determines color, icon, and dismiss behavior[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+#[derive(Debug, Clone, Copy, PartialEq, Eq)][0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+pub enum ToastVariant {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    Success,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    Error,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    Warning,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    Info,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+}[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+impl ToastVariant {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    pub fn css_class(self) -> &'static str {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        match self {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            ToastVariant::Success => "toast-success",[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            ToastVariant::Error => "toast-error",[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            ToastVariant::Warning => "toast-warning",[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            ToastVariant::Info => "toast-info",[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        }[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    }[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    pub fn icon(self) -> &'static str {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        match self {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            ToastVariant::Success => "✓",[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            ToastVariant::Error => "✕",[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            ToastVariant::Warning => "⚠",[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            ToastVariant::Info => "ℹ",[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        }[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    }[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    /// Auto-dismiss duration: 5s for success/info, None for error/warning (manual)[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    pub fn auto_dismiss_ms(self) -> Option<u32> {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        match self {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            ToastVariant::Success | ToastVariant::Info => Some(5000),[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            ToastVariant::Error | ToastVariant::Warning => None,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        }[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    }[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+}[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+/// A single toast notification[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+#[derive(Debug, Clone, PartialEq)][0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+pub struct Toast {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    pub id: u64,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    pub variant: ToastVariant,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    pub message: String,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+}[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+/// Global toast store — RwSignal<Vec<Toast>> provided via context[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+#[derive(Clone)][0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+pub struct ToastStore {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    toasts: RwSignal<Vec<Toast>>,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    next_id: std::cell::Cell<u64>,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+}[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+impl ToastStore {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    pub fn new() -> Self {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        Self {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            toasts: RwSignal::new(Vec::new()),[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            next_id: std::cell::Cell::new(1),[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        }[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    }[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    pub fn dispatch(&self, variant: ToastVariant, message: impl Into<String>) {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        let id = self.next_id.get();[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        self.next_id.set(id + 1);[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        let toast = Toast {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            id,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            variant,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            message: message.into(),[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        };[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        self.toasts.update(|list| {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            list.push(toast);[0m
[2m│[0m [38;2;139;134;130m… 205 linha(s) de diff omitida(s) em 1 arquivo(s)/seção(ões) adicional(is)[0m
[2m│[0m [1mapps\zylos-cockpit\src\components\empty_state.rs[0m
[2m│[0m [38;2;139;134;130m@@ -0,0 +1,83 @@[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! Zylos Cockpit — EmptyState Component (Fase 7)[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//![0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! Reusable empty state with inline SVG illustration, title, description,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! and optional CTA button per DESIGN_SYSTEM.md §6 states.[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+use leptos::prelude::*;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+/// Inline SVG icons for empty states[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+pub mod icons {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    pub const ROBOT: &str = r#"<svg width="64" height="64" viewBox="0 0 64 64" fill="none" xmlns="http://www.w3.org/2000/svg"><rect x="16" y="20" width="32" height="28" rx="4" stroke="currentColor" stroke-width="2"/><circle cx="26" cy="32" r="3" fill="currentColor"/><circle cx="38" cy="32" r="3" fill="currentColor"/><path d="M26 42h12" stroke="currentColor" stroke-width="2" stroke-linecap="round"/><path d="M32 12v8" stroke="currentColor" stroke-width="2" stroke-linecap="round"/><circle cx="32" cy="10" r="3" stroke="currentColor" stroke-width="2"/><path d="M12 30h4M48 30h4" stroke="currentColor" stroke-width="2" stroke-linecap="round"/><path d="M20 48v6M44 48v6" stroke="currentColor" stroke-width="2" stroke-linecap="round"/></svg>"#;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    pub const DOCUMENT: &str = r#"<svg width="64" height="64" viewBox="0 0 64 64" fill="none" xmlns="http://www.w3.org/2000/svg"><path d="M18 8h20l12 12v36a4 4 0 01-4 4H18a4 4 0 01-4-4V12a4 4 0 014-4z" stroke="currentColor" stroke-width="2"/><path d="M38 8v12h12" stroke="currentColor" stroke-width="2"/><path d="M24 32h16M24 40h10" stroke="currentColor" stroke-width="2" stroke-linecap="round"/></svg>"#;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    pub const CHART: &str = r#"<svg width="64" height="64" viewBox="0 0 64 64" fill="none" xmlns="http://www.w3.org/2000/svg"><rect x="8" y="36" width="10" height="20" rx="2" stroke="currentColor" stroke-width="2"/><rect x="22" y="24" width="10" height="32" rx="2" stroke="currentColor" stroke-width="2"/><rect x="36" y="16" width="10" height="40" rx="2" stroke="currentColor" stroke-width="2"/><rect x="50" y="28" width="10" height="28" rx="2" stroke="currentColor" stroke-width="2"/><path d="M8 60h52" stroke="currentColor" stroke-width="2" stroke-linecap="round"/></svg>"#;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    pub const SEARCH: &str = r#"<svg width="64" height="64" viewBox="0 0 64 64" fill="none" xmlns="http://www.w3.org/2000/svg"><circle cx="28" cy="28" r="16" stroke="currentColor" stroke-width="2"/><path d="M40 40l14 14" stroke="currentColor" stroke-width="2" stroke-linecap="round"/><path d="M22 28h12M28 22v12" stroke="currentColor" stroke-width="2" stroke-linecap="round"/></svg>"#;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+}[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+/// EmptyState component props[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+#[component][0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+pub fn EmptyState([0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    /// SVG markup string (use icons::ROBOT, icons::DOCUMENT, etc.)[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    #[prop(default = icons::ROBOT)][0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    icon: &'static str,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    /// Title text[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    title: String,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    /// Description text[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    #[prop(default = String::new())][0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    description: String,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    /// Optional CTA button label[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    #[prop(optional)][0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    cta_label: Option<String>,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    /// Optional CTA click handler[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    #[prop(optional)][0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    on_cta: Option<Callback<()>>,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+) -> impl IntoView {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    let has_cta = cta_label.is_some();[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    let label = cta_label.unwrap_or_default();[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    view! {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        <div class="empty-state-v2">[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            <div class="empty-state-icon" inner_html=icon></div>[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            <h3 class="empty-state-title">{title}</h3>[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            {if !description.is_empty() {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                view! { <p class="empty-state-description">{description}</p> }.into_any()[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            } else {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                view! { <span></span> }.into_any()[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            }}[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            {if has_cta {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                let cb = on_cta;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                view! {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                    <button[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                        class="cta-primary empty-state-cta"[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                        on:click=move |_| { if let Some(ref c) = cb { c.run(()); } }[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                    >{label.clone()}</button>[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                }.into_any()[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            } else {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                view! { <span></span> }.into_any()[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            }}[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        </div>[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    }[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+}[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+#[cfg(test)][0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+mod tests {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    use super::*;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    #[test][0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    fn test_icons_are_valid_svg() {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        assert!(icons::ROBOT.contains("<svg"));[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        assert!(icons::DOCUMENT.contains("<svg"));[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        assert!(icons::CHART.contains("<svg"));[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        assert!(icons::SEARCH.contains("<svg"));[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    }[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    #[test][0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    fn test_icons_close_properly() {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        assert!(icons::ROBOT.contains("</svg>"));[0m
[2m│[0m [38;2;139;134;130m… 5 linha(s) de diff omitida(s) em 1 arquivo(s)/seção(ões) adicional(is)[0m
[2m│[0m [1mapps\zylos-cockpit\src\components\error_boundary.rs[0m
[2m│[0m [38;2;139;134;130m@@ -0,0 +1,70 @@[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! Zylos Cockpit — ErrorBoundary Component (Fase 7)[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//![0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! Catches panics in child components and renders a fallback UI with[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! retry capability per DESIGN_SYSTEM.md §6 mandatory states.[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+use leptos::prelude::*;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+/// ErrorBoundary component — wraps children and catches panics[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+///[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+/// In WASM, `std::panic::catch_unwind` works for Rust panics but not JS errors.[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+/// This provides a recovery UI for component-level failures.[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+#[component][0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+pub fn ErrorBoundary([0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    /// Fallback message prefix[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    #[prop(default = "Algo deu errado".to_string())][0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    fallback_title: String,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    /// Children to render[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    children: Children,[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+) -> impl IntoView {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    let (error, set_error) = signal::<Option<String>>(None);[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    // Set panic hook to capture error messages[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    let set_err_clone = set_error;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    let _hook = std::panic::set_hook(Box::new(move |info| {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        let msg = if let Some(s) = info.payload().downcast_ref::<&str>() {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            s.to_string()[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        } else if let Some(s) = info.payload().downcast_ref::<String>() {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            s.clone()[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        } else {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            "Unknown error".to_string()[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        };[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        set_err_clone.set(Some(msg));[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    }));[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    let on_retry = move |_| {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        set_error.set(None);[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    };[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    view! {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        {move || {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            if let Some(ref err_msg) = error.get() {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                let title = fallback_title.clone();[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                let msg = err_msg.clone();[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                view! {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                    <div class="error-boundary" role="alert">[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                        <div class="error-boundary-icon">"⚠"</div>[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                        <h3 class="error-boundary-title">{title}</h3>[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                        <p class="error-boundary-message">{msg}</p>[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                        <button class="cta-retry" on:click=on_retry>[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                            "Tentar novamente"[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                        </button>[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                    </div>[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                }.into_any()[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            } else {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                view! { <>{children()}</> }.into_any()[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            }[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        }}[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    }[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+}[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+#[cfg(test)][0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+mod tests {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    use super::*;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    #[test][0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    fn test_error_boundary_compiles() {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        // Verify the component type exists and is constructible[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        let _ = ErrorBoundary;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    }[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+}[0m
