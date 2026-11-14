//! Bots: `<pasta dos bots>\<nome>\bot.toml` (provedor, modelo) + `ALMA.md`
//! (a personalidade, texto livre).

use std::path::Path;

use serde::Deserialize;

use crate::texto::{self, ErroTexto};

/// Tamanho máximo do `ALMA.md` (vai inteiro em cada pergunta e custa tokens).
pub const LIMITE_ALMA: usize = 32 * 1024;
const LIMITE_BOT_TOML: usize = 16 * 1024;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BotToml {
    provedor: String,
    #[serde(default)]
    modelo: Option<String>,
}

/// Um bot lido do disco.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bot {
    pub nome: String,
    pub provedor: String,
    pub modelo: Option<String>,
    pub alma: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErroBot {
    NomeInvalido,
    Inexistente,
    MalFeito(String),
}

/// Nome de bot: `a-z`, `0-9` e `-`, até 64, sem começar com `-`. Impede que o
/// nome saia da pasta dos bots (`..`, `\`, `/`, `:`).
pub fn nome_valido(nome: &str) -> bool {
    !nome.is_empty()
        && nome.len() <= 64
        && !nome.starts_with('-')
        && nome
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn ler_arquivo(caminho: &Path, limite: usize, rotulo: &str) -> Result<String, ErroBot> {
    texto::ler(caminho, Some(limite)).map_err(|e| match e {
        ErroTexto::Inexistente => ErroBot::MalFeito(format!("falta o arquivo {rotulo}")),
        outro => ErroBot::MalFeito(format!("{rotulo}: {outro}")),
    })
}

/// Lê o bot `nome` de `pasta_bots`.
pub fn carregar(pasta_bots: &Path, nome: &str) -> Result<Bot, ErroBot> {
    if !nome_valido(nome) {
        return Err(ErroBot::NomeInvalido);
    }
    let pasta = pasta_bots.join(nome);
    if !pasta.is_dir() {
        return Err(ErroBot::Inexistente);
    }
    let txt = ler_arquivo(&pasta.join("bot.toml"), LIMITE_BOT_TOML, "bot.toml")?;
    let cfg: BotToml = toml::from_str(&txt)
        .map_err(|e| ErroBot::MalFeito(format!("bot.toml inválido: {}", e.message())))?;
    let alma = ler_arquivo(&pasta.join("ALMA.md"), LIMITE_ALMA, "ALMA.md")?;
    let modelo = cfg
        .modelo
        .map(|m| m.trim().to_string())
        .filter(|m| !m.is_empty());
    Ok(Bot {
        nome: nome.to_string(),
        provedor: cfg.provedor.trim().to_string(),
        modelo,
        alma,
    })
}

#[cfg(test)]
mod testes {
    use super::*;

    fn bot(pasta: &Path, nome: &str, toml: &str, alma: Option<&[u8]>) {
        let d = pasta.join(nome);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("bot.toml"), toml).unwrap();
        if let Some(a) = alma {
            std::fs::write(d.join("ALMA.md"), a).unwrap();
        }
    }

    #[test]
    fn le_bot_com_bom() {
        let t = tempfile::tempdir().unwrap();
        bot(
            t.path(),
            "vigia",
            "\u{feff}provedor = \"nvidia\"\nmodelo = \"m\"\n",
            Some(b"\xEF\xBB\xBFVoce vigia."),
        );
        let b = carregar(t.path(), "vigia").unwrap();
        assert_eq!(b.provedor, "nvidia");
        assert_eq!(b.modelo.as_deref(), Some("m"));
        assert_eq!(b.alma, "Voce vigia.");
    }

    #[test]
    fn nomes_que_saem_da_pasta_sao_recusados() {
        let t = tempfile::tempdir().unwrap();
        for n in ["..", "../x", "a\\b", "a/b", "C:", "Vigia", "-a", "", "a b"] {
            assert_eq!(carregar(t.path(), n), Err(ErroBot::NomeInvalido), "{n:?}");
        }
        assert!(nome_valido("vigia-2"));
    }

    #[test]
    fn inexistente_e_mal_feito() {
        let t = tempfile::tempdir().unwrap();
        assert_eq!(carregar(t.path(), "nada"), Err(ErroBot::Inexistente));
        bot(t.path(), "sem-alma", "provedor = \"nvidia\"\n", None);
        assert!(
            matches!(carregar(t.path(), "sem-alma"), Err(ErroBot::MalFeito(m)) if m.contains("ALMA.md"))
        );
        bot(
            t.path(),
            "campo",
            "provedor = \"nvidia\"\nchave = \"sk\"\n",
            Some(b"a"),
        );
        assert!(matches!(
            carregar(t.path(), "campo"),
            Err(ErroBot::MalFeito(_))
        ));
        bot(
            t.path(),
            "grande",
            "provedor = \"nvidia\"\n",
            Some(&vec![b'a'; LIMITE_ALMA + 1]),
        );
        assert!(
            matches!(carregar(t.path(), "grande"), Err(ErroBot::MalFeito(m)) if m.contains("limite"))
        );
        bot(
            t.path(),
            "utf16",
            "provedor = \"nvidia\"\n",
            Some(b"\xFF\xFEa\0"),
        );
        assert!(
            matches!(carregar(t.path(), "utf16"), Err(ErroBot::MalFeito(m)) if m.contains("UTF-16"))
        );
    }
}
