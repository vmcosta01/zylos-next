//! Ponta a ponta contra um provedor falso nesta máquina: bot no disco, chave
//! num `.env` falso, fichas extras apontando para o servidor de teste.

use std::path::Path;
use std::time::Duration;

use axum::{extract::Json, http::HeaderMap, routing::post, Router};
use serde_json::{json, Value};
use zylos_pensar::{perguntar, Config, ErroPensar};

const CHAVE: &str = "sk-falsa-0123456789";

async fn openai(cab: HeaderMap, Json(c): Json<Value>) -> Json<Value> {
    let auth = cab.get("authorization").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    let msgs = c["messages"].as_array().cloned().unwrap_or_default();
    let sys = msgs.iter().find(|m| m["role"] == "system").map(|m| m["content"].clone()).unwrap_or(json!(null));
    // `veio_do_arquivo` diz qual chave chegou sem repetir o valor (o zylos apaga o valor da resposta).
    let veio_do_arquivo = auth == format!("Bearer {CHAVE}");
    let txt = format!(
        "auth={auth}|veio_do_arquivo={veio_do_arquivo}|sys={sys}|user={}|model={}",
        msgs.last().unwrap()["content"],
        c["model"]
    );
    Json(json!({"choices": [{"message": {"role": "assistant", "content": txt}}]}))
}

async fn anthropic(cab: HeaderMap, Json(c): Json<Value>) -> Json<Value> {
    let h = |n: &str| cab.get(n).and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    let txt = format!("key={}|ver={}|sys={}|model={}", h("x-api-key"), h("anthropic-version"), c["system"], c["model"]);
    Json(json!({"content": [{"type": "text", "text": txt}]}))
}

async fn recusa(cab: HeaderMap) -> (axum::http::StatusCode, String) {
    let auth = cab.get("authorization").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    (axum::http::StatusCode::UNAUTHORIZED, format!("chave invalida: {auth}"))
}

async fn lento() -> Json<Value> {
    tokio::time::sleep(Duration::from_secs(5)).await;
    Json(json!({}))
}

async fn subir() -> String {
    let app = Router::new()
        .route("/v1/chat/completions", post(openai))
        .route("/v1/messages", post(anthropic))
        .route("/recusa/chat/completions", post(recusa))
        .route("/lento/chat/completions", post(lento));
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let end = format!("http://127.0.0.1:{}", l.local_addr().unwrap().port());
    tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
    end
}

fn ficha(nome: &str, lingua: &str, endereco: &str, chave: &str) -> String {
    format!("[[provedor]]\nnome = \"{nome}\"\nlingua = \"{lingua}\"\nendereco = \"{endereco}\"\nnome_chave = \"{chave}\"\n")
}

fn bot(pasta: &Path, nome: &str, provedor: &str, modelo: Option<&str>, alma: &str) {
    let d = pasta.join(nome);
    std::fs::create_dir_all(&d).unwrap();
    let m = modelo.map(|m| format!("modelo = \"{m}\"\n")).unwrap_or_default();
    std::fs::write(d.join("bot.toml"), format!("provedor = \"{provedor}\"\n{m}")).unwrap();
    std::fs::write(d.join("ALMA.md"), alma).unwrap();
}

struct Cena {
    _dir: tempfile::TempDir,
    cfg: Config,
}

async fn cena() -> Cena {
    let end = subir().await;
    let dir = tempfile::tempdir().unwrap();
    let bots = dir.path().join("bots");
    std::fs::create_dir_all(&bots).unwrap();
    let extra = [
        ficha("falso-openai", "openai_chat", &format!("{end}/v1"), "FALSA_KEY"),
        ficha("falso-anthropic", "anthropic_messages", &format!("{end}/v1"), "FALSA_KEY"),
        ficha("falso-recusa", "openai_chat", &format!("{end}/recusa"), "FALSA_KEY"),
        ficha("falso-lento", "openai_chat", &format!("{end}/lento"), "FALSA_KEY"),
        ficha("falso-sem-chave", "openai_chat", &format!("{end}/v1"), "NAO_EXISTE_KEY"),
        ficha("falso-oat", "anthropic_messages", &format!("{end}/v1"), "OAT_KEY"),
    ]
    .join("\n");
    std::fs::write(dir.path().join("provedores.toml"), extra).unwrap();
    std::fs::write(dir.path().join(".env"), format!("FALSA_KEY={CHAVE}\r\nOAT_KEY=sk-ant-oat01-xyz\r\n")).unwrap();
    bot(&bots, "vigia", "falso-openai", Some("m-openai"), "Você é o Vigia.");
    bot(&bots, "poeta", "falso-anthropic", Some("m-claude"), "Você é o Poeta.");
    bot(&bots, "recusado", "falso-recusa", Some("m"), "a");
    bot(&bots, "lerdo", "falso-lento", Some("m"), "a");
    bot(&bots, "sem-chave", "falso-sem-chave", Some("m"), "a");
    bot(&bots, "oat", "falso-oat", Some("m"), "a");
    bot(&bots, "sem-modelo", "falso-openai", None, "a");
    bot(&bots, "fantasma", "nao-existe", Some("m"), "a");
    let cfg = Config {
        pasta_bots: bots,
        arquivo_chaves: dir.path().join(".env"),
        provedores_extra: Some(dir.path().join("provedores.toml")),
        tempo_limite: Duration::from_secs(1),
    };
    Cena { _dir: dir, cfg }
}

fn sem_ambiente(_: &str) -> Option<String> {
    None
}

