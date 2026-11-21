//! `/api/v1/skills/contracts`: salvar, listar, ler e apagar de verdade numa pasta temporária.

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use axum::Router;
use serde_json::{json, Value};
use tower::ServiceExt;
use zylos_gateway::skills_api::{create_skills_router, SkillsState};

fn app(dir: &std::path::Path) -> Router {
    Router::new().nest(
        "/api/v1",
        create_skills_router(SkillsState::new(dir.to_path_buf())),
    )
}

async fn chamar(app: &Router, m: Method, uri: &str, corpo: Option<Value>) -> (StatusCode, Value) {
    let mut req = Request::builder().method(m).uri(uri);
    let body = match corpo {
        Some(v) => {
            req = req.header(header::CONTENT_TYPE, "application/json");
            Body::from(v.to_string())
        }
        None => Body::empty(),
    };
    let resp = app.clone().oneshot(req.body(body).unwrap()).await.unwrap();
    let st = resp.status();
    let b = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    (st, serde_json::from_slice(&b).unwrap_or(Value::Null))
}

/// Reservas (`<nome>.json.<data-hora>.bak`) de uma skill, da mais antiga para a mais nova.
fn reservas_de(dir: &std::path::Path, nome: &str) -> Vec<std::path::PathBuf> {
    let prefixo = format!("{nome}.json.");
    let mut v: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| {
            let f = e.file_name().to_string_lossy().to_string();
            f.starts_with(&prefixo) && f.ends_with(".bak")
        })
        .map(|e| e.path())
        .collect();
    v.sort();
    v
}

