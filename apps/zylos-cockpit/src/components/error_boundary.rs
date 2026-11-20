//! Zylos Cockpit — ErrorBoundary Component (Fase 7)
//!
//! Catches panics in child components and renders a fallback UI with
//! retry capability per DESIGN_SYSTEM.md §6 mandatory states.

use leptos::prelude::*;

/// ErrorBoundary component — wraps children and catches panics
///
/// In WASM, `std::panic::catch_unwind` works for Rust panics but not JS errors.
/// This provides a recovery UI for component-level failures.
#[component]
pub fn ErrorBoundary(
    /// Fallback message prefix
    #[prop(default = "Algo deu errado".to_string())]
    fallback_title: String,
    /// Children to render
    children: ChildrenFn,
) -> impl IntoView {
    let (error, set_error) = signal::<Option<String>>(None);

    // Set panic hook to capture error messages
    let set_err_clone = set_error;
    std::panic::set_hook(Box::new(move |info| {
        let msg = if let Some(s) = info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Unknown error".to_string()
        };
        set_err_clone.set(Some(msg));
    }));

    let on_retry = move |_| {
        set_error.set(None);
    };

    view! {
        {move || {
            if let Some(ref err_msg) = error.get() {
                let title = fallback_title.clone();
                let msg = err_msg.clone();
                view! {
                    <div class="error-boundary" role="alert">
                        <div class="error-boundary-icon"><i class="ph ph-warning" aria-hidden="true"></i></div>
                        <h3 class="error-boundary-title">{title}</h3>
                        <p class="error-boundary-message">{msg}</p>
                        <button class="cta-retry" on:click=on_retry>
                            "Tentar novamente"
                        </button>
                    </div>
                }.into_any()
            } else {
                view! { <div>{children()}</div> }.into_any()
            }
        }}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_boundary_compiles() {
        // Verify the component type exists and is constructible
        let _ = ErrorBoundary;
    }
}
