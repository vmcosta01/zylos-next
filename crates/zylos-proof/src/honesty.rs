//! Motor de Honestidade Determinística — zero LLM calls.
//!
//! Implementa as funções de validação, normalização e decisão de entrega
//! conforme Task 7B.1.1 do master plan.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Teto máximo de itens em normalizar_lista().
pub const MAX_ITENS: usize = 1000;

/// Resultado de leitura_bool().
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BoolLeitura {
    Verdadeiro,
    Falso,
    Ininteligivel,
}

/// Resultado de decide_delivery().
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveryDecision {
    pub ok: bool,
    pub problema_provado: bool,
    pub motivo: Option<String>,
}

/// Relatório construído por build_report().
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HonestReport {
    pub titulo: String,
    pub conteudo_truncado: String,
    pub truncado: bool,
    pub tamanho_original: usize,
    pub itens_omitidos: usize,
    pub custo_real: f64,
    pub pct_aterrado: f64,
}

/// Manifesto de screenshot para checar_completude().
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenshotManifest {
    pub hash: String,
    pub timestamp: String,
    pub arquivo: String,
    pub width: u32,
    pub height: u32,
}

/// Resultado da verificação de completude de screenshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletudeResult {
    pub completa: bool,
    pub motivos: Vec<String>,
}

/// Calcula peso com clamp 1-5 e detecção de stakes malformado.
///
/// - Se `stakes` for NaN, infinito, negativo ou > 5 → suspeito=true
/// - Retorna (peso_clampado, suspeito)
pub fn peso_de(stakes: f64) -> (u8, bool) {
    let suspeito = stakes.is_nan()
        || stakes.is_infinite()
        || stakes < 0.0
        || stakes > 5.0;

    let clamped = if stakes.is_nan() || stakes.is_sign_negative() {
        1u8
    } else if stakes > 5.0 {
        5u8
    } else {
        stakes.round().clamp(1.0, 5.0) as u8
    };

    (clamped, suspeito)
}

/// Lê valor booleano de string com suporte a PT/EN/numérico.
///
/// Aceita: sim/não/true/false/1/0 (case-insensitive, trim).
/// Qualquer outro valor → Ininteligivel.
pub fn leitura_bool(valor: &str) -> BoolLeitura {
    match valor.trim().to_lowercase().as_str() {
        "sim" | "true" | "1" | "yes" | "s" => BoolLeitura::Verdadeiro,
        "não" | "nao" | "false" | "0" | "no" | "n" => BoolLeitura::Falso,
        _ => BoolLeitura::Ininteligivel,
    }
}

/// Normaliza lista com teto MAX_ITENS e contagem de omitidos.
///
/// Retorna (lista_truncada, total_omitidos).
pub fn normalizar_lista<T: Clone>(itens: &[T]) -> (Vec<T>, usize) {
    if itens.len() <= MAX_ITENS {
        (itens.to_vec(), 0)
    } else {
        (itens[..MAX_ITENS].to_vec(), itens.len() - MAX_ITENS)
    }
}

/// Acesso seguro a campo via closure — protege contra getters hostis.
///
/// Se o getter panics ou retorna erro, devolve None em vez de crashar.
pub fn campo_ex<T, F>(getter: F) -> Option<T>
where
    F: FnOnce() -> T + std::panic::UnwindSafe,
{
    std::panic::catch_unwind(getter).ok()
}

/// Versão alternativa que aceita Result do getter.
pub fn campo_ex_result<T, E, F>(getter: F) -> Option<T>
where
    F: FnOnce() -> Result<T, E>,
{
    getter().ok()
}

/// Decide entrega com lógica de achado negativo bloqueante.
///
/// Se `ok` é false E há evidência aterrada → problemaProvado=true, ok=false.
/// Nunca entrega ok=true quando há achado negativo aterrado.
pub fn decide_delivery(ok: bool, achado_negativo_aterrado: bool, motivo: Option<String>) -> DeliveryDecision {
    if !ok && achado_negativo_aterrado {
        DeliveryDecision {
            ok: false,
            problema_provado: true,
            motivo: motivo.or_else(|| Some("achado negativo aterrado".into())),
        }
    } else if !ok {
        DeliveryDecision {
            ok: false,
            problema_provado: false,
            motivo,
        }
    } else {
        // ok=true só se NÃO há achado negativo aterrado
        if achado_negativo_aterrado {
            DeliveryDecision {
                ok: false,
                problema_provado: true,
                motivo: Some("ok=true contradiz achado negativo aterrado".into()),
            }
        } else {
            DeliveryDecision {
                ok: true,
                problema_provado: false,
                motivo: None,
            }
        }
    }
}

