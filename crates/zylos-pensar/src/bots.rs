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
    /// Como o bot aparece na tela (o `nome` da pasta continua sendo o identificador).
    #[serde(default)]
    nome_exibicao: Option<String>,
}

/// Mais caracteres que isto no nome de exibição: o resto é cortado.
pub const LIMITE_NOME_EXIBICAO: usize = 40;

/// Nome de exibição pronto para a tela: sem espaço nas pontas, no máximo
/// [`LIMITE_NOME_EXIBICAO`] caracteres; vazio ou com caractere de controle (quebra de linha,
/// escape de terminal) vira `None` e a tela usa o nome da pasta.
fn limpar_nome_exibicao(bruto: &str) -> Option<String> {
    let t = bruto.trim();
    if t.is_empty() || t.chars().any(char::is_control) {
        return None;
    }
    Some(t.chars().take(LIMITE_NOME_EXIBICAO).collect())
}

/// Um bot lido do disco.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bot {
    pub nome: String,
    pub provedor: String,
    pub modelo: Option<String>,
    pub nome_exibicao: Option<String>,
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
        nome_exibicao: cfg.nome_exibicao.as_deref().and_then(limpar_nome_exibicao),
        alma,
    })
}

/// Mais bots que isto na pasta: o resto é ignorado (a lista vai inteira para a tela).
pub const LIMITE_LISTA: usize = 200;

/// Um bot como aparece na lista: sem a `ALMA.md` (é grande e é do próprio bot).
#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub struct InfoBot {
    pub nome: String,
    pub provedor: Option<String>,
    pub modelo: Option<String>,
    /// Nome para mostrar, se o `bot.toml` define um; senão a tela usa `nome`.
    pub nome_exibicao: Option<String>,
    /// Por que o bot não pode responder (sem `bot.toml`, sem `ALMA.md`, ...), quando não pode.
    pub problema: Option<String>,
}

/// Os bots de `pasta_bots`, em ordem de nome. Pasta que não existe = nenhum bot.
/// Subpasta com nome inválido é ignorada; bot mal feito aparece com o `problema`.
pub fn listar(pasta_bots: &Path) -> Vec<InfoBot> {
    let Ok(entradas) = std::fs::read_dir(pasta_bots) else {
        return Vec::new();
    };
    let mut nomes: Vec<String> = entradas
        .filter_map(Result::ok)
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| nome_valido(n))
        .collect();
    nomes.sort();
    nomes.truncate(LIMITE_LISTA);
    nomes
        .into_iter()
        .map(|nome| match carregar(pasta_bots, &nome) {
            Ok(b) => InfoBot {
                nome,
                provedor: Some(b.provedor),
                modelo: b.modelo,
                nome_exibicao: b.nome_exibicao,
                problema: None,
            },
            Err(e) => InfoBot {
                nome,
                provedor: None,
                modelo: None,
                nome_exibicao: None,
                problema: Some(match e {
                    ErroBot::MalFeito(m) => m,
                    ErroBot::NomeInvalido => "nome inválido".to_string(),
                    ErroBot::Inexistente => "pasta sumiu".to_string(),
                }),
            },
        })
        .collect()
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn lista_bots_em_ordem_e_aponta_o_mal_feito() {
        let t = tempfile::tempdir().unwrap();
        assert!(listar(&t.path().join("nao-existe")).is_empty());
        bot(
            t.path(),
            "vigia",
            "provedor = \"nvidia\"\nmodelo = \"m\"\n",
            Some(b"a"),
        );
        bot(
            t.path(),
            "arquivista",
            "provedor = \"openai\"\n",
            Some(b"a"),
        );
        bot(t.path(), "sem-alma", "provedor = \"openai\"\n", None);
        std::fs::create_dir_all(t.path().join("Nome Invalido")).unwrap();
        std::fs::write(t.path().join("solto.txt"), "x").unwrap();
        let l = listar(t.path());
        let nomes: Vec<_> = l.iter().map(|b| b.nome.as_str()).collect();
        assert_eq!(nomes, ["arquivista", "sem-alma", "vigia"]);
        assert_eq!(l[0].provedor.as_deref(), Some("openai"));
        assert_eq!(l[0].problema, None);
        assert!(l[1].problema.as_deref().unwrap().contains("ALMA.md"));
        assert_eq!(l[2].modelo.as_deref(), Some("m"));
    }

    fn bot(pasta: &Path, nome: &str, toml: &str, alma: Option<&[u8]>) {
        let d = pasta.join(nome);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("bot.toml"), toml).unwrap();
        if let Some(a) = alma {
            std::fs::write(d.join("ALMA.md"), a).unwrap();
        }
    }

    #[test]
    fn nome_de_exibicao_e_limpo_e_opcional() {
        let t = tempfile::tempdir().unwrap();
        bot(
            t.path(),
            "vigia",
            "provedor = \"nvidia\"\nnome_exibicao = \"  Vigia da Noite \"\n",
            Some(b"a"),
        );
        bot(t.path(), "arq", "provedor = \"openai\"\n", Some(b"a"));
        bot(
            t.path(),
            "linha",
            "provedor = \"openai\"\nnome_exibicao = \"a\\nb\"\n",
            Some(b"a"),
        );
        bot(
            t.path(),
            "comprido",
            &format!(
                "provedor = \"openai\"\nnome_exibicao = \"{}\"\n",
                "x".repeat(100)
            ),
            Some(b"a"),
        );
        bot(
            t.path(),
            "vazio",
            "provedor = \"openai\"\nnome_exibicao = \"   \"\n",
            Some(b"a"),
        );
        let l = listar(t.path());
        let por = |n: &str| {
            l.iter()
                .find(|b| b.nome == n)
                .unwrap()
                .nome_exibicao
                .clone()
        };
        assert_eq!(por("vigia").as_deref(), Some("Vigia da Noite"));
        assert_eq!(por("arq"), None);
        assert_eq!(
            por("linha"),
            None,
            "caractere de controle: usa o nome da pasta"
        );
        assert_eq!(
            por("comprido").unwrap().chars().count(),
            LIMITE_NOME_EXIBICAO
        );
        assert_eq!(por("vazio"), None);
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
