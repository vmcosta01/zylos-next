//! Autenticação do cockpit: token persistido em `localStorage['zylos_token']`.
//!
//! - chamadas HTTP: `Authorization: Bearer <token>` quando houver token;
//! - WebSocket/EventSource (não aceitam headers): `?access_token=<token url-encoded>`;
//! - respostas 401/403 em ações de aprovação → `ADMIN_LOGIN_MSG`.

/// Chave do token no `localStorage`.
pub const TOKEN_KEY: &str = "zylos_token";

/// Mensagem exibida quando uma aprovação é negada por falta de credencial.
pub const ADMIN_LOGIN_MSG: &str = "Faça login como administrador para aprovar";

/// `true` para respostas de autenticação/autorização negada.
pub fn is_auth_denied(status: u16) -> bool {
    status == 401 || status == 403
}

/// Converte um status HTTP de falha em mensagem para ações de aprovação.
pub fn approval_error(status: u16) -> String {
    if is_auth_denied(status) {
        ADMIN_LOGIN_MSG.to_string()
    } else {
        format!("HTTP {status}")
    }
}

/// Percent-encoding (RFC 3986, conjunto "unreserved" preservado).
pub fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Acrescenta `access_token` à URL (respeitando `?` existente). Sem token → URL intacta.
pub fn with_access_token(url: &str, token: Option<&str>) -> String {
    match token.map(str::trim).filter(|t| !t.is_empty()) {
        Some(t) => {
            let sep = if url.contains('?') { '&' } else { '?' };
            format!("{url}{sep}access_token={}", url_encode(t))
        }
        None => url.to_string(),
    }
}

/// Decodifica base64url (sem padding) — usado só para ler o payload do JWT.
fn b64url_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'-' | b'+' => 62,
            b'_' | b'/' => 63,
            b'=' => break,
            _ => return None,
        } as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// `sub` do JWT (sem validar assinatura — quem valida é o gateway).
pub fn jwt_subject(token: &str) -> Option<String> {
    let payload = token.split('.').nth(1)?;
    let bytes = b64url_decode(payload)?;
    let v: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    v.get("sub")?
        .as_str()
        .filter(|s| !s.is_empty())
        .map(String::from)
}

/// Identidade do usuário atual: `sub` do token, ou `"local"` sem login.
pub fn current_user() -> String {
    token()
        .as_deref()
        .and_then(jwt_subject)
        .unwrap_or_else(|| "local".to_string())
}

/// Valor do header `Authorization`.
pub fn bearer(token: &str) -> String {
    format!("Bearer {token}")
}

#[cfg(target_arch = "wasm32")]
fn storage() -> Option<web_sys::Storage> {
    web_sys::window().and_then(|w| w.local_storage().ok().flatten())
}

/// Token atual (se houver).
#[cfg(target_arch = "wasm32")]
pub fn token() -> Option<String> {
    storage()
        .and_then(|s| s.get_item(TOKEN_KEY).ok().flatten())
        .filter(|t| !t.trim().is_empty())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn token() -> Option<String> {
    None
}

/// Persiste o token.
pub fn set_token(_token: &str) {
    #[cfg(target_arch = "wasm32")]
    if let Some(s) = storage() {
        let _ = s.set_item(TOKEN_KEY, _token);
    }
}

/// Remove o token (logout).
pub fn clear_token() {
    #[cfg(target_arch = "wasm32")]
    if let Some(s) = storage() {
        let _ = s.remove_item(TOKEN_KEY);
    }
}

/// URL de WebSocket/EventSource com o token atual anexado.
pub fn authed_url(url: &str) -> String {
    with_access_token(url, token().as_deref())
}

/// Anexa `Authorization: Bearer` a um request `gloo_net` quando houver token.
pub fn authed(req: gloo_net::http::RequestBuilder) -> gloo_net::http::RequestBuilder {
    match token() {
        Some(t) => req.header("Authorization", &bearer(&t)),
        None => req,
    }
}

/// Anexa `Authorization: Bearer` a um `web_sys::Request` quando houver token.
pub fn authed_web_request(req: &web_sys::Request) {
    if let Some(t) = token() {
        let _ = req.headers().set("Authorization", &bearer(&t));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_encode_escapa_reservados() {
        assert_eq!(url_encode("a.b-c_d~"), "a.b-c_d~");
        assert_eq!(url_encode("a+b/c=d e"), "a%2Bb%2Fc%3Dd%20e");
    }

    #[test]
    fn access_token_respeita_query_existente() {
        assert_eq!(
            with_access_token("ws://h/api/v1/crdt/sync/d", Some("t+1")),
            "ws://h/api/v1/crdt/sync/d?access_token=t%2B1"
        );
        assert_eq!(
            with_access_token("http://h/e?x=1", Some("abc")),
            "http://h/e?x=1&access_token=abc"
        );
        assert_eq!(with_access_token("http://h/e", None), "http://h/e");
        assert_eq!(with_access_token("http://h/e", Some("  ")), "http://h/e");
    }

    #[test]
    fn aprovacao_negada_pede_login_admin() {
        assert_eq!(approval_error(401), ADMIN_LOGIN_MSG);
        assert_eq!(approval_error(403), ADMIN_LOGIN_MSG);
        assert_eq!(approval_error(500), "HTTP 500");
        assert_eq!(bearer("x"), "Bearer x");
    }

    #[test]
    fn jwt_subject_le_sub_do_payload() {
        // {"sub":"ana"} em base64url sem padding (montado em partes)
        let tok = format!("h.{}{}.s", "eyJzdWIi", "OiJhbmEifQ");
        assert_eq!(jwt_subject(&tok).as_deref(), Some("ana"));
        assert_eq!(jwt_subject("lixo"), None);
        assert_eq!(current_user(), "local");
    }
}
