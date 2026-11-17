//! Cabeçalhos de segurança HTTP (F11.11.7 / F11.11.8).
//!
//! Aplicados a **todas** as respostas do gateway (API JSON, SSE, WS upgrade).
//! A CSP é restritiva porque o gateway só serve JSON/eventos — nenhuma página
//! HTML; o cockpit (Cloudflare Pages) tem CSP própria em `_headers`/`index.html`.

use axum::{
    extract::Request,
    http::{header, HeaderValue},
    middleware::Next,
    response::Response,
};

/// CSP para respostas de API: nada pode ser carregado/embutido.
pub const API_CSP: &str = "default-src 'none'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'";

/// Middleware que adiciona cabeçalhos de segurança sem sobrescrever os
/// definidos explicitamente pelo handler.
pub async fn security_headers(req: Request, next: Next) -> Response {
    let mut resp = next.run(req).await;
    let h = resp.headers_mut();
    let set = |h: &mut axum::http::HeaderMap, k: header::HeaderName, v: &'static str| {
        if !h.contains_key(&k) {
            h.insert(k, HeaderValue::from_static(v));
        }
    };
    set(h, header::X_CONTENT_TYPE_OPTIONS, "nosniff");
    set(h, header::X_FRAME_OPTIONS, "DENY");
    set(h, header::REFERRER_POLICY, "no-referrer");
    set(h, header::CONTENT_SECURITY_POLICY, API_CSP);
    set(
        h,
        header::HeaderName::from_static("cross-origin-resource-policy"),
        "same-site",
    );
    set(
        h,
        header::HeaderName::from_static("permissions-policy"),
        "camera=(), microphone=(), geolocation=(), payment=()",
    );
    // API não deve ser cacheada por proxies/navegador (tokens, dados de tenant).
    set(h, header::CACHE_CONTROL, "no-store");
    resp
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, routing::get, Router};
    use tower::ServiceExt;

    #[tokio::test]
    async fn adds_headers_and_respects_handler_values() {
        let app = Router::new()
            .route("/x", get(|| async { "ok" }))
            .route(
                "/cached",
                get(|| async { ([(header::CACHE_CONTROL, "max-age=60")], "c") }),
            )
            .layer(axum::middleware::from_fn(security_headers));
        let r = app
            .clone()
            .oneshot(Request::get("/x").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let h = r.headers();
        assert_eq!(h["x-content-type-options"], "nosniff");
        assert_eq!(h["x-frame-options"], "DENY");
        assert_eq!(h["referrer-policy"], "no-referrer");
        assert_eq!(h["content-security-policy"], API_CSP);
        assert_eq!(h["cache-control"], "no-store");
        assert!(h.contains_key("permissions-policy"));
        let r = app
            .oneshot(Request::get("/cached").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(r.headers()["cache-control"], "max-age=60");
    }
}
