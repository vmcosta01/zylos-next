//! As duas línguas: OpenAI chat e Anthropic messages.

use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde_json::{json, Value};

use crate::chaves::Segredo;
use crate::provedores::{Lingua, Provedor};

/// Versão da API da Anthropic enviada no cabeçalho `anthropic-version`.
pub const VERSAO_ANTHROPIC: &str = "2023-06-01";
/// Teto de tokens da resposta na língua Anthropic (campo obrigatório lá).
pub const MAX_TOKENS_ANTHROPIC: u32 = 4096;
const TRECHO_MAX: usize = 300;

/// Um pedido a um provedor.
pub struct Pedido<'a> {
    pub provedor: &'a Provedor,
    pub modelo: &'a str,
    /// A personalidade do bot (vai como mensagem de sistema). Vazia = sem sistema.
    pub alma: &'a str,
    pub pergunta: &'a str,
    pub chave: &'a Segredo,
    pub tempo_limite: Duration,
}

/// Falha ao falar com o provedor. Nenhuma variante carrega a chave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErroProvedor {
    TempoEsgotado { segundos: u64 },
    Recusou { status: u16, trecho: String },
    Rede { motivo: String },
    RespostaEstranha { motivo: String },
}

/// Tira a chave de qualquer texto que vá sair (erro, log, resposta).
fn sem_chave(texto: &str, chave: &Segredo) -> String {
    let v = chave.expor();
    if v.is_empty() {
        texto.to_string()
    } else {
        texto.replace(v, "***")
    }
}

fn cortar(texto: &str) -> String {
    let mut s: String = texto.chars().take(TRECHO_MAX).collect();
    if texto.chars().count() > TRECHO_MAX {
        s.push('…');
    }
    s
}

fn valor_sensivel(v: &str) -> Result<HeaderValue, ErroProvedor> {
    let mut h = HeaderValue::from_str(v).map_err(|_| ErroProvedor::Rede {
        motivo: "a chave tem caracteres que não cabem num cabeçalho HTTP".into(),
    })?;
    h.set_sensitive(true);
    Ok(h)
}

/// Monta caminho, cabeçalhos e corpo de cada língua (separado para os testes).
pub fn montar(p: &Pedido<'_>) -> Result<(String, HeaderMap, Value), ErroProvedor> {
    let mut cab = HeaderMap::new();
    cab.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    match p.provedor.lingua {
        Lingua::OpenaiChat => {
            cab.insert(
                AUTHORIZATION,
                valor_sensivel(&format!("Bearer {}", p.chave.expor()))?,
            );
            let mut mensagens = Vec::new();
            if !p.alma.trim().is_empty() {
                mensagens.push(json!({"role": "system", "content": p.alma}));
            }
            mensagens.push(json!({"role": "user", "content": p.pergunta}));
            let corpo = json!({"model": p.modelo, "messages": mensagens});
            Ok((
                format!("{}/chat/completions", p.provedor.endereco),
                cab,
                corpo,
            ))
        }
        Lingua::AnthropicMessages => {
            cab.insert("x-api-key", valor_sensivel(p.chave.expor())?);
            cab.insert(
                "anthropic-version",
                HeaderValue::from_static(VERSAO_ANTHROPIC),
            );
            let mut corpo = json!({
                "model": p.modelo,
                "max_tokens": MAX_TOKENS_ANTHROPIC,
                "messages": [{"role": "user", "content": p.pergunta}],
            });
            if !p.alma.trim().is_empty() {
                corpo["system"] = json!(p.alma);
            }
            Ok((format!("{}/messages", p.provedor.endereco), cab, corpo))
        }
    }
}

/// Tokens que o provedor diz ter gastado na pergunta (entrada + saída), se informou. O gateway
/// não calcula nada: só repassa o `usage` da resposta. `None` quando o provedor não manda.
pub fn extrair_tokens(lingua: Lingua, v: &Value) -> Option<u64> {
    let u = &v["usage"];
    let n = match lingua {
        Lingua::OpenaiChat => u["total_tokens"]
            .as_u64()
            .or_else(|| Some(u["prompt_tokens"].as_u64()? + u["completion_tokens"].as_u64()?)),
        Lingua::AnthropicMessages => {
            Some(u["input_tokens"].as_u64()? + u["output_tokens"].as_u64()?)
        }
    }?;
    // Um provedor que manda lixo (negativo vira None acima; absurdo aqui) não polui a soma.
    (n <= 1_000_000_000).then_some(n)
}

/// Tokens de entrada e de saída separados, como o provedor informou. Só existe quando o provedor
/// informou as duas partes: é o que permite calcular custo (entrada e saída têm preços diferentes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct Uso {
    pub entrada: u64,
    pub saida: u64,
}

