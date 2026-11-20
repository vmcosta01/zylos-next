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
    // 204/205 não têm corpo (o DELETE do gateway responde assim): sucesso, não erro de JSON.
    if matches!(resp.status(), 204 | 205) {
        return sem_corpo();
    }
    resp.json::<T>().await.map_err(|e| format!("{e:?}"))
}

/// O valor de uma resposta sem corpo: `null` (serve para `serde_json::Value`, `Option<_>` e `()`).
fn sem_corpo<T: DeserializeOwned>() -> Result<T, String> {
    serde_json::from_value(serde_json::Value::Null)
        .map_err(|_| "a resposta veio sem corpo, mas esperava-se um valor".to_string())
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
pub async fn post_json<B: Serialize, T: DeserializeOwned>(
    path: &str,
    body: &B,
) -> Result<T, String> {
    send_json(Request::post(&api_url(path)?), body).await
}

/// `DELETE path` com corpo JSON → JSON.
pub async fn delete_json<B: Serialize, T: DeserializeOwned>(
    path: &str,
    body: &B,
) -> Result<T, String> {
    send_json(Request::delete(&api_url(path)?), body).await
}

#[cfg(test)]
mod testes_sem_corpo {
    use super::sem_corpo;

    #[test]
    fn resposta_sem_corpo_e_sucesso_para_value_option_e_unit() {
        assert_eq!(
            sem_corpo::<serde_json::Value>(),
            Ok(serde_json::Value::Null)
        );
        assert_eq!(sem_corpo::<Option<u32>>(), Ok(None));
        assert_eq!(sem_corpo::<()>(), Ok(()));
        // Um tipo que exige campos não existe sem corpo: erro claro, não pânico.
        assert!(sem_corpo::<Vec<u32>>().is_err());
    }
}
