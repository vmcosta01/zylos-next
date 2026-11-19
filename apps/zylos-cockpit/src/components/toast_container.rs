//! Zylos Cockpit — Toast Notification System (Fase 7)
//!
//! Global toast container with 4 variants, auto-dismiss, pause-on-hover,
//! and spring entrance/exit animations per DESIGN_SYSTEM.md §7.2.

use leptos::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;

/// Toast variant determines color, icon, and dismiss behavior
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastVariant {
    Success,
    Error,
    Warning,
    Info,
}

impl ToastVariant {
    pub fn css_class(self) -> &'static str {
        match self {
            ToastVariant::Success => "toast-success",
            ToastVariant::Error => "toast-error",
            ToastVariant::Warning => "toast-warning",
            ToastVariant::Info => "toast-info",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            ToastVariant::Success => "✓",
            ToastVariant::Error => "✕",
            ToastVariant::Warning => "⚠",
            ToastVariant::Info => "ℹ",
        }
    }

    /// Auto-dismiss duration: 5s for success/info, None for error/warning (manual)
    pub fn auto_dismiss_ms(self) -> Option<u32> {
        match self {
            ToastVariant::Success | ToastVariant::Info => Some(5000),
            ToastVariant::Error | ToastVariant::Warning => None,
        }
    }
}

/// A single toast notification
#[derive(Debug, Clone, PartialEq)]
pub struct Toast {
    pub id: u64,
    pub variant: ToastVariant,
    pub message: String,
}

/// Global toast store — RwSignal<Vec<Toast>> provided via context
#[derive(Clone)]
pub struct ToastStore {
    toasts: RwSignal<Vec<Toast>>,
    next_id: std::sync::Arc<AtomicU64>,
}

impl ToastStore {
    pub fn new() -> Self {
        Self {
            toasts: RwSignal::new(Vec::new()),
            next_id: std::sync::Arc::new(AtomicU64::new(1)),
        }
    }

    pub fn dispatch(&self, variant: ToastVariant, message: impl Into<String>) {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let toast = Toast {
            id,
            variant,
            message: message.into(),
        };
        self.toasts.update(|list| {
            list.push(toast);
            // Max 3 visible — remove oldest if exceeded
            while list.len() > 3 {
                list.remove(0);
            }
        });
    }

    pub fn remove(&self, id: u64) {
        self.toasts.update(|list| {
            list.retain(|t| t.id != id);
        });
    }

    pub fn toasts(&self) -> RwSignal<Vec<Toast>> {
        self.toasts
    }
}

impl Default for ToastStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Provide toast store to component tree
pub fn provide_toast_store() -> ToastStore {
    let store = ToastStore::new();
    provide_context(store.clone());
    store
}

/// Access toast store from any component
pub fn use_toast_store() -> ToastStore {
    expect_context::<ToastStore>()
}

/// Convenience functions for dispatching toasts
pub fn toast_success(msg: impl Into<String>) {
    use_toast_store().dispatch(ToastVariant::Success, msg);
}

pub fn toast_error(msg: impl Into<String>) {
    use_toast_store().dispatch(ToastVariant::Error, msg);
}

pub fn toast_warning(msg: impl Into<String>) {
    use_toast_store().dispatch(ToastVariant::Warning, msg);
}

pub fn toast_info(msg: impl Into<String>) {
    use_toast_store().dispatch(ToastVariant::Info, msg);
}