/// As duas partes do `usage`, se o provedor informou as duas. `None` caso contrário (por exemplo
/// só `total_tokens`): sem a divisão não há como precificar, e o gateway não inventa uma.
pub fn extrair_uso(lingua: Lingua, v: &Value) -> Option<Uso> {
    let u = &v["usage"];
    let (entrada, saida) = match lingua {
        Lingua::OpenaiChat => (
            u["prompt_tokens"].as_u64()?,
            u["completion_tokens"].as_u64()?,
        ),
        Lingua::AnthropicMessages => (u["input_tokens"].as_u64()?, u["output_tokens"].as_u64()?),
    };
    (entrada <= 1_000_000_000 && saida <= 1_000_000_000).then_some(Uso { entrada, saida })
}

/// Lê o texto da resposta de cada língua.
pub fn extrair(lingua: Lingua, v: &Value) -> Result<String, ErroProvedor> {
    let texto = match lingua {
        Lingua::OpenaiChat => v["choices"][0]["message"]["content"]
            .as_str()
            .map(str::to_string),
        Lingua::AnthropicMessages => v["content"].as_array().map(|partes| {
            partes
                .iter()
                .filter(|p| p["type"] == "text")
                .filter_map(|p| p["text"].as_str())
                .collect::<Vec<_>>()
                .join("")
        }),
    };
    match texto {
        Some(t) if !t.trim().is_empty() => Ok(t),
        _ => Err(ErroProvedor::RespostaEstranha {
            motivo: "a resposta do provedor não tem texto".into(),
        }),
    }
}

/// Faz a pergunta ao provedor e devolve o texto da resposta.
pub async fn perguntar(
    cliente: &reqwest::Client,
    p: Pedido<'_>,
) -> Result<(String, Option<u64>, Option<Uso>), ErroProvedor> {
    let (url, cab, corpo) = montar(&p)?;
    let resp = cliente
        .post(&url)
        .headers(cab)
        .timeout(p.tempo_limite)
        .json(&corpo)
        .send()
        .await
        .map_err(|e| erro_de_rede(e, &p))?;
    let status = resp.status();
    let texto = resp.text().await.map_err(|e| erro_de_rede(e, &p))?;
    if !status.is_success() {
        return Err(ErroProvedor::Recusou {
            status: status.as_u16(),
            trecho: cortar(&sem_chave(&texto, p.chave)),
        });
    }
    let v: Value = serde_json::from_str(&texto).map_err(|_| ErroProvedor::RespostaEstranha {
        motivo: "a resposta do provedor não é JSON".into(),
    })?;
    let tokens = extrair_tokens(p.provedor.lingua, &v);
    let uso = extrair_uso(p.provedor.lingua, &v);
    extrair(p.provedor.lingua, &v).map(|t| (sem_chave(&t, p.chave), tokens, uso))
}

fn erro_de_rede(e: reqwest::Error, p: &Pedido<'_>) -> ErroProvedor {
    erro_de_rede_com(e, p.chave, p.tempo_limite)
}

fn erro_de_rede_com(e: reqwest::Error, chave: &Segredo, tempo_limite: Duration) -> ErroProvedor {
    if e.is_timeout() {
        ErroProvedor::TempoEsgotado {
            segundos: tempo_limite.as_secs(),
        }
    } else {
        // `without_url`: o texto não leva o endereço; `sem_chave` por garantia.
        ErroProvedor::Rede {
            motivo: sem_chave(&e.without_url().to_string(), chave),
        }
    }
}

/// Monta o pedido de listagem de modelos: `GET {endereco}/models`, com a chave
/// no cabeçalho de cada língua. Na Anthropic a lista é paginada; `limit=1000`
/// traz tudo numa página.
pub fn montar_listagem(
    provedor: &Provedor,
    chave: &Segredo,
) -> Result<(String, HeaderMap), ErroProvedor> {
    let mut cab = HeaderMap::new();
    match provedor.lingua {
        Lingua::OpenaiChat => {
            cab.insert(
                AUTHORIZATION,
                valor_sensivel(&format!("Bearer {}", chave.expor()))?,
            );
            Ok((format!("{}/models", provedor.endereco), cab))
        }
        Lingua::AnthropicMessages => {
            cab.insert("x-api-key", valor_sensivel(chave.expor())?);
            cab.insert(
                "anthropic-version",
                HeaderValue::from_static(VERSAO_ANTHROPIC),
            );
            Ok((format!("{}/models?limit=1000", provedor.endereco), cab))
        }
    }
}

