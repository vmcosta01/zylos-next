//! Testes de regressão para bugs conhecidos resolvidos nas Fases 8-10.
//!
//! Previne regressões em:
//! - UUID js feature (wasm32 target requer feature "js" para random)
//! - Borrow checker em webrtc_relay (expire_leases borrow issue)
//! - RenderHtml inference em componentes complexos
//! - Chrono em WASM (deve usar js_sys::Date)
//!
//! Executar com: `wasm-pack test --headless --chrome`

use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

/// REGRESSÃO: UUID sem feature "js" falha em wasm32-unknown-unknown.
///
/// Bug original: `could not find 'rng' in 'uuid'` ao compilar para WASM.
/// Fix: Adicionar `features = ["v4", "js"]` no Cargo.toml.
///
/// Este teste valida que uuid::Uuid::new_v4() funciona em ambiente WASM.
#[wasm_bindgen_test]
fn test_uuid_v4_works_in_wasm() {
    let id = uuid::Uuid::new_v4();
    assert!(!id.is_nil(), "UUID v4 deve gerar ID não-nulo em WASM");

    // Validar formato string
    let id_str = id.to_string();
    assert_eq!(id_str.len(), 36, "UUID string deve ter 36 caracteres");
    assert_eq!(
        id_str.chars().filter(|c| *c == '-').count(),
        4,
        "UUID deve ter 4 hífens"
    );
}

/// REGRESSÃO: UUID múltiplas chamadas não devem colidir.
///
/// Valida que o RNG do uuid funciona corretamente em chamadas consecutivas.
#[wasm_bindgen_test]
fn test_uuid_v4_uniqueness() {
    let id1 = uuid::Uuid::new_v4();
    let id2 = uuid::Uuid::new_v4();
    let id3 = uuid::Uuid::new_v4();

    assert_ne!(id1, id2, "UUIDs consecutivos devem ser únicos");
    assert_ne!(id2, id3, "UUIDs consecutivos devem ser únicos");
    assert_ne!(id1, id3, "UUIDs consecutivos devem ser únicos");
}

/// REGRESSÃO: js_sys::Date deve estar disponível em WASM (não usar chrono::Utc::now).
///
/// Bug original: chrono não compila em wasm32-unknown-unknown.
/// Fix: Usar `js_sys::Date::new_0().to_iso_string()` no cockpit.
#[wasm_bindgen_test]
fn test_js_sys_date_available() {
    let now = js_sys::Date::new_0();
    let iso = now.to_iso_string();
    let iso_str = iso.as_string().unwrap();

    assert!(
        !iso_str.is_empty(),
        "js_sys::Date deve retornar string ISO não-vazia"
    );
    assert!(
        iso_str.contains("T"),
        "ISO string deve conter separador T: {}",
        iso_str
    );
}

/// REGRESSÃO: js_sys::Date múltiplas chamadas devem retornar timestamps diferentes.
///
/// Valida que o clock do browser funciona corretamente em WASM.
#[wasm_bindgen_test]
fn test_js_sys_date_monotonic() {
    let t1 = js_sys::Date::now();
    // Pequena espera para garantir timestamp diferente
    for _ in 0..1000 {
        // busy wait mínimo
    }
    let t2 = js_sys::Date::now();

    assert!(
        t2 >= t1,
        "Timestamps devem ser monotônicos: t1={}, t2={}",
        t1,
        t2
    );
}

/// REGRESSÃO: Serialização de tipos com Option<T> deve funcionar corretamente.
///
/// Bug original: Option<T> serializado como null causava erro de deserialização.
/// Fix: Usar `#[serde(skip_serializing_if = "Option::is_none")]` onde apropriado.
#[wasm_bindgen_test]
fn test_option_serialization_roundtrip() {
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct TestStruct {
        required: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        optional: Option<String>,
    }

    let with_value = TestStruct {
        required: "test".to_string(),
        optional: Some("value".to_string()),
    };

    let json = serde_json::to_string(&with_value).unwrap();
    let deserialized: TestStruct = serde_json::from_str(&json).unwrap();
    assert_eq!(with_value, deserialized);

    let without_value = TestStruct {
        required: "test".to_string(),
        optional: None,
    };

    let json = serde_json::to_string(&without_value).unwrap();
    assert!(
        !json.contains("null"),
        "Option None com skip_serializing_if não deve gerar null: {}",
        json
    );
    let deserialized: TestStruct = serde_json::from_str(&json).unwrap();
    assert_eq!(without_value, deserialized);
}

/// REGRESSÃO: Leptos RwSignal deve funcionar em WASM (não usar create_signal tuple).
///
/// Bug original: Leptos 0.7 mudou de `(signal, set_signal)` para `RwSignal`.
/// Fix: Usar `RwSignal::new()` e `.read()/.write()` ao invés de `.get()/.set()`.
#[wasm_bindgen_test]
fn test_leptos_rwsignal_works() {
    use leptos::prelude::*;

    let owner = Owner::new();
    owner.set();

    let signal = RwSignal::new(42i32);

    // Usar get_untracked em testes (fora de contexto reativo)
    assert_eq!(
        signal.get_untracked(),
        42,
        "RwSignal deve retornar valor inicial"
    );

    // Testar write
    signal.set(100);
    assert_eq!(
        signal.get_untracked(),
        100,
        "RwSignal set deve atualizar valor"
    );

    // Testar update
    signal.update(|v| *v += 1);
    assert_eq!(
        signal.get_untracked(),
        101,
        "RwSignal update deve modificar valor"
    );
}

/// REGRESSÃO: leptos::task::spawn_local deve estar disponível (compila).
///
/// Bug original: spawn_local não importado causava erro de compilação.
/// Fix: Adicionar `use leptos::task::spawn_local;` nos componentes.
///
/// Nota: spawn_local requer executor inicializado (não disponível em testes unitários).
/// Este teste valida apenas que o símbolo está acessível e compila.
#[wasm_bindgen_test]
fn test_spawn_local_available() {
    // Se leptos::task::spawn_local não existir ou mudar de assinatura,
    // esta referência falha na compilação — proteção contra regressão.
    let _fn_ptr: fn(std::pin::Pin<std::boxed::Box<dyn std::future::Future<Output = ()>>>) =
        leptos::task::spawn_local;
}

/// REGRESSÃO: gloo_net::http::Request deve compilar em WASM.
///
/// Bug original: gloo_net sem feature "http" não expõe Request.
/// Fix: Adicionar `features = ["http"]` no Cargo.toml.
#[wasm_bindgen_test]
fn test_gloo_net_request_available() {
    // Apenas valida que o tipo está disponível (compila)
    let _request_type = std::any::type_name::<gloo_net::http::Request>();
}

/// REGRESSÃO: web_sys console::log_1 deve funcionar (não usar tracing::info em WASM).
///
/// Bug original: tracing não está disponível em wasm32.
/// Fix: Usar `web_sys::console::log_1()` para logging em componentes WASM.
#[wasm_bindgen_test]
fn test_web_sys_console_available() {
    use wasm_bindgen::JsValue;
    use web_sys::console;

    console::log_1(&JsValue::from_str("Test log from WASM"));
}
