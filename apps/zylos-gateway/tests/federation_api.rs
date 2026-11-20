//! `GET /api/v1/federation` contra nós falsos de verdade (servidores HTTP locais).

use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::get;
use axum::{Json, Router};
use serde_json::{json, Value};
use tower::ServiceExt;
use zylos_gateway::federation_api::{create_federation_router, parse_peers, FederationState};

async fn subir(rota: Router) -> String {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://127.0.0.1:{}", l.local_addr().unwrap().port());
    tokio::spawn(async move { axum::serve(l, rota).await.unwrap() });
    url
}

async fn consultar(app: &Router) -> Value {
    let resp = app
        .clone()
        .oneshot(
            Request::get("/api/v1/federation")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let b = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    serde_json::from_slice(&b).unwrap()
}

#[tokio::test]
async fn sem_nos_configurados_devolve_lista_vazia() {
    let app = Router::new().nest(
        "/api/v1",
        create_federation_router(FederationState::new(vec![])),
    );
    let v = consultar(&app).await;
    assert_eq!(v["peers"].as_array().unwrap().len(), 0);
    assert!(v["self"]["version"].is_string());
    assert!(v["checked_at"].as_str().unwrap().contains('T'));
}

#[tokio::test]
async fn cada_no_e_sondado_de_verdade() {
    let ok = subir(Router::new().route(
        "/api/v1/health",
        get(|| async { Json(json!({"status": "healthy", "version": "9.9.9"})) }),
    ))
    .await;
    let meio = subir(Router::new().route(
        "/api/v1/health",
        get(|| async { Json(json!({"status": "starting"})) }),
    ))
    .await;
    let quebrado = subir(Router::new().route(
        "/api/v1/health",
        get(|| async {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                "segredo interno: senha=123",
            )
        }),
    ))
    .await;
    let lento = subir(Router::new().route(
        "/api/v1/health",
        get(|| async {
            tokio::time::sleep(Duration::from_secs(4)).await;
            Json(json!({"status": "healthy"}))
        }),
    ))
    .await;
    // Porta que ninguém escuta: bind e solta.
    let fechada = {
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        format!("http://127.0.0.1:{}", l.local_addr().unwrap().port())
    };

    let peers = parse_peers(&format!(
        "ok={ok},meio={meio},quebrado={quebrado},lento={lento},fechada={fechada}"
    ));
    assert_eq!(peers.len(), 5);
    let app = Router::new().nest(
        "/api/v1",
        create_federation_router(FederationState::new(peers)),
    );
    let v = consultar(&app).await;
    let achar = |id: &str| {
        v["peers"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"] == id)
            .unwrap()
            .clone()
    };

    let p = achar("ok");
    assert_eq!(p["status"], "online");
    assert_eq!(p["version"], "9.9.9");
    assert!(p["latency_ms"].is_u64());
    assert_eq!(p["error"], Value::Null);

    assert_eq!(achar("meio")["status"], "degraded");

    let q = achar("quebrado");
    assert_eq!(q["status"], "offline");
    assert_eq!(q["error"], "HTTP 503");
    assert!(
        !v.to_string().contains("senha"),
        "o corpo do erro do nó não pode vazar"
    );

    let l = achar("lento");
    assert_eq!(l["status"], "offline");
    assert_eq!(l["error"], "tempo esgotado");

    let f = achar("fechada");
    assert_eq!(f["status"], "offline");
    // Porta fechada: no Linux a conexão é recusada na hora; no Windows ela pode ficar tentando
    // até o limite de 2 s. Nos dois casos o nó está offline, sem latência, e o erro é uma categoria.
    assert!(
        matches!(f["error"].as_str(), Some("sem conexão" | "tempo esgotado")),
        "{f}"
    );
    assert_eq!(f["latency_ms"], Value::Null);
}

#[tokio::test]
async fn resultado_fica_em_cache_por_alguns_segundos() {
    let contador = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let c2 = contador.clone();
    let no = subir(Router::new().route(
        "/api/v1/health",
        get(move || {
            let c = c2.clone();
            async move {
                c.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Json(json!({"status": "healthy"}))
            }
        }),
    ))
    .await;
    let app = Router::new().nest(
        "/api/v1",
        create_federation_router(FederationState::new(parse_peers(&no))),
    );
    consultar(&app).await;
    consultar(&app).await;
    assert_eq!(
        contador.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "a segunda consulta veio do cache"
    );
}
