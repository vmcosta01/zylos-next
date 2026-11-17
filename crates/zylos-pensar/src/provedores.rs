//! Fichas de provedor: nome, língua, endereço, nome da chave e modelo padrão.
//!
//! As línguas e os nomes seguem o Hermes (`hermes_cli/providers.py`): `openai_chat`
//! é a língua da maioria; `anthropic_messages` é a do Claude. No Hermes, `openai`
//! sozinho é apelido do OpenRouter — por isso a ficha da OpenAI chama `openai-api`.

use serde::{Deserialize, Serialize};

/// A "língua" que o provedor fala.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Lingua {
    /// `POST {endereco}/chat/completions`, chave em `Authorization: Bearer`.
    OpenaiChat,
    /// `POST {endereco}/messages`, chave em `x-api-key`.
    AnthropicMessages,
}

/// Uma ficha de provedor. Guarda só o NOME da chave, nunca o valor.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Provedor {
    pub nome: String,
    pub lingua: Lingua,
    /// Endereço base, já com `/v1` (sem barra no fim).
    pub endereco: String,
    /// Nome da variável no arquivo de chaves do Hermes (ex.: `NVIDIA_API_KEY`).
    pub nome_chave: String,
    /// Modelo usado quando o bot não diz qual. `None` = o bot precisa dizer.
    #[serde(default)]
    pub modelo_padrao: Option<String>,
}