/// Lê os ids de modelo de `{"data": [{"id": ...}, ...]}` (formato das duas línguas),
/// em ordem alfabética e sem repetição.
pub fn extrair_modelos(v: &Value) -> Result<Vec<String>, ErroProvedor> {
    let lista = v["data"]
        .as_array()
        .ok_or_else(|| ErroProvedor::RespostaEstranha {
            motivo: "a lista de modelos não tem o campo data".into(),
        })?;
    let mut ids: Vec<String> = lista
        .iter()
        .filter_map(|m| m["id"].as_str())
        .map(str::to_string)
        .collect();
    ids.sort();
    ids.dedup();
    Ok(ids)
}

/// Pergunta ao provedor quais modelos ele oferece.
pub async fn listar_modelos(
    cliente: &reqwest::Client,
    provedor: &Provedor,
    chave: &Segredo,
    tempo_limite: Duration,
) -> Result<Vec<String>, ErroProvedor> {
    let (url, cab) = montar_listagem(provedor, chave)?;
    let resp = cliente
        .get(&url)
        .headers(cab)
        .timeout(tempo_limite)
        .send()
        .await
        .map_err(|e| erro_de_rede_com(e, chave, tempo_limite))?;
    let status = resp.status();
    let texto = resp
        .text()
        .await
        .map_err(|e| erro_de_rede_com(e, chave, tempo_limite))?;
    if !status.is_success() {
        return Err(ErroProvedor::Recusou {
            status: status.as_u16(),
            trecho: cortar(&sem_chave(&texto, chave)),
        });
    }
    let v: Value = serde_json::from_str(&texto).map_err(|_| ErroProvedor::RespostaEstranha {
        motivo: "a lista de modelos não é JSON".into(),
    })?;
    extrair_modelos(&v)
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::provedores::de_fabrica;

    fn pedido<'a>(prov: &'a Provedor, chave: &'a Segredo, alma: &'a str) -> Pedido<'a> {
        Pedido {
            provedor: prov,
            modelo: "m-1",
            alma,
            pergunta: "oi?",
            chave,
            tempo_limite: Duration::from_secs(5),
        }
    }

    #[test]
    fn openai_monta_bearer_sistema_e_caminho() {
        let fab = de_fabrica();
        let chave = crate::chaves::tests_segredo("sk-x");
        let (url, cab, corpo) = montar(&pedido(&fab[1], &chave, "Você é o Vigia.")).unwrap();
        assert_eq!(url, "https://integrate.api.nvidia.com/v1/chat/completions");
        assert_eq!(cab[AUTHORIZATION], "Bearer sk-x");
        assert!(cab[AUTHORIZATION].is_sensitive());
        assert_eq!(corpo["model"], "m-1");
        assert_eq!(
            corpo["messages"][0],
            json!({"role": "system", "content": "Você é o Vigia."})
        );
        assert_eq!(
            corpo["messages"][1],
            json!({"role": "user", "content": "oi?"})
        );
    }

    #[test]
    fn anthropic_monta_x_api_key_versao_e_system() {
        let fab = de_fabrica();
        let chave = crate::chaves::tests_segredo("sk-ant-api-x");
        let (url, cab, corpo) = montar(&pedido(&fab[2], &chave, "Alma")).unwrap();
        assert_eq!(url, "https://api.anthropic.com/v1/messages");
        assert_eq!(cab["x-api-key"], "sk-ant-api-x");
        assert!(cab["x-api-key"].is_sensitive());
        assert_eq!(cab["anthropic-version"], VERSAO_ANTHROPIC);
        assert!(cab.get(AUTHORIZATION).is_none());
        assert_eq!(corpo["system"], "Alma");
        assert_eq!(corpo["max_tokens"], MAX_TOKENS_ANTHROPIC);
        assert_eq!(
            corpo["messages"],
            json!([{"role": "user", "content": "oi?"}])
        );
    }

    #[test]
    fn alma_vazia_nao_manda_sistema() {
        let fab = de_fabrica();
        let chave = crate::chaves::tests_segredo("k");
        let (_, _, c) = montar(&pedido(&fab[0], &chave, "  ")).unwrap();
        assert_eq!(c["messages"].as_array().unwrap().len(), 1);
        let (_, _, c) = montar(&pedido(&fab[2], &chave, "")).unwrap();
        assert!(c.get("system").is_none());
    }

    #[test]
    fn extrai_texto_das_duas_linguas() {
        let o = json!({"choices": [{"message": {"content": "olá"}}]});
        assert_eq!(extrair(Lingua::OpenaiChat, &o).unwrap(), "olá");
        let a = json!({"content": [{"type": "text", "text": "a"}, {"type": "tool_use"}, {"type": "text", "text": "b"}]});
        assert_eq!(extrair(Lingua::AnthropicMessages, &a).unwrap(), "ab");
        assert!(extrair(Lingua::OpenaiChat, &json!({})).is_err());
    }

    #[test]
    fn extrai_uso_so_quando_tem_entrada_e_saida() {
        let o = json!({"usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}});
        assert_eq!(
            extrair_uso(Lingua::OpenaiChat, &o),
            Some(Uso {
                entrada: 10,
                saida: 5
            })
        );
        let a = json!({"usage": {"input_tokens": 7, "output_tokens": 3}});
        assert_eq!(
            extrair_uso(Lingua::AnthropicMessages, &a),
            Some(Uso {
                entrada: 7,
                saida: 3
            })
        );
        // Só o total, só uma parte, negativo ou absurdo: None (não inventa a divisão).
        assert_eq!(
            extrair_uso(Lingua::OpenaiChat, &json!({"usage": {"total_tokens": 15}})),
            None
        );
        assert_eq!(
            extrair_uso(
                Lingua::AnthropicMessages,
                &json!({"usage": {"input_tokens": 7}})
            ),
            None
        );
        assert_eq!(
            extrair_uso(
                Lingua::OpenaiChat,
                &json!({"usage": {"prompt_tokens": -1, "completion_tokens": 2}})
            ),
            None
        );
        assert_eq!(
            extrair_uso(
                Lingua::OpenaiChat,
                &json!({"usage": {"prompt_tokens": 9_000_000_000u64, "completion_tokens": 2}})
            ),
            None
        );
    }

    #[test]
    fn extrai_tokens_das_duas_linguas() {
        let o = json!({"usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}});
        assert_eq!(extrair_tokens(Lingua::OpenaiChat, &o), Some(15));
        let so_partes = json!({"usage": {"prompt_tokens": 10, "completion_tokens": 5}});
        assert_eq!(extrair_tokens(Lingua::OpenaiChat, &so_partes), Some(15));
        let a = json!({"usage": {"input_tokens": 7, "output_tokens": 3}});
        assert_eq!(extrair_tokens(Lingua::AnthropicMessages, &a), Some(10));
        // Sem `usage`, só uma parte, negativo, texto ou absurdo: None (não inventa).
        assert_eq!(extrair_tokens(Lingua::OpenaiChat, &json!({})), None);
        assert_eq!(
            extrair_tokens(
                Lingua::AnthropicMessages,
                &json!({"usage": {"input_tokens": 7}})
            ),
            None
        );
        assert_eq!(
            extrair_tokens(Lingua::OpenaiChat, &json!({"usage": {"total_tokens": -1}})),
            None
        );
        assert_eq!(
            extrair_tokens(
                Lingua::OpenaiChat,
                &json!({"usage": {"total_tokens": "muitos"}})
            ),
            None
        );
        assert_eq!(
            extrair_tokens(
                Lingua::OpenaiChat,
                &json!({"usage": {"total_tokens": 9_000_000_000u64}})
            ),
            None
        );
    }

    #[test]
    fn listagem_das_duas_linguas() {
        let fab = de_fabrica();
        let chave = crate::chaves::tests_segredo("k-1");
        let (url, cab) = montar_listagem(&fab[1], &chave).unwrap();
        assert_eq!(url, "https://integrate.api.nvidia.com/v1/models");
        assert_eq!(cab[AUTHORIZATION], "Bearer k-1");
        assert!(cab[AUTHORIZATION].is_sensitive());
        let (url, cab) = montar_listagem(&fab[2], &chave).unwrap();
        assert_eq!(url, "https://api.anthropic.com/v1/models?limit=1000");
        assert_eq!(cab["x-api-key"], "k-1");
        assert!(cab["x-api-key"].is_sensitive());
        assert_eq!(cab["anthropic-version"], VERSAO_ANTHROPIC);
        assert!(cab.get(AUTHORIZATION).is_none());
    }

    #[test]
    fn extrai_modelos_ordenados_sem_repeticao() {
        let v = json!({"data": [{"id": "b"}, {"id": "a"}, {"id": "b"}, {"sem": "id"}]});
        assert_eq!(extrair_modelos(&v).unwrap(), vec!["a", "b"]);
        assert!(extrair_modelos(&json!({"models": []})).is_err());
    }

    #[test]
    fn trecho_corta_e_tira_a_chave() {
        let chave = crate::chaves::tests_segredo("sk-abc");
        assert_eq!(
            sem_chave("chave sk-abc invalida", &chave),
            "chave *** invalida"
        );
        let longo = "x".repeat(400);
        assert_eq!(cortar(&longo).chars().count(), TRECHO_MAX + 1);
    }
}
