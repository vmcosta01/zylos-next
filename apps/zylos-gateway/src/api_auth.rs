//! Autenticação das rotas `/api/v1/*` (falhas altas 11.11.8 #1 e #2).
//!
//! Política:
//! - **Segredo configurado** (`ZYLOS_AUTH_SECRET` ≥ 32 bytes): toda rota sob
//!   `/api/v1` exige `Authorization: Bearer <jwt>` válido, exceto as rotas
//!   públicas ([`rota_publica`]: health e `/auth/login|refresh`). Em
//!   WebSocket/EventSource (`/ws`, `/events`, `/crdt/sync/:doc`) o navegador
//!   não manda cabeçalho, então o token também é aceito como `?access_token=`.
//! - **Segredo ausente** (modo local, já protegido por `local_guard`): as rotas
//!   comuns continuam abertas, mas as rotas de decisão humana ([`rota_admin`])
//!   ficam fechadas (503) — ninguém aprova sem identidade.
//! - Rotas de decisão sempre exigem papel `admin`. O campo de autoria do corpo
//!   (`resolved_by` / `approver`) é sobrescrito com o `sub` do token, para que
//!   ninguém registre a aprovação em nome de outra pessoa.

use axum::{
    body::Body,
    extract::{Request, State},
    http::{Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use zylos_auth::middleware::AuthContext;
use zylos_auth::{Claims, TokenService};

/// Limite do corpo lido nas rotas de decisão (são JSON pequenos).
const MAX_DECISION_BODY: usize = 64 * 1024;

/// Estado do middleware.
#[derive(Clone)]
pub struct ApiAuth {
    svc: Option<TokenService>,
}

impl ApiAuth {
    pub fn new(svc: Option<TokenService>) -> Self {
        Self { svc }
    }

    /// `true` quando há segredo e a autenticação é obrigatória.
    pub fn enforced(&self) -> bool {
        self.svc.is_some()
    }
}

fn negar(status: StatusCode, msg: &str) -> Response {
    (status, Json(serde_json::json!({ "error": msg }))).into_response()
}

/// Rotas sem token (caminho completo).
pub fn rota_publica(path: &str) -> bool {
    matches!(
        path,
        "/api/v1/health" | "/api/v1/cockpit/health" | "/api/v1/auth/login" | "/api/v1/auth/refresh"
    )
}

/// Rotas que aceitam `?access_token=` (navegador não envia cabeçalho).
fn aceita_token_na_query(path: &str) -> bool {
    path == "/api/v1/ws"
        || path == "/api/v1/events"
        || path.starts_with("/api/v1/crdt/sync/")
        || path.starts_with("/api/v1/ws/computer-view/")
}

/// Rotas de decisão humana: exigem papel admin. Retorna o campo de autoria do corpo.
pub fn rota_admin(method: &Method, path: &str) -> Option<&'static str> {
    if method != Method::POST {
        return None;
    }
    if path == "/api/v1/approval/resolve" || path == "/api/v1/approvals/resolve" {
        return Some("resolved_by");
    }
    if path == "/api/v1/approvals/rules/revoke" {
        return Some("revoked_by");
    }
    if let Some(resto) = path.strip_prefix("/api/v1/sentinel/remediations/") {
        let mut partes = resto.split('/');
        let id = partes.next().unwrap_or("");
        let acao = partes.next().unwrap_or("");
        if !id.is_empty() && partes.next().is_none() && (acao == "approve" || acao == "reject") {
            return Some("approver");
        }
    }
    None
}

fn token_do_pedido(req: &Request) -> Option<String> {
    if let Some(h) = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
    {
        return match h.split_once(' ') {
            Some((esquema, t))
                if esquema.eq_ignore_ascii_case("bearer") && !t.trim().is_empty() =>
            {
                Some(t.trim().to_string())
            }
            _ => None,
        };
    }
    if aceita_token_na_query(req.uri().path()) {
        let q = req.uri().query()?;
        for par in q.split('&') {
            if let Some(v) = par.strip_prefix("access_token=") {
                if !v.is_empty() {
                    // JWT é base64url + '.', não precisa de decodificação.
                    return Some(v.to_string());
                }
            }
        }
    }
    None
}

fn eh_admin(c: &Claims) -> bool {
    c.roles.iter().any(|r| r.eq_ignore_ascii_case("admin"))
}

/// Middleware aplicado no roteador completo (vê o caminho com `/api/v1`).
pub async fn exigir_auth(State(st): State<ApiAuth>, mut req: Request, next: Next) -> Response {
    let path = req.uri().path().to_string();
    if !path.starts_with("/api/v1/") || req.method() == Method::OPTIONS || rota_publica(&path) {
        return next.run(req).await;
    }
    let campo_autoria = rota_admin(req.method(), &path);

    let Some(svc) = st.svc.as_ref() else {
        if campo_autoria.is_some() {
            return negar(
                StatusCode::SERVICE_UNAVAILABLE,
                "aprovação exige login de administrador; configure ZYLOS_AUTH_SECRET",
            );
        }
        return next.run(req).await; // modo local sem segredo
    };

    let Some(token) = token_do_pedido(&req) else {
        return negar(StatusCode::UNAUTHORIZED, "token ausente");
    };
    let claims = match svc.validate_token(&token) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, path = %path, "JWT recusado");
            return negar(StatusCode::UNAUTHORIZED, "token inválido");
        }
    };

    if let Some(campo) = campo_autoria {
        if !eh_admin(&claims) {
            return negar(StatusCode::FORBIDDEN, "aprovação exige papel admin");
        }
        // Sobrescreve a autoria do corpo com o sub do token.
        let (partes, corpo) = req.into_parts();
        let bytes = match axum::body::to_bytes(corpo, MAX_DECISION_BODY).await {
            Ok(b) => b,
            Err(_) => return negar(StatusCode::PAYLOAD_TOO_LARGE, "corpo grande demais"),
        };
        let novo = match serde_json::from_slice::<serde_json::Value>(&bytes) {
            Ok(serde_json::Value::Object(mut m)) => {
                m.insert(campo.into(), serde_json::Value::String(claims.sub.clone()));
                serde_json::to_vec(&m).unwrap_or_else(|_| bytes.to_vec())
            }
            _ => return negar(StatusCode::BAD_REQUEST, "corpo JSON inválido"),
        };
        let mut partes = partes;
        partes.headers.remove(axum::http::header::CONTENT_LENGTH);
        req = Request::from_parts(partes, Body::from(novo));
    }

    req.extensions_mut().insert(AuthContext { claims });
    next.run(req).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{routing::post, Router};
    use tower::ServiceExt;
    use zylos_auth::tokens::CreateTokenRequest;
    use zylos_auth::TokenConfig;

    const SECRET: &str = "api-auth-test-secret-0123456789abcdef";

    fn svc() -> TokenService {
        TokenService::new(TokenConfig {
            secret: SECRET.into(),
            ..TokenConfig::default()
        })
        .unwrap()
    }

    fn token(s: &TokenService, roles: &[&str]) -> String {
        s.create_token(&CreateTokenRequest {
            user_id: "alice".into(),
            tenant_id: "t1".into(),
            roles: roles.iter().map(|r| r.to_string()).collect(),
            name: None,
        })
        .unwrap()
        .access_token
    }

    fn app(st: ApiAuth) -> Router {
        async fn eco(body: String) -> String {
            body
        }
        Router::new()
            .route("/api/v1/health", axum::routing::get(|| async { "ok" }))
            .route("/api/v1/agents", axum::routing::get(|| async { "agents" }))
            .route("/api/v1/ws", axum::routing::get(|| async { "ws" }))
            .route("/api/v1/approval/resolve", post(eco))
            .route("/api/v1/sentinel/remediations/:id/approve", post(eco))
            .layer(axum::middleware::from_fn_with_state(st, exigir_auth))
    }

    async fn call(app: Router, req: Request<Body>) -> (StatusCode, String) {
        let r = app.oneshot(req).await.unwrap();
        let s = r.status();
        let b = axum::body::to_bytes(r.into_body(), 1 << 20).await.unwrap();
        (s, String::from_utf8_lossy(&b).into_owned())
    }

    fn get(uri: &str, tok: Option<&str>) -> Request<Body> {
        let mut b = Request::get(uri);
        if let Some(t) = tok {
            b = b.header("authorization", format!("Bearer {t}"));
        }
        b.body(Body::empty()).unwrap()
    }

    fn post_json(uri: &str, tok: Option<&str>, body: &str) -> Request<Body> {
        let mut b = Request::post(uri).header("content-type", "application/json");
        if let Some(t) = tok {
            b = b.header("authorization", format!("Bearer {t}"));
        }
        b.body(Body::from(body.to_string())).unwrap()
    }

    #[tokio::test]
    async fn sem_token_recusa_e_health_passa() {
        let a = app(ApiAuth::new(Some(svc())));
        assert_eq!(
            call(a.clone(), get("/api/v1/agents", None)).await.0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            call(a.clone(), get("/api/v1/agents", Some("lixo"))).await.0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(call(a, get("/api/v1/health", None)).await.0, StatusCode::OK);
    }

    #[tokio::test]
    async fn token_valido_passa() {
        let s = svc();
        let t = token(&s, &["viewer"]);
        let a = app(ApiAuth::new(Some(s)));
        assert_eq!(
            call(a, get("/api/v1/agents", Some(&t))).await.0,
            StatusCode::OK
        );
    }

    #[tokio::test]
    async fn token_revogado_recusa() {
        let s = svc();
        let t = token(&s, &["admin"]);
        s.revoke_token(&t).unwrap();
        let a = app(ApiAuth::new(Some(s)));
        assert_eq!(
            call(a, get("/api/v1/agents", Some(&t))).await.0,
            StatusCode::UNAUTHORIZED
        );
    }

    #[tokio::test]
    async fn query_token_so_em_ws() {
        let s = svc();
        let t = token(&s, &["viewer"]);
        let a = app(ApiAuth::new(Some(s)));
        let ws = format!("/api/v1/ws?access_token={t}");
        assert_eq!(call(a.clone(), get(&ws, None)).await.0, StatusCode::OK);
        let ag = format!("/api/v1/agents?access_token={t}");
        assert_eq!(call(a, get(&ag, None)).await.0, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn aprovacao_exige_admin_e_sobrescreve_autoria() {
        let s = svc();
        let viewer = token(&s, &["viewer"]);
        let admin = token(&s, &["admin"]);
        let a = app(ApiAuth::new(Some(s)));
        let body = r#"{"approval_id":"x","decision":"approved","resolved_by":"mallory"}"#;
        assert_eq!(
            call(
                a.clone(),
                post_json("/api/v1/approval/resolve", Some(&viewer), body)
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
        let (st, eco) = call(
            a.clone(),
            post_json("/api/v1/approval/resolve", Some(&admin), body),
        )
        .await;
        assert_eq!(st, StatusCode::OK);
        let v: serde_json::Value = serde_json::from_str(&eco).unwrap();
        assert_eq!(v["resolved_by"], "alice");
        let (st, eco) = call(
            a,
            post_json(
                "/api/v1/sentinel/remediations/r1/approve",
                Some(&admin),
                r#"{"approver":"mallory"}"#,
            ),
        )
        .await;
        assert_eq!(st, StatusCode::OK);
        assert!(eco.contains("\"approver\":\"alice\""));
    }

    #[tokio::test]
    async fn sem_segredo_rotas_comuns_abertas_e_aprovacao_fechada() {
        let a = app(ApiAuth::new(None));
        assert_eq!(
            call(a.clone(), get("/api/v1/agents", None)).await.0,
            StatusCode::OK
        );
        assert_eq!(
            call(a, post_json("/api/v1/approval/resolve", None, "{}"))
                .await
                .0,
            StatusCode::SERVICE_UNAVAILABLE
        );
    }

    #[test]
    fn classifica_rotas_admin() {
        assert_eq!(
            rota_admin(&Method::POST, "/api/v1/approval/resolve"),
            Some("resolved_by")
        );
        assert_eq!(
            rota_admin(&Method::POST, "/api/v1/sentinel/remediations/a/reject"),
            Some("approver")
        );
        // A rota que o cockpit usa para decidir também exige admin (sem isso, em modo local
        // ficaria aberta e, com segredo, aceitaria qualquer papel).
        assert_eq!(
            rota_admin(&Method::POST, "/api/v1/approvals/resolve"),
            Some("resolved_by")
        );
        assert_eq!(
            rota_admin(&Method::POST, "/api/v1/approvals/rules/revoke"),
            Some("revoked_by")
        );
        assert_eq!(rota_admin(&Method::GET, "/api/v1/approval/resolve"), None);
        // O stream do computador do agente (WebSocket) também autentica por ?access_token=.
        assert!(aceita_token_na_query("/api/v1/ws/computer-view/abc"));
        assert!(!aceita_token_na_query("/api/v1/computer-view/sessions"));
        // O screenshot NÃO aceita token na URL (iria para log e histórico): o cockpit baixa com Authorization.
        assert!(!aceita_token_na_query(
            "/api/v1/computer-view/sessions/abc/screenshot"
        ));
        assert_eq!(
            rota_admin(&Method::POST, "/api/v1/sentinel/remediations/a/b/approve"),
            None
        );
    }
}
