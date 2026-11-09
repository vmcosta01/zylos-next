//! Zylos Cockpit — Leptos WASM frontend entrypoint.
//!
//! Este módulo existe apenas para satisfazer `crate-type = ["cdylib", "rlib"]`
//! no Cargo.toml e expor a função de hidratação/mount ao JavaScript via
//! wasm-bindgen. A lógica real da UI vive em `main.rs` e nos componentes
//! Leptos; este arquivo re-exporta o necessário para o build WASM funcionar.


/// Ponto de entrada chamado pelo JS (`wasm_bindgen`) para montar a aplicação
/// no DOM quando usado como biblioteca cdylib.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    // mount_to_body será chamado pelo hydrate/csr runtime do Leptos;
    // aqui garantimos apenas que o panic hook está instalado.
}

// Re-exporta o main module apenas quando compilado como binário.
// Em `cargo check --workspace` (lib-only) ou docs, `crate::main` não existe;
// o módulo app fica vazio para satisfazer o type-check sem quebrar o build.
#[cfg(all(not(target_arch = "wasm32"), feature = "ssr"))]
pub mod app {
    // SSR re-exports would go here when the ssr feature is enabled.
}

/// Fallback seguro para todos os outros targets (lib check, docs, wasm).
#[cfg(not(all(not(target_arch = "wasm32"), feature = "ssr")))]
pub mod app {
    // Empty during lib-only / doc / wasm builds; real components live in main.rs.
}