#[tokio::test]
async fn varios_saves_seguidos_nao_perdem_o_original() {
    // O editor ainda recarrega só nome/descrição/categoria: o 2º Save grava um YAML sem os
    // passos. Com uma reserva só, o 3º Save apagava o original; agora cada versão fica guardada.
    let d = tempfile::tempdir().unwrap();
    let app = app(d.path());
    let salvar = |yaml: &'static str| {
        let app = app.clone();
        async move {
            chamar(
                &app,
                Method::POST,
                "/api/v1/skills/contracts",
                Some(json!({"name":"deploy","description":"d","category":"ci","yaml":yaml})),
            )
            .await
            .0
        }
    };
    assert_eq!(salvar("ORIGINAL com passos\n").await, StatusCode::CREATED);
    assert_eq!(salvar("vazio 1\n").await, StatusCode::OK);
    assert_eq!(salvar("vazio 2\n").await, StatusCode::OK);
    let conteudos: Vec<String> = reservas_de(d.path(), "deploy")
        .iter()
        .map(|p| {
            serde_json::from_slice::<Value>(&std::fs::read(p).unwrap()).unwrap()["yaml"]
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect();
    assert_eq!(conteudos.len(), 2, "{conteudos:?}");
    assert!(
        conteudos.iter().any(|c| c.starts_with("ORIGINAL")),
        "o original não sobreviveu: {conteudos:?}"
    );
}

#[tokio::test]
async fn so_as_reservas_mais_recentes_ficam_e_nao_aparecem_na_lista() {
    let d = tempfile::tempdir().unwrap();
    let app = app(d.path());
    for i in 0..15 {
        let (st, _) = chamar(
            &app,
            Method::POST,
            "/api/v1/skills/contracts",
            Some(json!({"name":"s","yaml":format!("v{i}\n")})),
        )
        .await;
        assert!(st == StatusCode::CREATED || st == StatusCode::OK);
    }
    assert_eq!(reservas_de(d.path(), "s").len(), 10);
    let (_, v) = chamar(&app, Method::GET, "/api/v1/skills/contracts", None).await;
    assert_eq!(v["contracts"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn salva_lista_le_atualiza_e_apaga() {
    let d = tempfile::tempdir().unwrap();
    let app = app(d.path());

    let (st, v) = chamar(&app, Method::GET, "/api/v1/skills/contracts", None).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(v["contracts"].as_array().unwrap().len(), 0);

    let nova = json!({"name":"Hello World","description":"diz oi","category":"demo","yaml":"name: hello\nsteps: []\n"});
    let (st, _) = chamar(&app, Method::POST, "/api/v1/skills/contracts", Some(nova)).await;
    assert_eq!(st, StatusCode::CREATED);
    let (st, _) = chamar(
        &app,
        Method::POST,
        "/api/v1/skills/contracts",
        Some(json!({"name":"build","description":"","category":"ci","yaml":""})),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED);

    // Lista em ordem de nome, sem o YAML (é resumo).
    let (_, v) = chamar(&app, Method::GET, "/api/v1/skills/contracts", None).await;
    let lista = v["contracts"].as_array().unwrap();
    let nomes: Vec<_> = lista.iter().map(|c| c["name"].as_str().unwrap()).collect();
    assert_eq!(nomes, ["build", "Hello World"]);
    assert_eq!(lista[1]["category"], "demo");
    assert!(lista[1].get("yaml").is_none());
    assert!(lista[1]["updated_at"].as_str().unwrap().contains('T'));

    // Leitura completa, com o YAML.
    let (st, v) = chamar(
        &app,
        Method::GET,
        "/api/v1/skills/contracts/Hello%20World",
        None,
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(v["yaml"], "name: hello\nsteps: []\n");

    // Atualizar o mesmo nome troca o conteúdo (200, não cria outro).
    let (st, _) = chamar(
        &app,
        Method::POST,
        "/api/v1/skills/contracts",
        Some(json!({"name":"Hello World","description":"novo","category":"demo","yaml":"v: 2\n"})),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let (_, v) = chamar(
        &app,
        Method::GET,
        "/api/v1/skills/contracts/Hello%20World",
        None,
    )
    .await;
    assert_eq!(v["description"], "novo");
    // A versão anterior ficou de reserva e não vira skill na listagem.
    let reservas = reservas_de(d.path(), "Hello World");
    assert_eq!(reservas.len(), 1);
    let reserva: Value = serde_json::from_slice(&std::fs::read(&reservas[0]).unwrap()).unwrap();
    assert_eq!(reserva["yaml"], "name: hello\nsteps: []\n");
    assert_eq!(reserva["description"], "diz oi");
    let (_, v) = chamar(&app, Method::GET, "/api/v1/skills/contracts", None).await;
    assert_eq!(v["contracts"].as_array().unwrap().len(), 2);

    // Apagar.
    let (st, _) = chamar(&app, Method::DELETE, "/api/v1/skills/contracts/build", None).await;
    assert_eq!(st, StatusCode::NO_CONTENT);
    let (st, _) = chamar(&app, Method::DELETE, "/api/v1/skills/contracts/build", None).await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    let (st, _) = chamar(&app, Method::GET, "/api/v1/skills/contracts/build", None).await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    assert!(!d.path().join("build.json").exists());
    // Nenhum arquivo temporário sobrou.
    let sobras = std::fs::read_dir(d.path())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().starts_with('.'))
        .count();
    assert_eq!(sobras, 0);
}

#[tokio::test]
async fn recusa_nomes_perigosos_e_corpo_grande_e_nada_sai_da_pasta() {
    let d = tempfile::tempdir().unwrap();
    let app = app(d.path());
    for nome in ["../fora", ".env", "a/b", "a\\b", "", "é"] {
        let (st, _) = chamar(
            &app,
            Method::POST,
            "/api/v1/skills/contracts",
            Some(json!({"name":nome,"description":"","category":"","yaml":""})),
        )
        .await;
        assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{nome:?}");
    }
    let (st, _) = chamar(
        &app,
        Method::POST,
        "/api/v1/skills/contracts",
        Some(json!({"name":"grande","description":"","category":"","yaml":"x".repeat(70_000)})),
    )
    .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    let (st, _) = chamar(
        &app,
        Method::GET,
        "/api/v1/skills/contracts/..%2Fsecreto",
        None,
    )
    .await;
    assert!(
        matches!(st, StatusCode::NOT_FOUND | StatusCode::BAD_REQUEST),
        "{st}"
    );
    assert_eq!(
        std::fs::read_dir(d.path()).unwrap().count(),
        0,
        "nada foi gravado"
    );
    assert!(!d.path().parent().unwrap().join("fora.json").exists());
}