#[tokio::test]
async fn openai_responde_com_personalidade_e_chave_do_arquivo() {
    let c = cena().await;
    let r = perguntar(&c.cfg, &reqwest::Client::new(), &sem_ambiente, "vigia", "tudo bem?").await.unwrap();
    assert_eq!(r.bot, "vigia");
    assert_eq!(r.provedor, "falso-openai");
    assert_eq!(r.modelo, "m-openai");
    assert!(r.resposta.contains("sys=\"Você é o Vigia.\""), "{}", r.resposta);
    assert!(r.resposta.contains("user=\"tudo bem?\""), "{}", r.resposta);
    assert!(r.resposta.contains("model=\"m-openai\""), "{}", r.resposta);
    // A chave foi enviada (auth=Bearer ...), mas não volta na resposta.
    assert!(r.resposta.contains("auth=Bearer ***"), "{}", r.resposta);
    assert!(r.resposta.contains("veio_do_arquivo=true"), "{}", r.resposta);
    assert!(!r.resposta.contains(CHAVE));
}

#[tokio::test]
async fn anthropic_responde_com_x_api_key_versao_e_system() {
    let c = cena().await;
    let r = perguntar(&c.cfg, &reqwest::Client::new(), &sem_ambiente, "poeta", "um verso").await.unwrap();
    assert!(r.resposta.contains("key=***"), "{}", r.resposta);
    assert!(r.resposta.contains("ver=2023-06-01"), "{}", r.resposta);
    assert!(r.resposta.contains("sys=\"Você é o Poeta.\""), "{}", r.resposta);
    assert!(r.resposta.contains("model=\"m-claude\""), "{}", r.resposta);
}

#[tokio::test]
async fn arquivo_do_hermes_vence_o_ambiente() {
    let c = cena().await;
    let ambiente = |n: &str| (n == "FALSA_KEY").then(|| "do-ambiente".to_string());
    let r = perguntar(&c.cfg, &reqwest::Client::new(), &ambiente, "vigia", "x").await.unwrap();
    assert!(r.resposta.contains("veio_do_arquivo=true"), "tinha de usar a do arquivo: {}", r.resposta);
}

#[tokio::test]
async fn recusa_do_provedor_nao_vaza_a_chave() {
    let c = cena().await;
    let e = perguntar(&c.cfg, &reqwest::Client::new(), &sem_ambiente, "recusado", "x").await.unwrap_err();
    match &e {
        ErroPensar::ProvedorRecusou { status, trecho } => {
            assert_eq!(*status, 401);
            assert!(trecho.contains("chave invalida: Bearer ***"), "{trecho}");
        }
        outro => panic!("esperava ProvedorRecusou, veio {outro:?}"),
    }
    assert!(!e.to_string().contains(CHAVE));
    assert!(!format!("{e:?}").contains(CHAVE));
}

#[tokio::test]
async fn tempo_esgotado() {
    let c = cena().await;
    let e = perguntar(&c.cfg, &reqwest::Client::new(), &sem_ambiente, "lerdo", "x").await.unwrap_err();
    assert_eq!(e, ErroPensar::TempoEsgotado(1));
}

#[tokio::test]
async fn erros_de_configuracao() {
    let c = cena().await;
    let cl = reqwest::Client::new();
    let e = perguntar(&c.cfg, &cl, &sem_ambiente, "sem-chave", "x").await.unwrap_err();
    assert!(matches!(&e, ErroPensar::ChaveFaltando { nome, .. } if nome == "NAO_EXISTE_KEY"), "{e:?}");
    let e = perguntar(&c.cfg, &cl, &sem_ambiente, "oat", "x").await.unwrap_err();
    assert!(matches!(&e, ErroPensar::ChaveInvalida { motivo, .. } if motivo.contains("assinatura")), "{e:?}");
    assert!(!e.to_string().contains("sk-ant-oat01-xyz"));
    let e = perguntar(&c.cfg, &cl, &sem_ambiente, "sem-modelo", "x").await.unwrap_err();
    assert_eq!(e, ErroPensar::SemModelo("falso-openai".into()));
    let e = perguntar(&c.cfg, &cl, &sem_ambiente, "fantasma", "x").await.unwrap_err();
    assert!(matches!(&e, ErroPensar::ProvedorDesconhecido { provedor, existentes } if provedor == "nao-existe" && existentes.contains("nvidia")));
    assert_eq!(perguntar(&c.cfg, &cl, &sem_ambiente, "ninguem", "x").await.unwrap_err(), ErroPensar::BotInexistente("ninguem".into()));
    assert_eq!(perguntar(&c.cfg, &cl, &sem_ambiente, "..", "x").await.unwrap_err(), ErroPensar::NomeInvalido);
    assert_eq!(perguntar(&c.cfg, &cl, &sem_ambiente, "vigia", "  ").await.unwrap_err(), ErroPensar::PerguntaVazia);
}

#[test]
fn config_do_ambiente() {
    let amb = |n: &str| match n {
        "USERPROFILE" => Some("C:\\U".to_string()),
        "LOCALAPPDATA" => Some("C:\\L".to_string()),
        "ZYLOS_PENSAR_TEMPO_LIMITE_S" => Some("7".to_string()),
        _ => None,
    };
    let c = Config::de(&amb);
    assert_eq!(c.pasta_bots, Path::new("C:\\U").join(".zylos").join("bots"));
    assert_eq!(c.arquivo_chaves, Path::new("C:\\L").join("hermes").join(".env"));
    assert_eq!(c.tempo_limite, Duration::from_secs(7));
    let ruim = |n: &str| (n == "ZYLOS_PENSAR_TEMPO_LIMITE_S").then(|| "9999".to_string());
    assert_eq!(Config::de(&ruim).tempo_limite, zylos_pensar::TEMPO_LIMITE_PADRAO);
}
