//! Resolução da URL base do gateway (T11.5.6).
//!
//! Ordem de resolução:
//! 1. `<meta name="zylos-gateway-url" content="https://gateway.exemplo">` no `index.html`
//!    (vazio = não configurado);
//! 2. se a página é servida de `localhost`/`127.0.0.1`, usa `http://localhost:3000` (dev);
//! 3. caso contrário, `None` — o cockpit não dispara requisições para `localhost:3000`
//!    a partir de um domínio público (isso gerava `net::ERR_CONNECTION_REFUSED` no console
//!    e derrubava o Lighthouse Best Practices no preview do Cloudflare Pages).

/// URL padrão de desenvolvimento.
pub const DEV_GATEWAY_URL: &str = "http://localhost:3000";

/// Decide a URL base a partir do conteúdo do meta tag e do hostname da página.
/// Função pura para ser testável fora do navegador.
pub fn resolve_gateway_base(meta: Option<&str>, hostname: &str) -> Option<String> {
    if let Some(m) = meta.map(str::trim).filter(|m| !m.is_empty()) {
        return Some(m.trim_end_matches('/').to_string());
    }
    match hostname {
        "localhost" | "127.0.0.1" | "[::1]" | "::1" => Some(DEV_GATEWAY_URL.to_string()),
        _ => None,
    }
}

/// Junta base + caminho (`/api/v1/...`). Se `endpoint` já é absoluto, devolve como está.
pub fn join_url(base: &str, endpoint: &str) -> String {
    if endpoint.starts_with("http://") || endpoint.starts_with("https://") {
        endpoint.to_string()
    } else {
        format!("{}{}", base.trim_end_matches('/'), endpoint)
    }
}

/// Lê a configuração do documento atual (somente em wasm32).
#[cfg(target_arch = "wasm32")]
pub fn gateway_base() -> Option<String> {
    let window = web_sys::window()?;
    let hostname = window.location().hostname().unwrap_or_default();
    let meta = window
        .document()
        .and_then(|d| {
            d.query_selector("meta[name=\"zylos-gateway-url\"]")
                .ok()
                .flatten()
        })
        .and_then(|el| el.get_attribute("content"));
    resolve_gateway_base(meta.as_deref(), &hostname)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn gateway_base() -> Option<String> {
    Some(DEV_GATEWAY_URL.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meta_tem_prioridade() {
        assert_eq!(
            resolve_gateway_base(Some("https://gw.zylos.dev/"), "cockpit.zylos.dev"),
            Some("https://gw.zylos.dev".to_string())
        );
    }

    #[test]
    fn localhost_usa_dev() {
        assert_eq!(
            resolve_gateway_base(None, "localhost"),
            Some(DEV_GATEWAY_URL.to_string())
        );
        assert_eq!(
            resolve_gateway_base(Some("  "), "127.0.0.1"),
            Some(DEV_GATEWAY_URL.to_string())
        );
    }

    #[test]
    fn dominio_publico_sem_meta_nao_configura() {
        assert_eq!(resolve_gateway_base(None, "x.pages.dev"), None);
        assert_eq!(resolve_gateway_base(Some(""), "x.pages.dev"), None);
    }

    #[test]
    fn join_url_respeita_absoluto() {
        assert_eq!(
            join_url("https://o.dev", "http://localhost:3000/api/v1/events"),
            "http://localhost:3000/api/v1/events"
        );
        assert_eq!(
            join_url("https://gw.dev/", "/api/v1/events"),
            "https://gw.dev/api/v1/events"
        );
    }
}
