//! Trava "só desta máquina" do gateway.
//!
//! O gateway escuta em 127.0.0.1, o que o protege da rede, mas não do
//! navegador: qualquer página aberta pode mandar pedidos para
//! `http://127.0.0.1:3000`. Esta trava responde 403 em dois casos:
//!
//! 1. **Pedido de navegador vindo de outro site.** O cabeçalho `Origin` está
//!    presente e não é localhost / 127.0.0.1 / [::1]. Navegadores sempre mandam
//!    `Origin` em POST e em pedidos entre sites; clientes que não são navegador
//!    (zylos-cli, plugin do Hermes) não mandam e passam. `Origin: null`
//!    (página aberta de arquivo, iframe isolado) também é recusado.
//! 2. **DNS rebinding.** O `Host` (ou a autoridade da URI) não é um nome desta
//!    máquina. Um site que aponta o próprio domínio para 127.0.0.1 continua
//!    mandando o domínio dele no `Host`.
//!
//! O que continua passando de propósito: GET sem `Origin` (ex.: `<img src>` de
//! outro site). Esse pedido chega, mas o navegador não deixa o site ler a
//! resposta, porque o CORS só libera origens desta máquina
//! ([`cors_origem_local`]). Nenhuma rota GET altera estado.

use axum::{
    extract::Request,
    http::{header, request::Parts, HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};

/// `true` se o nome (já sem porta) é desta máquina.
fn nome_local(nome: &str) -> bool {
    let n = nome.trim().trim_end_matches('.').to_ascii_lowercase();
    n == "localhost" || n == "127.0.0.1" || n == "::1"
}

/// Tira a porta de `nome[:porta]` ou `[ipv6][:porta]`. `None` se a forma é inválida.
fn sem_porta(autoridade: &str) -> Option<&str> {
    let a = autoridade.trim();
    if a.is_empty() || a.contains('@') {
        // userinfo (`user@host`) nunca vem num Host ou Origin legítimo
        return None;
    }
    if let Some(resto) = a.strip_prefix('[') {
        let (ip, depois) = resto.split_once(']')?;
        return match depois.strip_prefix(':') {
            Some(porta) if !porta.is_empty() && porta.chars().all(|c| c.is_ascii_digit()) => Some(ip),
            None if depois.is_empty() => Some(ip),
            _ => None,
        };
    }
    match a.rsplit_once(':') {
        Some((nome, porta)) if !porta.is_empty() && porta.chars().all(|c| c.is_ascii_digit()) => Some(nome),
        Some(_) => None,
        None => Some(a),
    }
}

/// `Host` ou autoridade desta máquina (`127.0.0.1:3000`, `localhost`, `[::1]:3000`).
pub fn autoridade_local(autoridade: &str) -> bool {
    sem_porta(autoridade).map(nome_local).unwrap_or(false)
}

/// `Origin` desta máquina: só `http://` ou `https://` com host local.
pub fn origem_local(origem: &str) -> bool {
    let o = origem.trim();
    match o.strip_prefix("http://").or_else(|| o.strip_prefix("https://")) {
        Some(resto) => autoridade_local(resto.split('/').next().unwrap_or("")),
        None => false,
    }
}

/// Predicado de CORS: só origens desta máquina recebem `Access-Control-Allow-Origin`.
pub fn cors_origem_local(origem: &HeaderValue, _parts: &Parts) -> bool {
    origem.to_str().map(origem_local).unwrap_or(false)
}

fn recusa(detalhe: &str, instance: &str) -> Response {
    let body = serde_json::json!({
        "type": "https://zylos.dev/errors/fora-desta-maquina",
        "title": "Pedido recusado: o gateway só atende esta máquina",
        "status": 403,
        "detail": detalhe,
        "instance": instance,
    });
    (
        StatusCode::FORBIDDEN,
        [(header::CONTENT_TYPE, "application/problem+json")],
        Json(body),
    )
        .into_response()
}

/// Middleware: recusa `Origin` de fora e `Host` que não é desta máquina.
pub async fn so_desta_maquina(req: Request, next: Next) -> Response {
    let instance = req.uri().path().to_string();

    if let Some(origem) = req.headers().get(header::ORIGIN) {
        if !origem.to_str().map(origem_local).unwrap_or(false) {
            return recusa("o cabeçalho Origin não é desta máquina", &instance);
        }
    }

    let host_ok = match req.headers().get(header::HOST) {
        Some(h) => h.to_str().map(autoridade_local).unwrap_or(false),
        None => req
            .uri()
            .authority()
            .map(|a| autoridade_local(a.as_str()))
            .unwrap_or(true), // sem Host nem autoridade: não é navegador
    };
    if !host_ok {
        return recusa("o cabeçalho Host não é desta máquina", &instance);
    }

    next.run(req).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autoridades_locais_passam() {
        for a in ["127.0.0.1:3000", "127.0.0.1", "localhost", "LOCALHOST:3001", "localhost.:80", "[::1]:3000", "[::1]"] {
            assert!(autoridade_local(a), "devia ser local: {a}");
        }
    }

    #[test]
    fn autoridades_de_fora_ou_invalidas_sao_recusadas() {
        for a in [
            "evil.example:3000",
            "localhost.evil.example",
            "127.0.0.1.nip.io:3000",
            "user@localhost:3000",
            "localhost:abc",
            "localhost:",
            "[::1]x",
            "[::1]:",
            "[::2]:3000",
            "::1",
            "",
        ] {
            assert!(!autoridade_local(a), "devia ser recusada: {a:?}");
        }
    }

    #[test]
    fn origens_locais_passam() {
        for o in ["http://localhost:3001", "https://127.0.0.1", "http://[::1]:5173", "http://localhost"] {
            assert!(origem_local(o), "devia ser local: {o}");
        }
    }

    #[test]
    fn origens_de_fora_sao_recusadas() {
        for o in [
            "null",
            "https://evil.example",
            "http://localhost.evil.example",
            "http://127.0.0.1.nip.io",
            "file://",
            "chrome-extension://abcdef",
            "http://user@localhost",
            "",
        ] {
            assert!(!origem_local(o), "devia ser recusada: {o:?}");
        }
    }
}
