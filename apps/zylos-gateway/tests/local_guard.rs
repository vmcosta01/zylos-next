//! Trava "só desta máquina" no roteador completo (`build_router`).
//!
//! Cobre: POST e GET vindos de outro site, `Origin: null`, DNS rebinding pelo
//! `Host`, preflight de CORS de dentro e de fora, e que os clientes legítimos
//! (tela local, linha de comando, Hermes) continuam passando pela trava.

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower::ServiceExt;

use zylos_gateway::AppState;
use zylos_live_comm::{skills_api::SkillsApiState, EventBus};
use zylos_skills::{EvalFlywheel, SkillRegistry, SkillValidator};
use zylos_soar::{
    audit::AuditTrail,
    executor::ExecutorConfig,
    orchestrator::{OrchestratorState, SoarOrchestrator},
    registry::RunbookRegistry,
    router::EventRouter,
};
use zylos_swarm_orchestrator::SwarmOrchestrator;

const FORA: &str = "https://site-malicioso.example";
const TELA: &str = "http://localhost:3001";

async fn app() -> axum::Router {
    let event_bus = EventBus::new(64, None);
    let soar_state = OrchestratorState::new(
        EventRouter::new(),
        RunbookRegistry::new(),
        AuditTrail::new(b"teste-trava-local"),
        ExecutorConfig::default(),
    )
    .with_event_bus(event_bus.clone());
    let state = AppState {
        event_bus,
        skills_state: SkillsApiState {
            registry: Arc::new(RwLock::new(SkillRegistry::empty(std::path::Path::new(".")))),
            validator: Arc::new(SkillValidator::default_schema().unwrap()),
            eval_flywheel: Arc::new(RwLock::new(EvalFlywheel::empty())),
        },
        soar_orchestrator: Arc::new(SoarOrchestrator::new(Arc::new(RwLock::new(soar_state)))),
        swarm: Arc::new(SwarmOrchestrator::open(":memory:").await.expect("swarm em memória")),
        ready: Arc::new(AtomicBool::new(true)),
    };
    zylos_gateway::build_router(state)
}

fn pedido(metodo: Method, uri: &str, cabecalhos: &[(header::HeaderName, &str)], corpo: &str) -> Request<Body> {
    let mut b = Request::builder().method(metodo).uri(uri);
    for (k, v) in cabecalhos {
        b = b.header(k, *v);
    }
    if !corpo.is_empty() {
        b = b.header(header::CONTENT_TYPE, "application/json");
    }
    b.body(Body::from(corpo.to_string())).unwrap()
}

async fn status(req: Request<Body>) -> (StatusCode, axum::http::HeaderMap) {
    let resp = app().await.oneshot(req).await.unwrap();
    (resp.status(), resp.headers().clone())
}

// ---------------------------------------------------------------- recusas

#[tokio::test]
async fn post_orchestrate_de_outro_site_e_recusado() {
    let (st, h) = status(pedido(
        Method::POST,
        "/api/v1/orchestrate",
        &[(header::ORIGIN, FORA), (header::HOST, "127.0.0.1:3000")],
        r#"{"nodes":[]}"#,
    ))
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    assert_eq!(h.get(header::CONTENT_TYPE).unwrap(), "application/problem+json");
}

#[tokio::test]
async fn post_soar_de_outro_site_e_recusado() {
    let (st, _) = status(pedido(
        Method::POST,
        "/api/v1/soar/events",
        &[(header::ORIGIN, FORA), (header::HOST, "127.0.0.1:3000")],
        r#"{"source":"x"}"#,
    ))
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn get_de_outro_site_com_origin_e_recusado() {
    let (st, h) = status(pedido(
        Method::GET,
        "/api/v1/soar/playbooks",
        &[(header::ORIGIN, FORA), (header::HOST, "127.0.0.1:3000")],
        "",
    ))
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    assert!(h.get(header::ACCESS_CONTROL_ALLOW_ORIGIN).is_none());
}

#[tokio::test]
async fn origin_null_e_recusado() {
    let (st, _) = status(pedido(Method::GET, "/gateway/health", &[(header::ORIGIN, "null")], "")).await;
    assert_eq!(st, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn dns_rebinding_pelo_host_e_recusado() {
    // sem Origin (GET do mesmo "site" depois do rebinding), mas o Host denuncia o domínio de fora
    let (st, _) = status(pedido(
        Method::GET,
        "/api/v1/soar/playbooks",
        &[(header::HOST, "site-malicioso.example:3000")],
        "",
    ))
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn preflight_de_outro_site_nao_recebe_cors() {
    let (st, h) = status(pedido(
        Method::OPTIONS,
        "/api/v1/orchestrate",
        &[
            (header::ORIGIN, FORA),
            (header::ACCESS_CONTROL_REQUEST_METHOD, "POST"),
            (header::HOST, "127.0.0.1:3000"),
        ],
        "",
    ))
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    assert!(h.get(header::ACCESS_CONTROL_ALLOW_ORIGIN).is_none());
}

// ---------------------------------------------------------------- o que tem de continuar passando

#[tokio::test]
async fn cliente_sem_navegador_passa() {
    // zylos-cli e plugin do Hermes: sem Origin, Host local
    let (st, _) = status(pedido(Method::GET, "/gateway/health", &[(header::HOST, "127.0.0.1:3000")], "")).await;
    assert_eq!(st, StatusCode::OK);
}

#[tokio::test]
async fn tela_do_cockpit_passa() {
    // pelo proxy do trunk: Origin e Host da própria máquina
    let (st, _) = status(pedido(
        Method::GET,
        "/api/v1/health",
        &[(header::ORIGIN, TELA), (header::HOST, "localhost:3001")],
        "",
    ))
    .await;
    assert_eq!(st, StatusCode::OK);
}

#[tokio::test]
async fn post_local_chega_ao_handler() {
    // a trava não pode barrar a máquina: o handler responde (200 ou 422), nunca 403
    let (st, _) = status(pedido(
        Method::POST,
        "/api/v1/orchestrate",
        &[(header::ORIGIN, TELA), (header::HOST, "127.0.0.1:3000")],
        r#"{"nodes":[]}"#,
    ))
    .await;
    assert_ne!(st, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn preflight_local_recebe_cors_da_propria_origem() {
    let (_, h) = status(pedido(
        Method::OPTIONS,
        "/api/v1/orchestrate",
        &[
            (header::ORIGIN, TELA),
            (header::ACCESS_CONTROL_REQUEST_METHOD, "POST"),
            (header::HOST, "localhost:3001"),
        ],
        "",
    ))
    .await;
    assert_eq!(h.get(header::ACCESS_CONTROL_ALLOW_ORIGIN).map(|v| v.to_str().unwrap()), Some(TELA));
}

#[tokio::test]
async fn hermes_continua_exigindo_token() {
    // a trava não substitui a senha das rotas do Hermes: sem Bearer, não é 200
    let (st, _) = status(pedido(
        Method::POST,
        "/api/v1/hermes/events",
        &[(header::HOST, "127.0.0.1:3000")],
        r#"{"source":"x","event_type":"text.start","payload":{}}"#,
    ))
    .await;
    assert!(st == StatusCode::UNAUTHORIZED || st == StatusCode::SERVICE_UNAVAILABLE, "veio {st}");
}
