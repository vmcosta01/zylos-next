//! zylos-pensar — o zylos pensa sozinho.
//!
//! Um bot é uma pasta com `bot.toml` (provedor + modelo) e `ALMA.md`
//! (personalidade). Na hora de pensar, o zylos acha a ficha do provedor, lê a
//! chave no arquivo de chaves do Hermes e fala a língua do provedor.
//! Nada de memória, ferramentas ou rotinas nesta fatia.

pub mod bots;
pub mod chaves;
pub mod linguas;
pub mod provedores;
pub mod texto;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::chaves::ErroChave;
use crate::linguas::ErroProvedor;
use crate::provedores::{Catalogo, Lingua, Provedor};

/// Tempo-limite padrão de uma chamada ao provedor.
pub const TEMPO_LIMITE_PADRAO: Duration = Duration::from_secs(120);
/// Tamanho máximo de uma pergunta.
pub const LIMITE_PERGUNTA: usize = 32 * 1024;

/// Onde ficam as coisas.
#[derive(Debug, Clone)]
pub struct Config {
    /// Pasta com uma subpasta por bot.
    pub pasta_bots: PathBuf,
    /// Arquivo de chaves do Hermes (`.env`).
    pub arquivo_chaves: PathBuf,
    /// `provedores.toml` opcional com fichas extras.
    pub provedores_extra: Option<PathBuf>,
    pub tempo_limite: Duration,
}

impl Config {
    /// A partir de um leitor de ambiente:
    /// - bots: `ZYLOS_BOTS_DIR`, senão `%USERPROFILE%\.zylos\bots`;
    /// - fichas extras: `ZYLOS_PROVEDORES`, senão `%USERPROFILE%\.zylos\provedores.toml`;
    /// - chaves: `<pasta do Hermes>\.env`;
    /// - tempo-limite: `ZYLOS_PENSAR_TEMPO_LIMITE_S` (1..=600), senão 120 s.
    pub fn de(ambiente: &dyn Fn(&str) -> Option<String>) -> Self {
        let casa_zylos = ambiente("USERPROFILE")
            .or_else(|| ambiente("HOME"))
            .map(|c| PathBuf::from(c).join(".zylos"))
            .unwrap_or_else(|| PathBuf::from(".zylos"));
        let pasta_bots = ambiente("ZYLOS_BOTS_DIR")
            .filter(|s| !s.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| casa_zylos.join("bots"));
        let provedores_extra = Some(
            ambiente("ZYLOS_PROVEDORES")
                .filter(|s| !s.trim().is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| casa_zylos.join("provedores.toml")),
        );
        let arquivo_chaves = chaves::pasta_hermes(ambiente)
            .unwrap_or_else(|| PathBuf::from("hermes"))
            .join(".env");
        let tempo_limite = ambiente("ZYLOS_PENSAR_TEMPO_LIMITE_S")
            .and_then(|s| s.trim().parse::<u64>().ok())
            .filter(|s| (1..=600).contains(s))
            .map(Duration::from_secs)
            .unwrap_or(TEMPO_LIMITE_PADRAO);
        Self {
            pasta_bots,
            arquivo_chaves,
            provedores_extra,
            tempo_limite,
        }
    }

    /// Do ambiente do processo.
    pub fn do_ambiente() -> Self {
        Self::de(&|n| std::env::var(n).ok())
    }
}

/// Resposta de um bot.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Resposta {
    pub resposta: String,
    pub bot: String,
    pub provedor: String,
    pub modelo: String,
    pub duracao_ms: u64,
}

