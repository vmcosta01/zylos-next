//! Spaces (APP_FLOW v2.1 §2): grupos de agentes que filtram Inbox, aprovações no chat e bots.
//!
//! O Space ativo vem do gateway (`/api/v1/spaces`); o escolhido fica em `localStorage`
//! (`zylos-active-space`). O "Geral" é aberto: todos os agentes passam.

use leptos::prelude::*;
use serde::Deserialize;

/// Space como devolvido pelo gateway.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SpaceView {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub open_access: bool,
    #[serde(default)]
    pub agents: Vec<String>,
}

/// Id do Space padrão (aberto).
pub const GENERAL_ID: &str = "geral";
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
const STORAGE_KEY: &str = "zylos-active-space";

/// O agente tem acesso ao Space? Sem Space carregado, tudo passa (não esconde dados por falha de rede).
pub fn space_allows(space: Option<&SpaceView>, agent: &str) -> bool {
    match space {
        None => true,
        Some(s) => s.open_access || s.agents.iter().any(|a| a == agent),
    }
}

/// Estado global dos Spaces, fornecido por contexto.
#[derive(Clone, Copy)]
pub struct SpacesCtx {
    pub list: RwSignal<Vec<SpaceView>>,
    pub active_id: RwSignal<String>,
}

impl SpacesCtx {
    /// Space ativo (rastreia `list` e `active_id`).
    pub fn active(&self) -> Option<SpaceView> {
        let id = self.active_id.get();
        self.list.get().into_iter().find(|s| s.id == id)
    }

    /// O agente passa no Space ativo? (rastreia os sinais, serve dentro de views e effects)
    pub fn allows(&self, agent: &str) -> bool {
        space_allows(self.active().as_ref(), agent)
    }

    /// Escolhe o Space ativo e guarda a escolha.
    pub fn select(&self, id: &str) {
        self.active_id.set(id.to_string());
        #[cfg(target_arch = "wasm32")]
        if let Some(s) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
            let _ = s.set_item(STORAGE_KEY, id);
        }
    }

    /// Recarrega a lista do gateway; se o Space ativo sumiu, volta ao "Geral".
    pub fn reload(&self) {
        let ctx = *self;
        wasm_bindgen_futures::spawn_local(async move {
            if let Ok(list) = fetch_spaces().await {
                let id = ctx.active_id.get_untracked();
                if !list.iter().any(|s| s.id == id) {
                    ctx.select(GENERAL_ID);
                }
                ctx.list.set(list);
            }
        });
    }
}

fn stored_active() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(v) = web_sys::window()
            .and_then(|w| w.local_storage().ok().flatten())
            .and_then(|s| s.get_item(STORAGE_KEY).ok().flatten())
        {
            return v;
        }
    }
    GENERAL_ID.to_string()
}

/// Cria o contexto, carrega os Spaces e o fornece à árvore de componentes.
pub fn provide_spaces() -> SpacesCtx {
    let ctx = SpacesCtx {
        list: RwSignal::new(Vec::new()),
        active_id: RwSignal::new(stored_active()),
    };
    provide_context(ctx);
    ctx.reload();
    ctx
}

/// Lê o contexto (None em testes e previews).
pub fn use_spaces() -> Option<SpacesCtx> {
    use_context::<SpacesCtx>()
}

#[derive(Deserialize)]
struct ListResp {
    spaces: Vec<SpaceView>,
}

fn url(path: &str) -> Result<String, String> {
    let base = crate::gateway_config::gateway_base().ok_or("Gateway não configurado")?;
    Ok(crate::gateway_config::join_url(&base, path))
}

async fn expect_ok(resp: gloo_net::http::Response) -> Result<gloo_net::http::Response, String> {
    if resp.ok() {
        return Ok(resp);
    }
    // O gateway devolve `{ "error": "..." }` com mensagem em português.
    let status = resp.status();
    let msg = resp
        .json::<serde_json::Value>()
        .await
        .ok()
        .and_then(|v| v["error"].as_str().map(str::to_string));
    Err(msg.unwrap_or_else(|| format!("HTTP {status}")))
}

pub async fn fetch_spaces() -> Result<Vec<SpaceView>, String> {
    let resp = crate::auth::authed(gloo_net::http::Request::get(&url("/api/v1/spaces")?))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let resp = expect_ok(resp).await?;
    Ok(resp
        .json::<ListResp>()
        .await
        .map_err(|e| e.to_string())?
        .spaces)
}

/// Cria um Space e devolve o criado.
pub async fn create_space(name: &str) -> Result<SpaceView, String> {
    let resp = crate::auth::authed(gloo_net::http::Request::post(&url("/api/v1/spaces")?))
        .json(&serde_json::json!({ "name": name }))
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    expect_ok(resp)
        .await?
        .json()
        .await
        .map_err(|e| e.to_string())
}

/// Define a lista de agentes com acesso.
pub async fn set_agents(id: &str, agents: &[String]) -> Result<(), String> {
    let resp = crate::auth::authed(gloo_net::http::Request::put(&url(&format!(
        "/api/v1/spaces/{id}"
    ))?))
    .json(&serde_json::json!({ "open_access": false, "agents": agents }))
    .map_err(|e| e.to_string())?
    .send()
    .await
    .map_err(|e| e.to_string())?;
    expect_ok(resp).await.map(|_| ())
}

pub async fn delete_space(id: &str) -> Result<(), String> {
    let resp = crate::auth::authed(gloo_net::http::Request::delete(&url(&format!(
        "/api/v1/spaces/{id}"
    ))?))
    .send()
    .await
    .map_err(|e| e.to_string())?;
    expect_ok(resp).await.map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn space(open: bool, agents: &[&str]) -> SpaceView {
        SpaceView {
            id: "x".into(),
            name: "X".into(),
            description: String::new(),
            open_access: open,
            agents: agents.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn geral_aberto_deixa_todos_passarem() {
        assert!(space_allows(Some(&space(true, &[])), "qualquer"));
    }

    #[test]
    fn space_fechado_so_deixa_os_listados() {
        let s = space(false, &["chief"]);
        assert!(space_allows(Some(&s), "chief"));
        assert!(!space_allows(Some(&s), "backend"));
    }

    #[test]
    fn sem_space_carregado_nao_esconde_nada() {
        assert!(space_allows(None, "backend"));
    }
}
