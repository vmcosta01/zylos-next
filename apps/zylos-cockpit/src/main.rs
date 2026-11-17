//! Zylos Cockpit — Leptos WASM Frontend (ADR-042)
//!
//! Supports both CSR-only and SSR+hydration modes via feature flags.
//! In hydrate mode, uses `hydrate_body` to attach to server-rendered HTML.
//! In CSR mode, uses `mount_to_body` for client-only rendering.

use leptos::prelude::*;

mod app;
mod benchmarks;
mod components;
mod i18n;
mod streaming;
use app::App;

fn main() {
    console_error_panic_hook::set_once();

    #[cfg(feature = "hydrate")]
    {
        hydrate_body(App);
    }

    #[cfg(not(feature = "hydrate"))]
    {
        mount_to_body(App);
    }
}