/// Constrói relatório com truncamento honesto e métricas de custo.
///
/// - `conteudo`: texto completo do relatório
/// - `max_chars`: limite de caracteres antes de truncar
/// - `total_itens`: número total de itens analisados
/// - `itens_aterrados`: quantos têm ground >= Confirmed
/// - `custo`: custo real em tokens/dinheiro
pub fn build_report(
    titulo: impl Into<String>,
    conteudo: &str,
    max_chars: usize,
    total_itens: usize,
    itens_aterrados: usize,
    custo: f64,
) -> HonestReport {
    let tamanho_original = conteudo.len();
    let (truncado, conteudo_final) = if tamanho_original > max_chars {
        (true, format!("{}…[TRUNCADO {} chars omitidos]", &conteudo[..max_chars], tamanho_original - max_chars))
    } else {
        (false, conteudo.to_string())
    };

    let pct_aterrado = if total_itens == 0 {
        0.0
    } else {
        itens_aterrados as f64 / total_itens as f64 * 100.0
    };

    HonestReport {
        titulo: titulo.into(),
        conteudo_truncado: conteudo_final,
        truncado,
        tamanho_original,
        itens_omitidos: if truncado { tamanho_original - max_chars } else { 0 },
        custo_real: custo,
        pct_aterrado,
    }
}

