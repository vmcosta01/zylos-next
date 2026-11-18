//! Troca real de código OAuth2 (oauth2 5.x + reqwest 0.12) contra um servidor
//! de token local — valida a migração F12 sem tocar provedores externos.

use axum::{routing::post, Form, Json, Router};
use std::collections::HashMap;
use zylos_integrations::db::TokenStore;
use zylos_integrations::{IntegrationSpec, OAuthFlow};

async fn token_endpoint(Form(f): Form<HashMap<String, String>>) -> Json<serde_json::Value> {
    // PKCE: o verificador precisa chegar no corpo da troca de código.
    let grant = f.get("grant_type").cloned().unwrap_or_default();
    if grant == "authorization_code" {
        assert_eq!(f.get("code").map(String::as_str), Some("codigo-ok"));
        assert!(f
            .get("code_verifier")
            .map(|v| v.len() >= 43)
            .unwrap_or(false));
        assert_eq!(
            f.get("redirect_uri").map(String::as_str),
            Some("http://127.0.0.1/callback")
        );
        Json(serde_json::json!({
            "access_token": "at-1", "token_type": "bearer",
            "expires_in": 3600, "refresh_token": "rt-1", "scope": "read write"
        }))
    } else {
        assert_eq!(grant, "refresh_token");
        assert_eq!(f.get("refresh_token").map(String::as_str), Some("rt-1"));
        Json(serde_json::json!({
            "access_token": "at-2", "token_type": "bearer", "expires_in": 3600
        }))
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn f12_oauth_code_exchange_and_refresh_against_local_server() {
    let dir = tempfile::tempdir().unwrap();
    std::env::set_var("ZYLOS_DATA_DIR", dir.path());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = Router::new().route("/token", post(token_endpoint));
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    let spec: IntegrationSpec = serde_json::from_value(serde_json::json!({
        "slug": "local", "display_name": "Local",
        "auth_url": format!("http://{addr}/authorize"),
        "token_url": format!("http://{addr}/token"),
        "client_id": "cid", "client_secret": "csecret",
        "redirect_uri": "http://127.0.0.1/callback",
        "scopes": ["read", "write"]
    }))
    .unwrap();

    let flow = OAuthFlow::new(TokenStore::new().await.unwrap());
    let session = flow
        .initiate(&spec, "http://127.0.0.1/callback")
        .await
        .unwrap();
    assert!(session
        .authorization_url
        .contains("code_challenge_method=S256"));
    assert!(session.authorization_url.contains("redirect_uri="));
    assert!(
        session.pkce_verifier.is_empty(),
        "verifier nunca sai do servidor"
    );

    let bundle = flow
        .complete(&session.session_id, "codigo-ok")
        .await
        .unwrap();
    assert_eq!(bundle.access_token, "at-1");
    assert_eq!(bundle.refresh_token.as_deref(), Some("rt-1"));
    assert_eq!(bundle.scopes, vec!["read", "write"]);
    assert!(!bundle.is_expired());

    // Sessão é de uso único.
    assert!(flow
        .complete(&session.session_id, "codigo-ok")
        .await
        .is_err());

    let refreshed = flow.refresh(&spec, &bundle).await.unwrap();
    assert_eq!(refreshed.access_token, "at-2");
    // Sem refresh_token novo, mantém o anterior.
    assert_eq!(refreshed.refresh_token.as_deref(), Some("rt-1"));
}