/// Tudo o que pode dar errado. Nenhuma variante carrega o valor de uma chave.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ErroPensar {
    #[error("nome de bot inválido: use só letras minúsculas, números e -")]
    NomeInvalido,
    #[error("a pergunta está vazia")]
    PerguntaVazia,
    #[error("a pergunta passa do limite de {LIMITE_PERGUNTA} bytes")]
    PerguntaGrande,
    #[error("bot \"{0}\" não existe na pasta dos bots")]
    BotInexistente(String),
    #[error("bot \"{bot}\" mal configurado: {motivo}")]
    BotMalFeito { bot: String, motivo: String },
    #[error("fichas de provedor com problema: {0}")]
    Catalogo(String),
    #[error("o bot usa o provedor \"{provedor}\", que não existe; os que existem: {existentes}")]
    ProvedorDesconhecido {
        provedor: String,
        existentes: String,
    },
    #[error("o provedor \"{provedor}\" não existe; os que existem: {existentes}")]
    ProvedorNaoExiste {
        provedor: String,
        existentes: String,
    },
    #[error("o bot não diz o modelo e o provedor \"{0}\" não tem modelo padrão; ponha modelo = \"...\" no bot.toml")]
    SemModelo(String),
    #[error("{nome} não está no arquivo de chaves do Hermes ({arquivo}). Pode estar no auth.json ou num perfil do Hermes, que esta versão não lê.")]
    ChaveFaltando { nome: String, arquivo: String },
    #[error("a chave {nome} não pode ser usada: {motivo}")]
    ChaveInvalida { nome: String, motivo: String },
    #[error("o provedor recusou (HTTP {status}): {trecho}")]
    ProvedorRecusou { status: u16, trecho: String },
    #[error("o provedor não respondeu em {0} s")]
    TempoEsgotado(u64),
    #[error("falha de rede ao falar com o provedor: {0}")]
    Rede(String),
    #[error("resposta inesperada do provedor: {0}")]
    RespostaEstranha(String),
}

/// Fichas de fábrica + `provedores.toml` (se existir).
fn carregar_catalogo(cfg: &Config) -> Result<Catalogo, ErroPensar> {
    let extra = match &cfg.provedores_extra {
        Some(p) => match texto::ler(p, Some(256 * 1024)) {
            Ok(t) => Some(t),
            Err(texto::ErroTexto::Inexistente) => None,
            Err(e) => return Err(ErroPensar::Catalogo(format!("provedores.toml: {e}"))),
        },
        None => None,
    };
    Catalogo::carregar(extra.as_deref()).map_err(ErroPensar::Catalogo)
}

/// A chave do provedor, pronta para uso (ou o motivo de não poder ser usada).
fn chave_do_provedor(
    cfg: &Config,
    provedor: &Provedor,
    ambiente: &(dyn Fn(&str) -> Option<String> + Sync),
) -> Result<chaves::Segredo, ErroPensar> {
    let chave = chaves::ler_chave(&cfg.arquivo_chaves, &provedor.nome_chave, &|n| ambiente(n))
        .map_err(|e| match e {
            ErroChave::Faltando { nome } => ErroPensar::ChaveFaltando {
                nome,
                arquivo: cfg.arquivo_chaves.display().to_string(),
            },
            ErroChave::ComExpansao { nome } => ErroPensar::ChaveInvalida {
                nome,
                motivo: "o valor usa ${...}, e o zylos não expande variáveis".into(),
            },
            ErroChave::ArquivoIlegivel { motivo } => ErroPensar::ChaveInvalida {
                nome: provedor.nome_chave.clone(),
                motivo: format!("arquivo de chaves ilegível: {motivo}"),
            },
        })?;
    if provedor.lingua == Lingua::AnthropicMessages && chave.expor().starts_with("sk-ant-oat") {
        return Err(ErroPensar::ChaveInvalida {
            nome: provedor.nome_chave.clone(),
            motivo: "é um token de assinatura do Claude, não uma chave de API; assinatura fica para outra fatia".into(),
        });
    }
    Ok(chave)
}

/// Um provedor como aparece na lista: a ficha + se a chave está pronta para uso.
/// Nunca traz o valor da chave.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct InfoProvedor {
    pub nome: String,
    pub lingua: Lingua,
    pub endereco: String,
    pub nome_chave: String,
    pub modelo_padrao: Option<String>,
    pub chave_pronta: bool,
    /// Por que a chave não está pronta (sem o valor), quando não está.
    pub problema_da_chave: Option<String>,
}