/// As três fichas de fábrica. Sem modelo padrão: nomes de modelo mudam com o
/// tempo e não vamos inventar um; cada bot diz o seu.
pub fn de_fabrica() -> Vec<Provedor> {
    vec![
        Provedor {
            nome: "openai-api".into(),
            lingua: Lingua::OpenaiChat,
            endereco: "https://api.openai.com/v1".into(),
            nome_chave: "OPENAI_API_KEY".into(),
            modelo_padrao: None,
        },
        Provedor {
            nome: "nvidia".into(),
            lingua: Lingua::OpenaiChat,
            endereco: "https://integrate.api.nvidia.com/v1".into(),
            nome_chave: "NVIDIA_API_KEY".into(),
            modelo_padrao: None,
        },
        Provedor {
            nome: "anthropic".into(),
            lingua: Lingua::AnthropicMessages,
            endereco: "https://api.anthropic.com/v1".into(),
            nome_chave: "ANTHROPIC_API_KEY".into(),
            modelo_padrao: None,
        },
    ]
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ArquivoExtra {
    #[serde(default)]
    provedor: Vec<Provedor>,
}

/// O conjunto de fichas: as de fábrica + as do `provedores.toml` opcional.
#[derive(Debug, Clone)]
pub struct Catalogo {
    provedores: Vec<Provedor>,
}

fn nome_simples(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && !s.starts_with('-')
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn nome_de_chave(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
}

/// Só `https://`, ou `http://` para esta máquina (proxy local, testes).
fn endereco_aceito(s: &str) -> bool {
    if let Some(resto) = s.strip_prefix("https://") {
        return !resto.is_empty();
    }
    if let Some(resto) = s.strip_prefix("http://") {
        let host = resto.split(['/', ':']).next().unwrap_or("");
        return host == "127.0.0.1" || host == "localhost" || resto.starts_with("[::1]");
    }
    false
}

impl Catalogo {
    /// Fichas de fábrica + `extra_toml` (conteúdo de um `provedores.toml`, com
    /// tabelas `[[provedor]]`). Nome repetido, nome/endereço/chave inválidos → erro.
    pub fn carregar(extra_toml: Option<&str>) -> Result<Self, String> {
        let mut provedores = de_fabrica();
        if let Some(txt) = extra_toml {
            let extra: ArquivoExtra = toml::from_str(txt)
                .map_err(|e| format!("provedores.toml inválido: {}", e.message()))?;
            provedores.extend(extra.provedor);
        }
        for (i, p) in provedores.iter_mut().enumerate() {
            if !nome_simples(&p.nome) {
                return Err(format!(
                    "ficha {}: nome \"{}\" inválido (use a-z, 0-9 e -)",
                    i + 1,
                    p.nome
                ));
            }
            if !nome_de_chave(&p.nome_chave) {
                return Err(format!(
                    "ficha \"{}\": nome_chave \"{}\" inválido (use A-Z, 0-9 e _)",
                    p.nome, p.nome_chave
                ));
            }
            let end = p.endereco.trim_end_matches('/').to_string();
            if !endereco_aceito(&end) {
                return Err(format!("ficha \"{}\": endereço precisa começar com https:// (ou http:// só para esta máquina)", p.nome));
            }
            p.endereco = end;
        }
        for (i, p) in provedores.iter().enumerate() {
            if provedores[..i].iter().any(|q| q.nome == p.nome) {
                return Err(format!("ficha \"{}\" aparece mais de uma vez", p.nome));
            }
        }
        Ok(Self { provedores })
    }

    pub fn achar(&self, nome: &str) -> Option<&Provedor> {
        self.provedores.iter().find(|p| p.nome == nome)
    }

    pub fn nomes(&self) -> Vec<&str> {
        self.provedores.iter().map(|p| p.nome.as_str()).collect()
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn tres_de_fabrica_com_enderecos_completos() {
        let c = Catalogo::carregar(None).unwrap();
        assert_eq!(c.nomes(), vec!["openai-api", "nvidia", "anthropic"]);
        assert_eq!(
            c.achar("openai-api").unwrap().endereco,
            "https://api.openai.com/v1"
        );
        assert_eq!(
            c.achar("nvidia").unwrap().endereco,
            "https://integrate.api.nvidia.com/v1"
        );
        assert_eq!(
            c.achar("anthropic").unwrap().endereco,
            "https://api.anthropic.com/v1"
        );
        assert_eq!(
            c.achar("anthropic").unwrap().lingua,
            Lingua::AnthropicMessages
        );
        assert_eq!(c.achar("nvidia").unwrap().nome_chave, "NVIDIA_API_KEY");
        assert!(
            c.achar("openai").is_none(),
            "openai sozinho é o OpenRouter no Hermes"
        );
    }

    #[test]
    fn extra_carrega_e_tira_barra_final() {
        let c = Catalogo::carregar(Some(
            "[[provedor]]\nnome = \"openrouter\"\nlingua = \"openai_chat\"\nendereco = \"https://openrouter.ai/api/v1/\"\nnome_chave = \"OPENROUTER_API_KEY\"\nmodelo_padrao = \"x/y\"\n",
        ))
        .unwrap();
        let p = c.achar("openrouter").unwrap();
        assert_eq!(p.endereco, "https://openrouter.ai/api/v1");
        assert_eq!(p.modelo_padrao.as_deref(), Some("x/y"));
    }

    #[test]
    fn recusa_repetido_lingua_ruim_e_endereco_ruim() {
        let rep = "[[provedor]]\nnome = \"nvidia\"\nlingua = \"openai_chat\"\nendereco = \"https://a\"\nnome_chave = \"A\"\n";
        assert!(Catalogo::carregar(Some(rep))
            .unwrap_err()
            .contains("mais de uma vez"));
        let lingua = "[[provedor]]\nnome = \"x\"\nlingua = \"klingon\"\nendereco = \"https://a\"\nnome_chave = \"A\"\n";
        assert!(Catalogo::carregar(Some(lingua)).is_err());
        let http = "[[provedor]]\nnome = \"x\"\nlingua = \"openai_chat\"\nendereco = \"http://exemplo.com/v1\"\nnome_chave = \"A\"\n";
        assert!(Catalogo::carregar(Some(http))
            .unwrap_err()
            .contains("https://"));
        let local = "[[provedor]]\nnome = \"x\"\nlingua = \"openai_chat\"\nendereco = \"http://127.0.0.1:9/v1\"\nnome_chave = \"A\"\n";
        assert!(Catalogo::carregar(Some(local)).is_ok());
        let campo = "[[provedor]]\nnome = \"x\"\nlingua = \"openai_chat\"\nendereco = \"https://a\"\nnome_chave = \"A\"\nchave = \"sk-123\"\n";
        assert!(
            Catalogo::carregar(Some(campo)).is_err(),
            "campo desconhecido (ex.: a chave em si) é recusado"
        );
    }
}
