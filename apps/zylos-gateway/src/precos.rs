//! Tabela de preços do operador: quanto custa cada modelo por milhão de tokens.
//!
//! O gateway **não sabe preços**: eles mudam, variam por contrato e não vêm na resposta do
//! provedor. Quem os conhece é o operador, que os põe num arquivo JSON apontado por
//! `ZYLOS_PRECOS_FILE`:
//!
//! ```json
//! { "modelos": {
//!     "claude-sonnet-5-5":   { "entrada_por_milhao_usd": 3.0, "saida_por_milhao_usd": 15.0 },
//!     "anthropic/claude-x":  { "entrada_por_milhao_usd": 1.0, "saida_por_milhao_usd": 5.0 }
//! } }
//! ```
//!
//! A chave é o nome do modelo (como aparece na resposta do bot) ou `provedor/modelo`, que vence
//! quando existe. Sem arquivo, sem a entrada do modelo ou sem a divisão entrada/saída na resposta
//! do provedor, **não há custo**: a Frota mostra "—", nunca um zero inventado.

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

/// Variável com o caminho do arquivo de preços.
pub const PRECOS_ENV: &str = "ZYLOS_PRECOS_FILE";
/// Teto de tamanho do arquivo (uma tabela de preços cabe folgada em 256 KB).
const MAX_BYTES: u64 = 256 * 1024;
/// Acima disto por milhão de tokens o valor é digitação errada, não preço.
const PRECO_MAXIMO: f64 = 100_000.0;

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct Preco {
    pub entrada_por_milhao_usd: f64,
    pub saida_por_milhao_usd: f64,
}

impl Preco {
    fn valido(&self) -> bool {
        [self.entrada_por_milhao_usd, self.saida_por_milhao_usd]
            .iter()
            .all(|p| p.is_finite() && (0.0..=PRECO_MAXIMO).contains(p))
    }
}

#[derive(Debug, Deserialize)]
struct Arquivo {
    modelos: HashMap<String, Preco>,
}

/// Preços carregados. O padrão (vazio) significa "não há preço para nada".
#[derive(Debug, Clone, Default)]
pub struct Precos {
    por_modelo: HashMap<String, Preco>,
}

impl Precos {
    /// Lê o arquivo de `ZYLOS_PRECOS_FILE`; qualquer problema vira aviso no log e tabela vazia
    /// (a Frota segue funcionando, só sem custo).
    pub fn do_ambiente() -> Self {
        let Some(caminho) = std::env::var(PRECOS_ENV)
            .ok()
            .filter(|c| !c.trim().is_empty())
        else {
            return Self::default();
        };
        match Self::de_arquivo(Path::new(&caminho)) {
            Ok(p) => p,
            Err(motivo) => {
                tracing::warn!(motivo = %motivo, "tabela de preços ignorada: custo não será calculado");
                Self::default()
            }
        }
    }

    pub fn de_arquivo(caminho: &Path) -> Result<Self, String> {
        let meta = std::fs::metadata(caminho).map_err(|e| format!("não abriu: {}", e.kind()))?;
        if meta.len() > MAX_BYTES {
            return Err(format!("arquivo passa de {MAX_BYTES} bytes"));
        }
        let texto =
            std::fs::read_to_string(caminho).map_err(|e| format!("não leu: {}", e.kind()))?;
        Self::de_json(texto.trim_start_matches('\u{feff}'))
    }

    pub fn de_json(texto: &str) -> Result<Self, String> {
        let a: Arquivo = serde_json::from_str(texto).map_err(|e| format!("JSON inválido: {e}"))?;
        let mut por_modelo = HashMap::new();
        for (nome, preco) in a.modelos {
            if preco.valido() {
                por_modelo.insert(nome.trim().to_ascii_lowercase(), preco);
            } else {
                tracing::warn!(modelo = %nome, "preço inválido ignorado (negativo, infinito ou absurdo)");
            }
        }
        Ok(Self { por_modelo })
    }

    /// Quantos modelos têm preço.
    pub fn len(&self) -> usize {
        self.por_modelo.len()
    }

