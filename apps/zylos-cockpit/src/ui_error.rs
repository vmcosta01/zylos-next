//! Mensagens de erro para o usuário.
//!
//! Os erros internos costumam carregar `format!("{:?}", JsValue)`, com stack
//! trace JS/WASM (`TypeError: Failed to fetch at ... wasm-function[123]`).
//! O banner mostra só uma frase curta; o detalhe completo vai para o console.

/// Mensagem genérica quando o erro não é reconhecido.
pub const GENERIC_MSG: &str = "Não foi possível falar com o gateway.";

const MAX_LEN: usize = 120;

/// `true` se o texto parece stack trace / dump de `JsValue`.
fn looks_technical(raw: &str) -> bool {
    const MARKERS: [&str; 10] = [
        "JsValue",
        "TypeError",
        "Error:",
        "wasm-function",
        ".wasm",
        "stack",
        " at ",
        "panicked",
        "{",
        "\n",
    ];
    MARKERS.iter().any(|m| raw.contains(m))
}

/// Converte um erro técnico em mensagem curta e amigável (função pura).
pub fn user_message(raw: &str) -> String {
    let t = raw.trim();
    if let Some(code) = t.strip_prefix("HTTP ").and_then(|c| c.parse::<u16>().ok()) {
        return match code {
            401 | 403 => "Acesso negado. Faça login.".to_string(),
            404 => "Recurso não encontrado no gateway (HTTP 404).".to_string(),
            500..=599 => format!("O gateway falhou (HTTP {code}). Tente novamente."),
            _ => format!("O gateway respondeu HTTP {code}."),
        };
    }
    if t.contains("Failed to fetch") || t.contains("NetworkError") || t.contains("fetch error") {
        return "Gateway indisponível. Verifique a conexão.".to_string();
    }
    if t.is_empty() || looks_technical(t) || t.chars().count() > MAX_LEN {
        return GENERIC_MSG.to_string();
    }
    t.to_string()
}

/// Loga o detalhe no console (`console.error`) e devolve a mensagem amigável.
pub fn report(context: &str, raw: &str) -> String {
    #[cfg(target_arch = "wasm32")]
    web_sys::console::error_1(&format!("[zylos-cockpit] {context}: {raw}").into());
    #[cfg(not(target_arch = "wasm32"))]
    let _ = context;
    user_message(raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stack_trace_nao_vaza() {
        let raw = "JsValue(TypeError: Failed to fetch\nTypeError: Failed to fetch\n    at http://x/zylos_cockpit.js:1:2 wasm-function[812]:0x1f00)";
        let m = user_message(raw);
        assert!(!m.contains("wasm") && !m.contains(" at ") && !m.contains('\n'));
        assert_eq!(m, "Gateway indisponível. Verifique a conexão.");
        assert_eq!(
            user_message("JsValue(Object({\"x\":1}))"),
            GENERIC_MSG.to_string()
        );
        assert_eq!(user_message(&"x".repeat(500)), GENERIC_MSG.to_string());
    }

    #[test]
    fn mensagens_curtas_passam() {
        assert_eq!(
            user_message("HTTP 503"),
            "O gateway falhou (HTTP 503). Tente novamente."
        );
        assert_eq!(user_message("HTTP 401"), "Acesso negado. Faça login.");
        assert_eq!(
            user_message("Gateway não configurado (meta zylos-gateway-url)"),
            "Gateway não configurado (meta zylos-gateway-url)"
        );
        assert_eq!(report("t", "HTTP 418"), "O gateway respondeu HTTP 418.");
    }
}
