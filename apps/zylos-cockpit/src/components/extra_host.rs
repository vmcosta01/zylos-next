//! Host de painéis carregados sob demanda (módulo wasm "extras", ver `src/bin/extras.rs`).
//!
//! Ao montar, carrega `extras.js` + `extras_bg.wasm` (uma vez por página, via
//! `extras-loader.js`) e monta o painel da aba num elemento próprio. Ao desmontar, libera a
//! instância. Estados: carregando (esqueleto), erro (com Tentar novamente) e pronto.

use leptos::html::Div;
use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::{spawn_local, JsFuture};

use crate::tabs::Tab;

#[wasm_bindgen]
extern "C" {
    /// Definida por `extras-loader.js`: carrega o módulo e devolve a Promise do objeto de exports.
    #[wasm_bindgen(js_namespace = window, js_name = __zylosLoadExtras, catch)]
    fn load_extras() -> Result<js_sys::Promise, JsValue>;
}

#[derive(Debug, Clone, PartialEq)]
enum HostState {
    Loading,
    Ready,
    Failed(String),
}

/// Carrega o módulo (se preciso) e monta o painel `key` em `parent`; devolve o handle (`free()` desmonta).
async fn mount_panel(key: &str, parent: web_sys::HtmlElement) -> Result<JsValue, String> {
    let promise = load_extras().map_err(|e| format!("{e:?}"))?;
    let exports = JsFuture::from(promise)
        .await
        .map_err(|e| format!("{e:?}"))?;
    let mount = js_sys::Reflect::get(&exports, &JsValue::from_str("mount_extra"))
        .map_err(|e| format!("{e:?}"))?
        .dyn_into::<js_sys::Function>()
        .map_err(|_| "mount_extra não encontrado no módulo extras".to_string())?;
    mount
        .call2(&exports, &JsValue::from_str(key), &parent)
        .map_err(|e| format!("{e:?}"))
}

/// Mostra o painel da aba `tab`, montado pelo módulo extras.
#[component]
pub fn ExtraHost(tab: Tab) -> impl IntoView {
    let mount_point = NodeRef::<Div>::new();
    let (state, set_state) = signal(HostState::Loading);
    let (attempt, set_attempt) = signal(0u32);
    let handle: StoredValue<Option<JsValue>, LocalStorage> = StoredValue::new_local(None);
    let disposed = StoredValue::new(false);

    Effect::new(move |_| {
        let _ = attempt.get();
        let Some(parent) = mount_point.get() else {
            return;
        };
        set_state.set(HostState::Loading);
        let parent: web_sys::HtmlElement = (*parent).clone();
        spawn_local(async move {
            match mount_panel(tab.css_class(), parent).await {
                Ok(h) if disposed.get_value() => free(&h),
                Ok(h) => {
                    handle.set_value(Some(h));
                    set_state.set(HostState::Ready);
                }
                Err(e) => set_state.set(HostState::Failed(e)),
            }
        });
    });

    on_cleanup(move || {
        disposed.set_value(true);
        if let Some(h) = handle.get_value() {
            free(&h);
        }
    });

    view! {
        <div class="extra-host">
            {move || match state.get() {
                HostState::Loading => view! {
                    <div class="skeleton-grid" role="status" aria-label="Carregando painel">
                        <div class="skeleton-card pulse"></div>
                        <div class="skeleton-card pulse"></div>
                    </div>
                }.into_any(),
                HostState::Failed(e) => view! {
                    <div class="error-banner" role="alert">
                        // O detalhe técnico vai só para o console (report); a tela mostra uma frase acionável.
                        <p>{{ let _ = crate::ui_error::report("Painel", &e); "Não foi possível carregar este painel. Verifique a conexão e tente novamente." }}</p>
                        <button class="cta-retry" on:click=move |_| set_attempt.update(|n| *n += 1)>"Tentar novamente"</button>
                    </div>
                }.into_any(),
                HostState::Ready => ().into_any(),
            }}
            <div class="extra-mount" node_ref=mount_point></div>
        </div>
    }
}

/// Como [`ExtraHost`], mas remonta o painel quando o Space ativo muda: o módulo extras não
/// recebe o contexto de Spaces, então o painel relê o Space escolhido ao montar.
#[component]
pub fn SpaceScopedHost(tab: Tab) -> impl IntoView {
    let spaces = crate::spaces::use_spaces();
    move || {
        if let Some(s) = spaces {
            let _ = s.active_id.get();
        }
        view! { <ExtraHost tab=tab /> }
    }
}

/// Desmonta a instância no módulo extras (`ExtraHandle.free()`).
fn free(handle: &JsValue) {
    if let Ok(f) = js_sys::Reflect::get(handle, &JsValue::from_str("free")) {
        if let Ok(f) = f.dyn_into::<js_sys::Function>() {
            let _ = f.call0(handle);
        }
    }
}