/// Single toast item with auto-dismiss and pause-on-hover
#[component]
fn ToastItem(toast: Toast) -> impl IntoView {
    let store = use_toast_store();
    let id = toast.id;
    let variant = toast.variant;
    let message = toast.message.clone();
    let css_class = variant.css_class().to_string();
    let icon = variant.icon().to_string();

    // Timer handle for auto-dismiss (web_sys setTimeout returns i32)
    let timer_handle: StoredValue<Option<i32>> = StoredValue::new(None);
    // Track remaining time for pause-on-hover
    let remaining_ms: StoredValue<Option<u32>> = StoredValue::new(variant.auto_dismiss_ms());
    let paused: StoredValue<bool> = StoredValue::new(false);

    let store_for_timer = store.clone();
    let start_timer = std::rc::Rc::new(move || {
        if let Some(ms) = remaining_ms.get_value() {
            let store_clone = store_for_timer.clone();
            let id_clone = id;
            let cb = Closure::wrap(Box::new(move || {
                store_clone.remove(id_clone);
            }) as Box<dyn FnMut()>);

            if let Some(window) = web_sys::window() {
                let handle = window
                    .set_timeout_with_callback_and_timeout_and_arguments_0(
                        cb.as_ref().unchecked_ref(),
                        ms as i32,
                    )
                    .ok();
                timer_handle.set_value(handle);
            }
            cb.forget();
        }
    });

    let clear_timer = move || {
        if let Some(handle) = timer_handle.get_value() {
            if let Some(window) = web_sys::window() {
                window.clear_timeout_with_handle(handle);
            }
            timer_handle.set_value(None);
        }
    };

    // Start auto-dismiss timer on mount
    let start_timer_effect = start_timer.clone();
    Effect::new(move |_| {
        start_timer_effect();
    });

    // Cleanup on unmount
    on_cleanup(move || {
        clear_timer();
    });

    let on_mouse_enter = move |_| {
        paused.set_value(true);
        clear_timer();
    };

    let start_timer_mouse = start_timer.clone();
    let on_mouse_leave = move |_| {
        paused.set_value(false);
        start_timer_mouse();
    };

    let on_close = move |_| {
        clear_timer();
        store.remove(id);
    };

    view! {
        <div
            class=format!("toast-item {}", css_class)
            role="alert"
            aria-live="polite"
            on:mouseenter=on_mouse_enter
            on:mouseleave=on_mouse_leave
        >
            <span class="toast-icon">{icon}</span>
            <span class="toast-message">{message}</span>
            <button
                class="toast-close"
                aria-label="Fechar notificação"
                on:click=on_close
            >"×"</button>
        </div>
    }
}

/// ToastContainer — fixed bottom-right, renders active toasts
#[component]
pub fn ToastContainer() -> impl IntoView {
    let store = use_toast_store();
    let toasts = store.toasts();

    view! {
        <div class="toast-container" aria-live="polite" aria-label="Notificações">
            {move || {
                let items = toasts.get();
                items.iter().rev().map(|toast| {
                    view! { <ToastItem toast=toast.clone() /> }
                }).collect::<Vec<_>>()
            }}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_variant_css_classes() {
        assert_eq!(ToastVariant::Success.css_class(), "toast-success");
        assert_eq!(ToastVariant::Error.css_class(), "toast-error");
        assert_eq!(ToastVariant::Warning.css_class(), "toast-warning");
        assert_eq!(ToastVariant::Info.css_class(), "toast-info");
    }

    #[test]
    fn test_variant_icons() {
        assert_eq!(ToastVariant::Success.icon(), "✓");
        assert_eq!(ToastVariant::Error.icon(), "✕");
        assert_eq!(ToastVariant::Warning.icon(), "⚠");
        assert_eq!(ToastVariant::Info.icon(), "ℹ");
    }

    #[test]
    fn test_auto_dismiss_timing() {
        assert_eq!(ToastVariant::Success.auto_dismiss_ms(), Some(5000));
        assert_eq!(ToastVariant::Info.auto_dismiss_ms(), Some(5000));
        assert_eq!(ToastVariant::Error.auto_dismiss_ms(), None);
        assert_eq!(ToastVariant::Warning.auto_dismiss_ms(), None);
    }

    #[test]
    fn test_store_max_three() {
        let store = ToastStore::new();
        store.dispatch(ToastVariant::Info, "one");
        store.dispatch(ToastVariant::Info, "two");
        store.dispatch(ToastVariant::Info, "three");
        store.dispatch(ToastVariant::Info, "four");
        let toasts = store.toasts().get_untracked();
        assert_eq!(toasts.len(), 3);
        assert_eq!(toasts[0].message, "two");
        assert_eq!(toasts[2].message, "four");
    }

    #[test]
    fn test_store_remove() {
        let store = ToastStore::new();
        store.dispatch(ToastVariant::Success, "test");
        let id = store.toasts().get_untracked()[0].id;
        store.remove(id);
        assert!(store.toasts().get_untracked().is_empty());
    }
}
