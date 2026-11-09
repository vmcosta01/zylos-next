//! Chaves de API lidas do arquivo de chaves do Hermes (`<pasta do Hermes>\.env`).
//!
//! Igual ao Hermes (`hermes_cli/auth.py:379`, `env_loader.py:658`): o valor do
//! arquivo VENCE a variável de ambiente do processo; o ambiente só vale quando o
//! arquivo não tem a chave. O arquivo é lido a cada chamada.
//! O valor vive num `Segredo`, que nunca se imprime.

use std::path::{Path, PathBuf};

use crate::texto;

/// Valor de uma chave. `Debug` e `Display` imprimem `***`.
#[derive(Clone, PartialEq, Eq)]
pub struct Segredo(String);

impl Segredo {
    /// O valor, só para montar o cabeçalho da chamada.
    pub fn expor(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Segredo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Segredo(***)")
    }
}

impl std::fmt::Display for Segredo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("***")
    }
}

#[cfg(test)]
pub(crate) fn tests_segredo(v: &str) -> Segredo {
    Segredo(v.to_string())
}

/// Por que a chave não pôde ser usada. Nunca carrega o valor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErroChave {
    /// Nem o arquivo nem o ambiente têm a chave (ou ela está vazia).
    Faltando { nome: String },
    /// O valor usa `${...}`; o zylos não expande variáveis.
    ComExpansao { nome: String },
    /// O arquivo de chaves existe mas não pôde ser lido.
    ArquivoIlegivel { motivo: String },
}

/// Pasta do Hermes: `HERMES_HOME` (com `~` e `%VAR%` expandidos), senão
/// `%LOCALAPPDATA%\hermes`.
pub fn pasta_hermes(ambiente: &dyn Fn(&str) -> Option<String>) -> Option<PathBuf> {
    if let Some(h) = ambiente("HERMES_HOME").filter(|h| !h.trim().is_empty()) {
        return Some(PathBuf::from(expandir(h.trim(), ambiente)));
    }
    ambiente("LOCALAPPDATA").map(|l| PathBuf::from(l).join("hermes"))
}

/// Expande `~` no começo e `%VAR%` (estilo Windows).
fn expandir(s: &str, ambiente: &dyn Fn(&str) -> Option<String>) -> String {
    let mut s = s.to_string();
    if s == "~" || s.starts_with("~/") || s.starts_with("~\\") {
        if let Some(casa) = ambiente("USERPROFILE").or_else(|| ambiente("HOME")) {
            s = format!("{casa}{}", &s[1..]);
        }
    }
    let mut saida = String::new();
    let mut resto = s.as_str();
    while let Some(i) = resto.find('%') {
        saida.push_str(&resto[..i]);
        let depois = &resto[i + 1..];
        match depois.find('%') {
            Some(j) if j > 0 => {
                let nome = &depois[..j];
                match ambiente(nome) {
                    Some(v) => saida.push_str(&v),
                    None => {
                        saida.push('%');
                        saida.push_str(nome);
                        saida.push('%');
                    }
                }
                resto = &depois[j + 1..];
            }
            _ => {
                saida.push('%');
                resto = depois;
            }
        }
    }
    saida.push_str(resto);
    saida
}

/// Procura `nome` no texto de um `.env`. A última ocorrência vence.
/// Aceita: BOM (tirado antes), CRLF, linhas vazias, `# comentário`,
/// `export NOME=`, valor entre aspas simples ou duplas, `# comentário` no fim
/// de valor sem aspas (precedido de espaço).
pub fn procurar_no_env(conteudo: &str, nome: &str) -> Option<String> {
    let mut achado = None;
    for linha in conteudo.split('\n') {
        let linha = linha.trim_end_matches('\r').trim();
        if linha.is_empty() || linha.starts_with('#') {
            continue;
        }
        let linha = linha.strip_prefix("export ").map(str::trim_start).unwrap_or(linha);
        let Some((k, v)) = linha.split_once('=') else { continue };
        if k.trim() != nome {
            continue;
        }
        achado = Some(valor_do_env(v.trim()));
    }
    achado
}

fn valor_do_env(v: &str) -> String {
    for aspa in ['"', '\''] {
        if let Some(dentro) = v.strip_prefix(aspa) {
            return match dentro.find(aspa) {
                Some(fim) => dentro[..fim].to_string(),
                None => dentro.to_string(),
            };
        }
    }
    let sem_comentario = match v.find(" #").or_else(|| v.find("\t#")) {
        Some(i) => &v[..i],
        None => v,
    };
    sem_comentario.trim().to_string()
}