    pub fn is_empty(&self) -> bool {
        self.por_modelo.is_empty()
    }

    /// Custo em dólares de uma resposta, ou `None` se o modelo não tem preço na tabela.
    pub fn custo_usd(&self, provedor: &str, modelo: &str, entrada: u64, saida: u64) -> Option<f64> {
        let modelo = modelo.trim().to_ascii_lowercase();
        let com_provedor = format!("{}/{modelo}", provedor.trim().to_ascii_lowercase());
        let p = self
            .por_modelo
            .get(&com_provedor)
            .or_else(|| self.por_modelo.get(&modelo))?;
        Some(
            (entrada as f64 * p.entrada_por_milhao_usd + saida as f64 * p.saida_por_milhao_usd)
                / 1_000_000.0,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tabela() -> Precos {
        Precos::de_json(
            r#"{"modelos":{
                "Modelo-A":{"entrada_por_milhao_usd":3.0,"saida_por_milhao_usd":15.0},
                "prov/modelo-b":{"entrada_por_milhao_usd":1.0,"saida_por_milhao_usd":2.0},
                "modelo-b":{"entrada_por_milhao_usd":9.0,"saida_por_milhao_usd":9.0}
            }}"#,
        )
        .unwrap()
    }

    #[test]
    fn custo_e_entrada_vezes_preco_mais_saida_vezes_preco() {
        // 1000 entrada a US$3/M + 200 saída a US$15/M = 0,003 + 0,003.
        let c = tabela().custo_usd("x", "modelo-a", 1000, 200).unwrap();
        assert!((c - 0.006).abs() < 1e-12, "{c}");
        // Maiúsculas não importam.
        assert!(tabela().custo_usd("x", "MODELO-A", 1, 1).is_some());
    }

    #[test]
    fn provedor_barra_modelo_vence_o_preco_so_do_modelo() {
        let c = tabela()
            .custo_usd("prov", "modelo-b", 1_000_000, 0)
            .unwrap();
        assert!((c - 1.0).abs() < 1e-12, "{c}");
        let c = tabela()
            .custo_usd("outro", "modelo-b", 1_000_000, 0)
            .unwrap();
        assert!((c - 9.0).abs() < 1e-12, "{c}");
    }

    #[test]
    fn sem_preco_nao_ha_custo_nunca_zero() {
        assert_eq!(tabela().custo_usd("x", "desconhecido", 10, 10), None);
        assert_eq!(Precos::default().custo_usd("x", "modelo-a", 10, 10), None);
    }

    #[test]
    fn preco_invalido_e_ignorado_e_json_ruim_e_erro() {
        let p = Precos::de_json(
            r#"{"modelos":{
                "neg":{"entrada_por_milhao_usd":-1.0,"saida_por_milhao_usd":1.0},
                "absurdo":{"entrada_por_milhao_usd":1.0,"saida_por_milhao_usd":1e12},
                "ok":{"entrada_por_milhao_usd":0.0,"saida_por_milhao_usd":1.0}
            }}"#,
        )
        .unwrap();
        assert_eq!(p.len(), 1);
        assert!(p.custo_usd("x", "ok", 5, 5).is_some());
        assert!(Precos::de_json("{").is_err());
        assert!(Precos::de_json(r#"{"modelos":{"a":{"entrada_por_milhao_usd":"x"}}}"#).is_err());
    }

    #[test]
    fn arquivo_com_bom_e_arquivo_enorme() {
        let d = tempfile::tempdir().unwrap();
        let bom = d.path().join("bom.json");
        std::fs::write(
            &bom,
            "\u{feff}{\"modelos\":{\"m\":{\"entrada_por_milhao_usd\":1.0,\"saida_por_milhao_usd\":1.0}}}",
        )
        .unwrap();
        assert_eq!(Precos::de_arquivo(&bom).unwrap().len(), 1);
        let grande = d.path().join("grande.json");
        std::fs::write(&grande, vec![b' '; (MAX_BYTES + 1) as usize]).unwrap();
        assert!(Precos::de_arquivo(&grande).is_err());
        assert!(Precos::de_arquivo(&d.path().join("nao-existe.json")).is_err());
    }
}
