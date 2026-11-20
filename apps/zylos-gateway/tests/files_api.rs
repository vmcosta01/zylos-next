//! `/api/v1/files`: guardar, listar, baixar e apagar de verdade numa pasta temporária.

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use axum::Router;
use serde_json::Value;
use tower::ServiceExt;
use zylos_gateway::files_api::{create_files_router, FilesState, MAX_FILE_BYTES};

fn app(dir: &std::path::Path) -> Router {
    Router::new().nest(
        "/api/v1",
        create_files_router(FilesState::new(dir.to_path_buf())),
    )
}

async fn chamar(
    app: &Router,
    m: Method,
    uri: &str,
    corpo: Vec<u8>,
) -> (StatusCode, Vec<u8>, header_map::H) {
    let req = Request::builder()
        .method(m)
        .uri(uri)
        .body(Body::from(corpo))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let st = resp.status();
    let h = header_map::H(resp.headers().clone());
    let bytes = axum::body::to_bytes(resp.into_body(), 32 << 20)
        .await
        .unwrap();
    (st, bytes.to_vec(), h)
}

mod header_map {
    pub struct H(pub axum::http::HeaderMap);
    impl H {
        pub fn get(&self, k: &str) -> String {
            self.0
                .get(k)
                .map(|v| v.to_str().unwrap().to_string())
                .unwrap_or_default()
        }
    }
}

fn json(b: &[u8]) -> Value {
    serde_json::from_slice(b).unwrap_or(Value::Null)
}

#[tokio::test]
async fn guarda_lista_baixa_e_apaga() {
    let d = tempfile::tempdir().unwrap();
    let app = app(d.path());

    let (st, b, _) = chamar(&app, Method::GET, "/api/v1/files", vec![]).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(json(&b)["files"].as_array().unwrap().len(), 0);

    let (st, _, _) = chamar(
        &app,
        Method::POST,
        "/api/v1/files?name=nota.txt",
        b"oi".to_vec(),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED);
    let (st, _, _) = chamar(
        &app,
        Method::POST,
        "/api/v1/files?folder=docs&name=a.json",
        b"{}".to_vec(),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED);

    let (_, b, _) = chamar(&app, Method::GET, "/api/v1/files", vec![]).await;
    let v = json(&b);
    let lista = v["files"].as_array().unwrap();
    assert_eq!(lista.len(), 2);
    let nota = lista.iter().find(|f| f["name"] == "nota.txt").unwrap();
    assert_eq!(nota["folder"], "root");
    assert_eq!(nota["size_bytes"], 2);
    assert_eq!(nota["mime_type"], "text/plain");
    let a = lista.iter().find(|f| f["name"] == "a.json").unwrap();
    assert_eq!(a["folder"], "docs");
    assert!(a["uploaded_at"].as_str().unwrap().contains('T'));

    let (st, bytes, h) = chamar(
        &app,
        Method::GET,
        "/api/v1/files/content?folder=docs&name=a.json",
        vec![],
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(bytes, b"{}");
    assert!(h.get("content-disposition").starts_with("attachment"));
    assert_eq!(h.get("x-content-type-options"), "nosniff");

    let (st, _, _) = chamar(&app, Method::DELETE, "/api/v1/files?name=nota.txt", vec![]).await;
    assert_eq!(st, StatusCode::NO_CONTENT);
    let (st, _, _) = chamar(&app, Method::DELETE, "/api/v1/files?name=nota.txt", vec![]).await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    assert!(!d.path().join("nota.txt").exists());
}

#[tokio::test]
async fn nunca_sobrescreve_e_recusa_nomes_perigosos() {
    let d = tempfile::tempdir().unwrap();
    let app = app(d.path());

    let (st, _, _) = chamar(
        &app,
        Method::POST,
        "/api/v1/files?name=a.txt",
        b"um".to_vec(),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED);
    let (st, _, _) = chamar(
        &app,
        Method::POST,
        "/api/v1/files?name=a.txt",
        b"dois".to_vec(),
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT);
    assert_eq!(std::fs::read(d.path().join("a.txt")).unwrap(), b"um");

    for uri in [
        "/api/v1/files?name=..%2Fa.txt",
        "/api/v1/files?folder=..&name=a.txt",
        "/api/v1/files?name=.env",
        "/api/v1/files?folder=a%2Fb&name=x",
        "/api/v1/files?name=a%5Cb",
    ] {
        let (st, _, _) = chamar(&app, Method::POST, uri, b"x".to_vec()).await;
        assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{uri}");
        let (st, _, _) = chamar(
            &app,
            Method::GET,
            &uri.replace("/files?", "/files/content?"),
            vec![],
        )
        .await;
        assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{uri}");
    }
    // Nada vazou para fora da pasta: só o a.txt existe.
    let fora = d.path().parent().unwrap().join("a.txt");
    assert!(!fora.exists() || std::fs::read(&fora).unwrap() != b"x");
}

#[tokio::test]
async fn html_baixa_como_anexo_opaco_e_arquivo_grande_e_recusado() {
    let d = tempfile::tempdir().unwrap();
    let app = app(d.path());

    chamar(
        &app,
        Method::POST,
        "/api/v1/files?name=p.html",
        b"<script>1</script>".to_vec(),
    )
    .await;
    let (_, _, h) = chamar(
        &app,
        Method::GET,
        "/api/v1/files/content?name=p.html",
        vec![],
    )
    .await;
    assert_eq!(h.get("content-type"), "application/octet-stream");
    assert!(h.get("content-disposition").starts_with("attachment"));

    let grande = vec![0u8; MAX_FILE_BYTES + 1];
    let (st, _, _) = chamar(&app, Method::POST, "/api/v1/files?name=g.bin", grande).await;
    assert_eq!(st, StatusCode::PAYLOAD_TOO_LARGE);
    assert!(!d.path().join("g.bin").exists());
    let _ = header::CONTENT_TYPE;
}