/// Todos os provedores, com o estado da chave de cada um. Não chama ninguém na rede.
pub fn listar_provedores(
    cfg: &Config,
    ambiente: &(dyn Fn(&str) -> Option<String> + Sync),
) -> Result<Vec<InfoProvedor>, ErroPensar> {
    let catalogo = carregar_catalogo(cfg)?;
    Ok(catalogo
        .nomes()
        .into_iter()
        .filter_map(|n| catalogo.achar(n))
        .map(|p| {
            let estado = chave_do_provedor(cfg, p, ambiente);
            InfoProvedor {
                nome: p.nome.clone(),
                lingua: p.lingua,
                endereco: p.endereco.clone(),
                nome_chave: p.nome_chave.clone(),
                modelo_padrao: p.modelo_padrao.clone(),
                chave_pronta: estado.is_ok(),
                problema_da_chave: estado.err().map(|e| e.to_string()),
            }
        })
        .collect())
}

/// Modelos que o provedor `nome_provedor` oferece, perguntando a ele.
pub async fn listar_modelos(
    cfg: &Config,
    cliente: &reqwest::Client,
    ambiente: &(dyn Fn(&str) -> Option<String> + Sync),
    nome_provedor: &str,
) -> Result<Vec<String>, ErroPensar> {
    let catalogo = carregar_catalogo(cfg)?;
    let provedor = catalogo
        .achar(nome_provedor)
        .ok_or_else(|| ErroPensar::ProvedorNaoExiste {
            provedor: nome_provedor.to_string(),
            existentes: catalogo.nomes().join(", "),
        })?;
    let chave = chave_do_provedor(cfg, provedor, ambiente)?;
    linguas::listar_modelos(cliente, provedor, &chave, cfg.tempo_limite)
        .await
        .map_err(erro_do_provedor)
}

fn erro_do_provedor(e: ErroProvedor) -> ErroPensar {
    match e {
        ErroProvedor::TempoEsgotado { segundos } => ErroPensar::TempoEsgotado(segundos),
        ErroProvedor::Recusou { status, trecho } => ErroPensar::ProvedorRecusou { status, trecho },
        ErroProvedor::Rede { motivo } => ErroPensar::Rede(motivo),
        ErroProvedor::RespostaEstranha { motivo } => ErroPensar::RespostaEstranha(motivo),
    }
}

/// Faz `pergunta` ao bot `nome_bot`.
pub async fn perguntar(
    cfg: &Config,
    cliente: &reqwest::Client,
    ambiente: &(dyn Fn(&str) -> Option<String> + Sync),
    nome_bot: &str,
    pergunta: &str,
) -> Result<Resposta, ErroPensar> {
    if !bots::nome_valido(nome_bot) {
        return Err(ErroPensar::NomeInvalido);
    }
    if pergunta.trim().is_empty() {
        return Err(ErroPensar::PerguntaVazia);
    }
    if pergunta.len() > LIMITE_PERGUNTA {
        return Err(ErroPensar::PerguntaGrande);
    }
    let bot = bots::carregar(&cfg.pasta_bots, nome_bot).map_err(|e| match e {
        bots::ErroBot::NomeInvalido => ErroPensar::NomeInvalido,
        bots::ErroBot::Inexistente => ErroPensar::BotInexistente(nome_bot.to_string()),
        bots::ErroBot::MalFeito(m) => ErroPensar::BotMalFeito {
            bot: nome_bot.to_string(),
            motivo: m,
        },
    })?;

    let catalogo = carregar_catalogo(cfg)?;
    let provedor =
        catalogo
            .achar(&bot.provedor)
            .ok_or_else(|| ErroPensar::ProvedorDesconhecido {
                provedor: bot.provedor.clone(),
                existentes: catalogo.nomes().join(", "),
            })?;
    let modelo = bot
        .modelo
        .clone()
        .or_else(|| provedor.modelo_padrao.clone())
        .ok_or_else(|| ErroPensar::SemModelo(provedor.nome.clone()))?;

    let chave = chave_do_provedor(cfg, provedor, ambiente)?;

    let inicio = Instant::now();
    let texto = linguas::perguntar(
        cliente,
        linguas::Pedido {
            provedor,
            modelo: &modelo,
            alma: &bot.alma,
            pergunta,
            chave: &chave,
            tempo_limite: cfg.tempo_limite,
        },
    )
    .await
    .map_err(erro_do_provedor)?;
    Ok(Resposta {
        resposta: texto,
        bot: bot.nome,
        provedor: provedor.nome.clone(),
        modelo,
        duracao_ms: inicio.elapsed().as_millis() as u64,
    })
}