/// Lê a chave `nome`: primeiro do arquivo `arquivo_env`, depois do ambiente.
pub fn ler_chave(
    arquivo_env: &Path,
    nome: &str,
    ambiente: &dyn Fn(&str) -> Option<String>,
) -> Result<Segredo, ErroChave> {
    let do_arquivo = match texto::ler(arquivo_env, None) {
        Ok(conteudo) => procurar_no_env(&conteudo, nome),
        Err(texto::ErroTexto::Inexistente) => None,
        Err(e) => return Err(ErroChave::ArquivoIlegivel { motivo: e.to_string() }),
    };
    let valor = do_arquivo
        .filter(|v| !v.is_empty())
        .or_else(|| ambiente(nome).map(|v| v.trim().to_string()).filter(|v| !v.is_empty()));
    match valor {
        None => Err(ErroChave::Faltando { nome: nome.to_string() }),
        Some(v) if v.contains("${") => Err(ErroChave::ComExpansao { nome: nome.to_string() }),
        Some(v) => Ok(Segredo(v)),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn sem_ambiente(_: &str) -> Option<String> {
        None
    }

    fn arquivo(conteudo: &[u8]) -> (tempfile::TempDir, PathBuf) {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join(".env");
        std::fs::write(&p, conteudo).unwrap();
        (d, p)
    }

    #[test]
    fn formatos_do_env() {
        let env = "\u{feff}# comentario\r\n\r\nA=um\r\nexport B = dois \r\nC=\"tres # nao e comentario\"\nD='quatro'\nE=cinco # comentario\nF=\nA=ultimo\n";
        let env = env.trim_start_matches('\u{feff}');
        assert_eq!(procurar_no_env(env, "A").as_deref(), Some("ultimo"));
        assert_eq!(procurar_no_env(env, "B").as_deref(), Some("dois"));
        assert_eq!(procurar_no_env(env, "C").as_deref(), Some("tres # nao e comentario"));
        assert_eq!(procurar_no_env(env, "D").as_deref(), Some("quatro"));
        assert_eq!(procurar_no_env(env, "E").as_deref(), Some("cinco"));
        assert_eq!(procurar_no_env(env, "F").as_deref(), Some(""));
        assert_eq!(procurar_no_env(env, "G"), None);
        assert_eq!(procurar_no_env("AB=x\n", "A"), None, "prefixo do nome não conta");
    }

    #[test]
    fn bom_e_crlf_no_arquivo_nao_viram_valor() {
        let (_d, p) = arquivo(b"\xEF\xBB\xBFNVIDIA_API_KEY=nv-123\r\n");
        assert_eq!(ler_chave(&p, "NVIDIA_API_KEY", &sem_ambiente).unwrap().expor(), "nv-123");
    }

    #[test]
    fn arquivo_vence_o_ambiente() {
        let (_d, p) = arquivo(b"OPENAI_API_KEY=do-arquivo\n");
        let ambiente = |n: &str| (n == "OPENAI_API_KEY").then(|| "do-ambiente".to_string());
        assert_eq!(ler_chave(&p, "OPENAI_API_KEY", &ambiente).unwrap().expor(), "do-arquivo");
    }

    #[test]
    fn ambiente_so_quando_o_arquivo_nao_tem() {
        let (_d, p) = arquivo(b"OUTRA=x\nOPENAI_API_KEY=\n");
        let ambiente = |n: &str| (n == "OPENAI_API_KEY").then(|| "do-ambiente".to_string());
        assert_eq!(ler_chave(&p, "OPENAI_API_KEY", &ambiente).unwrap().expor(), "do-ambiente");
        let inexistente = p.with_file_name("nao-existe.env");
        assert_eq!(ler_chave(&inexistente, "OPENAI_API_KEY", &ambiente).unwrap().expor(), "do-ambiente");
    }

    #[test]
    fn faltando_e_expansao_citam_so_o_nome() {
        let (_d, p) = arquivo(b"A=${B}\nV=\n");
        assert_eq!(ler_chave(&p, "A", &sem_ambiente), Err(ErroChave::ComExpansao { nome: "A".into() }));
        assert_eq!(ler_chave(&p, "V", &sem_ambiente), Err(ErroChave::Faltando { nome: "V".into() }));
        assert_eq!(ler_chave(&p, "Z", &sem_ambiente), Err(ErroChave::Faltando { nome: "Z".into() }));
    }

    #[test]
    fn utf16_e_ilegivel() {
        let (_d, p) = arquivo(b"\xFF\xFEA\0=\0x\0");
        assert!(matches!(ler_chave(&p, "A", &sem_ambiente), Err(ErroChave::ArquivoIlegivel { .. })));
    }

    #[test]
    fn segredo_nunca_se_imprime() {
        let s = Segredo("sk-segredo-123".into());
        assert!(!format!("{s:?}").contains("sk-segredo"));
        assert!(!format!("{s}").contains("sk-segredo"));
        assert!(!format!("{:?}", Some(&s)).contains("sk-segredo"));
    }

    #[test]
    fn pasta_hermes_expande() {
        let amb = |n: &str| match n {
            "HERMES_HOME" => Some("%LOCALAPPDATA%\\hermes-teste".to_string()),
            "LOCALAPPDATA" => Some("C:\\L".to_string()),
            _ => None,
        };
        assert_eq!(pasta_hermes(&amb).unwrap(), PathBuf::from("C:\\L\\hermes-teste"));
        let til = |n: &str| match n {
            "HERMES_HOME" => Some("~\\h".to_string()),
            "USERPROFILE" => Some("C:\\U".to_string()),
            _ => None,
        };
        assert_eq!(pasta_hermes(&til).unwrap(), PathBuf::from("C:\\U\\h"));
        let padrao = |n: &str| (n == "LOCALAPPDATA").then(|| "C:\\L".to_string());
        assert_eq!(pasta_hermes(&padrao).unwrap(), PathBuf::from("C:\\L").join("hermes"));
    }
}
