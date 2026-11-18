//! Chamadas HTTP ao gateway com `Authorization: Bearer` (via [`crate::auth`]) e URL
//! resolvida por [`crate::gateway_config::gateway_base`] + [`crate::gateway_config::join_url`].
//!
//! Erros saem como texto curto (`HTTP 404`, `gateway não configurado`, ...) — passe por
//! [`crate::ui_error::user_message`] antes de mostrar ao usuário.

use gloo_net::http::{Request, RequestBuilder, Response};
use serde::{de::DeserializeOwned, Serialize};

/// Mensagem quando o cockpit não sabe onde está o gateway.
pub const NO_GATEWAY: &str = "gateway não configurado";

/// URL absoluta para `path` (`/api/v1/...`) ou erro se o gateway não está configurado.
pub fn api_url(path: &str) -> Result<String, String> {
    let base = crate::gateway_config::gateway_base().ok_or_else(|| NO_GATEWAY.to_string())?;
    Ok(crate::gateway_config::join_url(&base, path))
}

async fn finish<T: DeserializeOwned>(resp: Response) -> Result<T, String> {
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json::<T>().await.map_err(|e| format!("{e:?}"))
}

async fn send_json<B: Serialize, T: DeserializeOwned>(
    req: RequestBuilder,
    body: &B,
) -> Result<T, String> {
    let resp = crate::auth::authed(req)
        .header("Content-Type", "application/json")
        .body(serde_json::to_string(body).map_err(|e| e.to_string())?)
        .map_err(|e| format!("{e:?}"))?
        .send()
        .await
        .map_err(|e| format!("{e:?}"))?;
    finish(resp).await
}

/// `GET path` → JSON.
pub async fn get_json<T: DeserializeOwned>(path: &str) -> Result<T, String> {
    let resp = crate::auth::authed(Request::get(&api_url(path)?))
        .send()
        .await
        .map_err(|e| format!("{e:?}"))?;
    finish(resp).await
}

/// `POST path` com corpo JSON → JSON.
pub async fn post_json<B: Serialize, T: DeserializeOwned>(path: &str, body: &B) -> Result<T, String> {
    send_json(Request::post(&api_url(path)?), body).await
}

/// `DELETE path` com corpo JSON → JSON.
pub async fn delete_json<B: Serialize, T: DeserializeOwned>(
    path: &str,
    body: &B,
) -> Result<T, String> {
    send_json(Request::delete(&api_url(path)?), body).await
}
