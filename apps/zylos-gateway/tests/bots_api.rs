//! `POST /api/v1/bots/:nome/perguntar` contra um provedor falso nesta máquina.

use std::time::Duration;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use axum::{routing::post, Json, Router};
use serde_json::{json, Value};
use tower::ServiceExt;
use zylos_gateway::bots_api::create_bots_router_with_bus;
use zylos_pensar::Config;

const CHAVE: &str = "sk-falsa-gateway-42";

async fn subir_falso() -> String {
    let app = Router::new()
        .route(
            "/v1/chat/completions",
            post(|Json(c): Json<Value>| async move {
                let sys = c["messages"][0]["content"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                Json(json!({"choices": [{"message": {"content": format!("[{sys}] ok")}}]}))
            }),
        )
        .route(
            "/v1/models",
            axum::routing::get(|| async { Json(json!({"data": [{"id": "m-2"}, {"id": "m-1"}]})) }),
        )
        .route(
            "/recusa/chat/completions",
            post(|| async { (StatusCode::UNAUTHORIZED, format!("bad key {CHAVE}")) }),
        )
        .route(
            "/lento/chat/completions",
            post(|| async {
                tokio::time::sleep(Duration::from_secs(5)).await;
                Json(json!({}))
            }),
        );
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let end = format!("http://127.0.0.1:{}", l.local_addr().unwrap().port());
    tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
    end
}

async fn roteador() -> (tempfile::TempDir, Router) {
    roteador_com_bus(None).await
}

async fn roteador_com_bus(bus: Option<zylos_live_comm::EventBus>) -> (tempfile::TempDir, Router) {
    let end = subir_falso().await;
    let d = tempfile::tempdir().unwrap();
    let ficha = |nome: &str, caminho: &str, chave: &str| {
        format!("[[provedor]]\nnome = \"{nome}\"\nlingua = \"openai_chat\"\nendereco = \"{end}/{caminho}\"\nnome_chave = \"{chave}\"\n")
    };
    std::fs::write(
        d.path().join("provedores.toml"),
        [
            ficha("falso", "v1", "FALSA_KEY"),
            ficha("recusa", "recusa", "FALSA_KEY"),
            ficha("lento", "lento", "FALSA_KEY"),
            ficha("sem", "v1", "SEM_KEY"),
        ]
        .join("\n"),
    )
    .unwrap();
    std::fs::write(d.path().join(".env"), format!("FALSA_KEY={CHAVE}\n")).unwrap();
    for (nome, prov, modelo) in [
        ("vigia", "falso", Some("m")),
        ("recusado", "recusa", Some("m")),
        ("lerdo", "lento", Some("m")),
        ("sem-chave", "sem", Some("m")),
        ("sem-modelo", "falso", None),
    ] {
        let b = d.path().join("bots").join(nome);
        std::fs::create_dir_all(&b).unwrap();
        let m = modelo
            .map(|m| format!("modelo = \"{m}\"\n"))
            .unwrap_or_default();
        let m = if nome == "vigia" {
            format!("{m}nome_exibicao = \"Vigia da Noite\"\n")
        } else {
            m
        };
        std::fs::write(b.join("bot.toml"), format!("provedor = \"{prov}\"\n{m}")).unwrap();
        std::fs::write(b.join("ALMA.md"), format!("Sou o {nome}")).unwrap();
    }
    let cfg = Config {
        pasta_bots: d.path().join("bots"),
        arquivo_chaves: d.path().join(".env"),
        provedores_extra: Some(d.path().join("provedores.toml")),
        tempo_limite: Duration::from_secs(1),
    };
    (
        d,
        Router::new().nest("/api/v1", create_bots_router_with_bus(cfg, bus)),
    )
}

async fn perguntar(app: &Router, nome: &str, corpo: &str) -> (StatusCode, String, Value) {
    let req = Request::builder()
        .method(Method::POST)
        .uri(format!("/api/v1/bots/{nome}/perguntar"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(corpo.to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let st = resp.status();
    let tipo = resp
        .headers()
        .get(header::CONTENT_TYPE)
        .map(|v| v.to_str().unwrap().to_string())
        .unwrap_or_default();
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    let txt = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(!txt.contains(CHAVE), "a chave vazou na resposta: {txt}");
    (
        st,
        tipo,
        serde_json::from_str(&txt).unwrap_or(json!({"cru": txt})),
    )
}

async fn pegar(app: &Router, uri: &str) -> (StatusCode, Value) {
    let req = Request::builder()
        .method(Method::GET)
        .uri(uri)
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let st = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    let txt = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(!txt.contains(CHAVE), "a chave vazou: {txt}");
    (st, serde_json::from_str(&txt).unwrap())
}

#[tokio::test]
async fn lista_bots_sem_alma() {
    let (_d, app) = roteador().await;
    let (st, v) = pegar(&app, "/api/v1/bots").await;
    assert_eq!(st, StatusCode::OK);
    let lista = v["bots"].as_array().unwrap();
    let nomes: Vec<_> = lista.iter().map(|b| b["nome"].as_str().unwrap()).collect();
    assert_eq!(
        nomes,
        ["lerdo", "recusado", "sem-chave", "sem-modelo", "vigia"]
    );
    let vigia = lista.iter().find(|b| b["nome"] == "vigia").unwrap();
    assert_eq!(vigia["provedor"], "falso");
    assert_eq!(vigia["modelo"], "m");
    assert_eq!(vigia["problema"], Value::Null);
    assert_eq!(vigia["nome_exibicao"], "Vigia da Noite");
    let lerdo = lista.iter().find(|b| b["nome"] == "lerdo").unwrap();
    assert_eq!(lerdo["nome_exibicao"], Value::Null);
    assert!(
        !v.to_string().contains("Sou o"),
        "a ALMA.md não pode sair na lista"
    );
}

#[tokio::test]
async fn estado_do_bot_acompanha_a_pergunta_em_andamento() {
    let (_d, app) = roteador().await;
    let estado = |v: &Value, nome: &str| {
        v["bots"]
            .as_array()
            .unwrap()
            .iter()
            .find(|b| b["nome"] == nome)
            .unwrap()["estado"]
            .clone()
    };
    let (_, v) = pegar(&app, "/api/v1/bots").await;
    assert_eq!(estado(&v, "lerdo"), "ocioso");

    // O provedor "lento" demora 5 s e o tempo limite é 1 s: dá uma janela para ver o estado.
    let app2 = app.clone();
    let em_andamento =
        tokio::spawn(async move { perguntar(&app2, "lerdo", r#"{"pergunta":"oi"}"#).await });
    let mut visto = false;
    for _ in 0..40 {
        let (_, v) = pegar(&app, "/api/v1/bots").await;
        if estado(&v, "lerdo") == "trabalhando" {
            assert_eq!(estado(&v, "vigia"), "ocioso", "só o bot que pensa trabalha");
            visto = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert!(visto, "o bot lerdo nunca apareceu como trabalhando");

    em_andamento.await.unwrap();
    let (_, v) = pegar(&app, "/api/v1/bots").await;
    assert_eq!(
        estado(&v, "lerdo"),
        "ocioso",
        "terminou (com erro): volta a ocioso"
    );
}

#[tokio::test]
async fn lista_provedores_e_modelos() {
    let (_d, app) = roteador().await;
    let (st, v) = pegar(&app, "/api/v1/provedores").await;
    assert_eq!(st, StatusCode::OK);
    let lista = v["provedores"].as_array().unwrap();
    let achar = |n: &str| lista.iter().find(|p| p["nome"] == n).unwrap().clone();
    assert_eq!(achar("falso")["chave_pronta"], true);
    assert_eq!(achar("sem")["chave_pronta"], false);
    assert!(achar("sem")["problema_da_chave"]
        .as_str()
        .unwrap()
        .contains("SEM_KEY"));
    assert_eq!(achar("anthropic")["lingua"], "anthropic_messages");

    let (st, v) = pegar(&app, "/api/v1/provedores/falso/modelos").await;
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(v["modelos"], json!(["m-1", "m-2"]));
    let (st, v) = pegar(&app, "/api/v1/provedores/nao-ha/modelos").await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    assert_eq!(v["type"], "https://zylos.dev/errors/provedor-inexistente");
    let (st, _) = pegar(&app, "/api/v1/provedores/sem/modelos").await;
    assert_eq!(st, StatusCode::FAILED_DEPENDENCY);
}

#[tokio::test]
async fn responde_200_com_personalidade() {
    let (_d, app) = roteador().await;
    let (st, _, v) = perguntar(&app, "vigia", r#"{"pergunta":"oi"}"#).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(v["resposta"], "[Sou o vigia] ok");
    assert_eq!(v["bot"], "vigia");
    assert_eq!(v["provedor"], "falso");
    assert_eq!(v["modelo"], "m");
    assert!(v["duracao_ms"].is_u64());
}

#[tokio::test]
async fn cada_erro_tem_seu_status_em_problem_json() {
    let (_d, app) = roteador().await;
    let casos = [
        (
            "ninguem",
            r#"{"pergunta":"oi"}"#,
            StatusCode::NOT_FOUND,
            "bot-inexistente",
        ),
        (
            "vigia",
            r#"{"pergunta":"  "}"#,
            StatusCode::UNPROCESSABLE_ENTITY,
            "pergunta-invalida",
        ),
        (
            "sem-modelo",
            r#"{"pergunta":"oi"}"#,
            StatusCode::UNPROCESSABLE_ENTITY,
            "bot-sem-modelo",
        ),
        (
            "sem-chave",
            r#"{"pergunta":"oi"}"#,
            StatusCode::FAILED_DEPENDENCY,
            "chave-faltando",
        ),
        (
            "recusado",
            r#"{"pergunta":"oi"}"#,
            StatusCode::BAD_GATEWAY,
            "provedor-recusou",
        ),
        (
            "lerdo",
            r#"{"pergunta":"oi"}"#,
            StatusCode::GATEWAY_TIMEOUT,
            "tempo-esgotado",
        ),
    ];
    for (nome, corpo, esperado, tipo) in casos {
        let (st, ct, v) = perguntar(&app, nome, corpo).await;
        assert_eq!(st, esperado, "{nome}: {v}");
        assert_eq!(ct, "application/problem+json", "{nome}");
        assert_eq!(
            v["type"],
            format!("https://zylos.dev/errors/{tipo}"),
            "{nome}"
        );
        assert_eq!(v["status"], esperado.as_u16(), "{nome}");
    }
    let (_, _, v) = perguntar(&app, "sem-chave", r#"{"pergunta":"oi"}"#).await;
    assert!(
        v["detail"].as_str().unwrap().contains("SEM_KEY"),
        "o detalhe cita o nome da chave: {v}"
    );
    let (_, _, v) = perguntar(&app, "recusado", r#"{"pergunta":"oi"}"#).await;
    assert!(v["detail"].as_str().unwrap().contains("bad key ***"), "{v}");
}

#[tokio::test]
async fn falha_do_provedor_avisa_o_cockpit_e_erro_de_entrada_nao() {
    use futures_util::StreamExt;
    use zylos_live_comm::{EventBus, ZylosEvent};

    let bus = EventBus::new(16, None);
    let mut rx = bus.subscribe_legacy();
    let (_d, app) = roteador_com_bus(Some(bus)).await;

    // Bot que não existe: erro do usuário, nenhum aviso.
    let (st, _, _) = perguntar(&app, "fantasma", r#"{"pergunta":"oi"}"#).await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    // Provedor recusou a chave: falha do agente, vira AgentStateChanged.
    let (st, _, _) = perguntar(&app, "recusado", r#"{"pergunta":"oi"}"#).await;
    assert_eq!(st, StatusCode::BAD_GATEWAY);

    // Primeiro `running` (a pergunta começou), depois o `error`; nunca `idle` depois de falha.
    let env = tokio::time::timeout(Duration::from_secs(2), rx.next())
        .await
        .expect("running não chegou")
        .unwrap()
        .unwrap();
    match env.payload {
        ZylosEvent::AgentStateChanged(a) => {
            assert_eq!(a.agent_id, "recusado");
            assert_eq!(a.new_state, "running");
        }
        outro => panic!("evento inesperado: {outro:?}"),
    }
    let env = tokio::time::timeout(Duration::from_secs(2), rx.next())
        .await
        .expect("aviso não chegou")
        .unwrap()
        .unwrap();
    match env.payload {
        ZylosEvent::AgentStateChanged(a) => {
            assert_eq!(a.agent_id, "recusado");
            assert_eq!(a.new_state, "error");
            let motivo = a.reason.unwrap();
            assert_eq!(motivo, "O provedor recusou");
            assert!(!motivo.contains(CHAVE), "a chave vazou no aviso");
        }
        outro => panic!("evento inesperado: {outro:?}"),
    }
    // Nada mais: o bot inexistente não publicou nada e a falha não vira `idle`.
    assert!(tokio::time::timeout(Duration::from_millis(300), rx.next())
        .await
        .is_err());
}

#[tokio::test]
async fn pergunta_respondida_publica_running_e_depois_idle() {
    use futures_util::StreamExt;
    use zylos_live_comm::{EventBus, ZylosEvent};

    let bus = EventBus::new(16, None);
    let mut rx = bus.subscribe_legacy();
    let (_d, app) = roteador_com_bus(Some(bus)).await;

    let (st, _, _) = perguntar(&app, "vigia", r#"{"pergunta":"oi"}"#).await;
    assert_eq!(st, StatusCode::OK);

    let mut estados = Vec::new();
    for _ in 0..2 {
        let env = tokio::time::timeout(Duration::from_secs(2), rx.next())
            .await
            .expect("evento não chegou")
            .unwrap()
            .unwrap();
        match env.payload {
            ZylosEvent::AgentStateChanged(a) => {
                assert_eq!(a.agent_id, "vigia");
                estados.push((a.previous_state, a.new_state));
            }
            outro => panic!("evento inesperado: {outro:?}"),
        }
    }
    assert_eq!(
        estados,
        [
            ("idle".to_string(), "running".to_string()),
            ("running".to_string(), "idle".to_string())
        ]
    );
}