/// Verifica completude de screenshot: manifesto + hash + timestamp.
///
/// Retorna Completa apenas se todos os campos estiverem presentes e válidos.
pub fn checar_completude(manifesto: &ScreenshotManifest, conteudo_bytes: &[u8]) -> CompletudeResult {
    let mut motivos = Vec::new();

    // Verifica hash
    let mut hasher = Sha256::new();
    hasher.update(conteudo_bytes);
    let hash_calculado = format!("{:x}", hasher.finalize());
    if hash_calculado != manifesto.hash {
        motivos.push(format!(
            "hash mismatch: esperado={}, calculado={}",
            manifesto.hash, hash_calculado
        ));
    }

    // Verifica timestamp não-vazio
    if manifesto.timestamp.trim().is_empty() {
        motivos.push("timestamp vazio".into());
    }

    // Verifica arquivo não-vazio
    if manifesto.arquivo.trim().is_empty() {
        motivos.push("arquivo vazio".into());
    }

    // Verifica dimensões razoáveis
    if manifesto.width == 0 || manifesto.height == 0 {
        motivos.push(format!(
            "dimensões inválidas: {}x{}",
            manifesto.width, manifesto.height
        ));
    }

    CompletudeResult {
        completa: motivos.is_empty(),
        motivos,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // === peso_de ===

    #[test]
    fn test_peso_de_normal() {
        assert_eq!(peso_de(3.0), (3, false));
        assert_eq!(peso_de(1.0), (1, false));
        assert_eq!(peso_de(5.0), (5, false));
    }

    #[test]
    fn test_peso_de_clamp_baixo() {
        // 0.0 rounds to 0 → clamped to 1, but is NOT suspicious (not NaN/neg/inf/>5)
        assert_eq!(peso_de(0.0), (1, false));
        assert_eq!(peso_de(-1.0), (1, true));
    }

    #[test]
    fn test_peso_de_clamp_alto() {
        assert_eq!(peso_de(6.0), (5, true));
        assert_eq!(peso_de(100.0), (5, true));
    }

    #[test]
    fn test_peso_de_nan() {
        let (peso, suspeito) = peso_de(f64::NAN);
        assert_eq!(peso, 1);
        assert!(suspeito);
    }

    #[test]
    fn test_peso_de_infinito() {
        let (peso, suspeito) = peso_de(f64::INFINITY);
        assert_eq!(peso, 5);
        assert!(suspeito);
    }

    #[test]
    fn test_peso_de_fracao() {
        assert_eq!(peso_de(2.7), (3, false)); // round(2.7)=3
        assert_eq!(peso_de(1.2), (1, false)); // round(1.2)=1
    }

    // === leitura_bool ===

    #[test]
    fn test_leitura_bool_verdadeiro() {
        assert_eq!(leitura_bool("sim"), BoolLeitura::Verdadeiro);
        assert_eq!(leitura_bool("SIM"), BoolLeitura::Verdadeiro);
        assert_eq!(leitura_bool("true"), BoolLeitura::Verdadeiro);
        assert_eq!(leitura_bool("TRUE"), BoolLeitura::Verdadeiro);
        assert_eq!(leitura_bool("1"), BoolLeitura::Verdadeiro);
        assert_eq!(leitura_bool("yes"), BoolLeitura::Verdadeiro);
        assert_eq!(leitura_bool("s"), BoolLeitura::Verdadeiro);
    }

    #[test]
    fn test_leitura_bool_falso() {
        assert_eq!(leitura_bool("não"), BoolLeitura::Falso);
        assert_eq!(leitura_bool("nao"), BoolLeitura::Falso);
        assert_eq!(leitura_bool("NAO"), BoolLeitura::Falso);
        assert_eq!(leitura_bool("false"), BoolLeitura::Falso);
        assert_eq!(leitura_bool("FALSE"), BoolLeitura::Falso);
        assert_eq!(leitura_bool("0"), BoolLeitura::Falso);
        assert_eq!(leitura_bool("no"), BoolLeitura::Falso);
        assert_eq!(leitura_bool("n"), BoolLeitura::Falso);
    }

    #[test]
    fn test_leitura_bool_ininteligivel() {
        assert_eq!(leitura_bool("talvez"), BoolLeitura::Ininteligivel);
        assert_eq!(leitura_bool(""), BoolLeitura::Ininteligivel);
        assert_eq!(leitura_bool("2"), BoolLeitura::Ininteligivel);
        assert_eq!(leitura_bool("abc"), BoolLeitura::Ininteligivel);
    }

    #[test]
    fn test_leitura_bool_trim() {
        assert_eq!(leitura_bool("  sim  "), BoolLeitura::Verdadeiro);
        assert_eq!(leitura_bool("\tfalse\n"), BoolLeitura::Falso);
    }

    // === normalizar_lista ===

    #[test]
    fn test_normalizar_lista_dentro_teto() {
        let itens: Vec<i32> = (0..10).collect();
        let (resultado, omitidos) = normalizar_lista(&itens);
        assert_eq!(resultado.len(), 10);
        assert_eq!(omitidos, 0);
    }

    #[test]
    fn test_normalizar_lista_no_teto() {
        let itens: Vec<i32> = (0..MAX_ITENS as i32).collect();
        let (resultado, omitidos) = normalizar_lista(&itens);
        assert_eq!(resultado.len(), MAX_ITENS);
        assert_eq!(omitidos, 0);
    }

    #[test]
    fn test_normalizar_lista_acima_teto() {
        let itens: Vec<i32> = (0..1500).collect();
        let (resultado, omitidos) = normalizar_lista(&itens);
        assert_eq!(resultado.len(), MAX_ITENS);
        assert_eq!(omitidos, 500);
        assert_eq!(resultado[0], 0);
        assert_eq!(resultado[MAX_ITENS - 1], (MAX_ITENS - 1) as i32);
    }

    #[test]
    fn test_normalizar_lista_vazia() {
        let itens: Vec<i32> = vec![];
        let (resultado, omitidos) = normalizar_lista(&itens);
        assert!(resultado.is_empty());
        assert_eq!(omitidos, 0);
    }

    // === campo_ex ===

    #[test]
    fn test_campo_ex_sucesso() {
        let resultado = campo_ex(|| 42);
        assert_eq!(resultado, Some(42));
    }

    #[test]
    fn test_campo_ex_panic() {
        let resultado = campo_ex(|| -> i32 { panic!("getter hostil") });
        assert_eq!(resultado, None);
    }

    #[test]
    fn test_campo_ex_result_ok() {
        let resultado = campo_ex_result(|| Ok::<i32, &str>(42));
        assert_eq!(resultado, Some(42));
    }

    #[test]
    fn test_campo_ex_result_err() {
        let resultado = campo_ex_result(|| Err::<i32, &str>("falha"));
        assert_eq!(resultado, None);
    }

    // === decide_delivery ===

    #[test]
    fn test_decide_delivery_ok_sem_problema() {
        let d = decide_delivery(true, false, None);
        assert!(d.ok);
        assert!(!d.problema_provado);
        assert!(d.motivo.is_none());
    }

    #[test]
    fn test_decide_delivery_nao_ok_sem_achado() {
        let d = decide_delivery(false, false, Some("timeout".into()));
        assert!(!d.ok);
        assert!(!d.problema_provado);
        assert_eq!(d.motivo.unwrap(), "timeout");
    }

    #[test]
    fn test_decide_delivery_nao_ok_com_achado_aterrado() {
        let d = decide_delivery(false, true, Some("teste falhou".into()));
        assert!(!d.ok);
        assert!(d.problema_provado);
        assert_eq!(d.motivo.unwrap(), "teste falhou");
    }

    #[test]
    fn test_decide_delivery_ok_contradiz_achado_aterrado() {
        let d = decide_delivery(true, true, None);
        assert!(!d.ok); // nunca entrega ok=true com achado negativo
        assert!(d.problema_provado);
        assert!(d.motivo.unwrap().contains("contradiz"));
    }

    #[test]
    fn test_decide_delivery_nao_ok_com_achado_sem_motivo() {
        let d = decide_delivery(false, true, None);
        assert!(!d.ok);
        assert!(d.problema_provado);
        assert_eq!(d.motivo.unwrap(), "achado negativo aterrado");
    }

    // === build_report ===

    #[test]
    fn test_build_report_sem_truncamento() {
        let r = build_report("Teste", "conteúdo curto", 1000, 10, 8, 0.5);
        assert!(!r.truncado);
        // "conteúdo curto" is 15 bytes in UTF-8 (û = 2 bytes)
        assert_eq!(r.tamanho_original, 15);
        assert_eq!(r.itens_omitidos, 0);
        assert!((r.pct_aterrado - 80.0).abs() < f64::EPSILON);
        assert!((r.custo_real - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_build_report_com_truncamento() {
        let conteudo = "a".repeat(200);
        let r = build_report("Longo", &conteudo, 50, 100, 60, 1.2);
        assert!(r.truncado);
        assert_eq!(r.tamanho_original, 200);
        assert!(r.conteudo_truncado.contains("TRUNCADO"));
        assert!(r.conteudo_truncado.len() <= 100); // 50 + suffix
        assert!((r.pct_aterrado - 60.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_build_report_zero_itens() {
        let r = build_report("Vazio", "", 100, 0, 0, 0.0);
        assert!((r.pct_aterrado - 0.0).abs() < f64::EPSILON);
    }

    // === checar_completude ===

    #[test]
    fn test_checar_completude_valida() {
        let conteudo = b"screenshot data";
        let mut hasher = Sha256::new();
        hasher.update(conteudo);
        let hash = format!("{:x}", hasher.finalize());

        let manifesto = ScreenshotManifest {
            hash,
            timestamp: "2026-11-03T00:00:00Z".into(),
            arquivo: "screen.png".into(),
            width: 1920,
            height: 1080,
        };

        let result = checar_completude(&manifesto, conteudo);
        assert!(result.completa);
        assert!(result.motivos.is_empty());
    }

    #[test]
    fn test_checar_completude_hash_mismatch() {
        let manifesto = ScreenshotManifest {
            hash: "wrong_hash".into(),
            timestamp: "2026-11-03T00:00:00Z".into(),
            arquivo: "screen.png".into(),
            width: 1920,
            height: 1080,
        };

        let result = checar_completude(&manifesto, b"data");
        assert!(!result.completa);
        assert!(result.motivos.iter().any(|m| m.contains("hash mismatch")));
    }

    #[test]
    fn test_checar_completude_timestamp_vazio() {
        let conteudo = b"data";
        let mut hasher = Sha256::new();
        hasher.update(conteudo);
        let hash = format!("{:x}", hasher.finalize());

        let manifesto = ScreenshotManifest {
            hash,
            timestamp: "".into(),
            arquivo: "screen.png".into(),
            width: 1920,
            height: 1080,
        };

        let result = checar_completude(&manifesto, conteudo);
        assert!(!result.completa);
        assert!(result.motivos.iter().any(|m| m.contains("timestamp")));
    }

    #[test]
    fn test_checar_completude_dimensoes_zero() {
        let conteudo = b"data";
        let mut hasher = Sha256::new();
        hasher.update(conteudo);
        let hash = format!("{:x}", hasher.finalize());

        let manifesto = ScreenshotManifest {
            hash,
            timestamp: "2026-11-03T00:00:00Z".into(),
            arquivo: "screen.png".into(),
            width: 0,
            height: 0,
        };

        let result = checar_completude(&manifesto, conteudo);
        assert!(!result.completa);
        assert!(result.motivos.iter().any(|m| m.contains("dimensões")));
    }

    #[test]
    fn test_checar_completude_multiplos_erros() {
        let manifesto = ScreenshotManifest {
            hash: "wrong".into(),
            timestamp: "".into(),
            arquivo: "".into(),
            width: 0,
            height: 0,
        };

        let result = checar_completude(&manifesto, b"data");
        assert!(!result.completa);
        assert!(result.motivos.len() >= 3);
    }
}