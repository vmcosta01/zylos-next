//! Zylos Cockpit — Leptos WASM frontend entrypoint.
//!
//! Este módulo existe apenas para satisfazer `crate-type = ["cdylib", "rlib"]`
//! no Cargo.toml e expor a função de hidratação/mount ao JavaScript via
//! wasm-bindgen. A lógica real da UI vive em `main.rs` e nos componentes
//! Leptos; este arquivo re-exporta o necessário para o build WASM funcionar.

// Sem `#[wasm_bindgen(start)]` aqui: o binário (`main.rs`) importa esta lib e,
// se a lib tivesse um start, o wasm-bindgen o usaria NO LUGAR de `main()` — o
// App nunca seria montado (tela presa em "Carregando…"). O panic hook e o
// `mount_to_body` ficam em `main.rs`.

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

/// Componentes do cockpit — expostos publicamente para testes E2E e integração.
pub mod auth;

pub mod components;

/// Cliente CRDT (yrs) do painel de edição colaborativa (Fase 11.9).
pub mod crdt_client;

/// Resolução da URL base do gateway.
pub mod gateway_config;

/// HTTP autenticado ao gateway (GET/POST/DELETE JSON).
pub mod gateway_http;

/// Traduções pt-BR / en-US.
pub mod i18n;

/// Abas do cockpit (enum, ordem, chaves i18n, atalhos).
pub mod tabs;

pub mod ui_error;
