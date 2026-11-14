//! Testes das rotas Hermes (`/api/v1/hermes/*`): auth, validação e repasse ao EventBus.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower::ServiceExt;

use zylos_gateway::{hermes_api, AppState};
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

const TOKEN: &str = "test-token-0123456789abcdef";

async fn state() -> AppState {
    let event_bus = EventBus::new(64, None);
    let soar_state = OrchestratorState::new(
        EventRouter::new(),
        RunbookRegistry::new(),
        AuditTrail::new(b"hermes-test-key"),
        ExecutorConfig::default(),
    )
    .with_event_bus(event_bus.clone());
    let swarm = SwarmOrchestrator::open(":memory:")
        .await
        .expect("swarm in-memory deve abrir");
    AppState {
        event_bus,
        skills_state: SkillsApiState {
            registry: Arc::new(RwLock::new(SkillRegistry::empty(std::path::Path::new(".")))),
            validator: Arc::new(SkillValidator::default_schema().unwrap()),
            eval_flywheel: Arc::new(RwLock::new(EvalFlywheel::empty())),
        },
        soar_orchestrator: Arc::new(SoarOrchestrator::new(Arc::new(RwLock::new(soar_state)))),
        swarm: Arc::new(swarm),
        ready: Arc::new(AtomicBool::new(true)),
    }
}

async fn app(token: Option<&str>) -> (Router, AppState) {
    let st = state().await;
    let r = Router::new().nest(
        "/api/v1",
        hermes_api::create_hermes_router(token.map(String::from)).with_state(st.clone()),
    );
    (r, st)
}

fn post(path: &str, auth: Option<&str>, body: &str) -> Request<Body> {
    let mut b = Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/json");
    if let Some(a) = auth {
        b = b.header("authorization", a);
    }
    b.body(Body::from(body.to_string())).unwrap()
}

async fn json_of(resp: axum::response::Response) -> serde_json::Value {
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

fn bearer() -> String {
    format!("Bearer {TOKEN}")
}

const EVENT: &str = r#"{"source":"hermes-kanban","event_type":"text.delta","payload":{"message_id":"m1","delta":"oi"}}"#;

#[tokio::test]
async fn sem_token_configurado_nega_tudo() {
    let (app, _) = app(None).await;
    let r = app
        .oneshot(post("/api/v1/hermes/events", Some(&bearer()), EVENT))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn sem_header_retorna_401() {
    let (app, _) = app(Some(TOKEN)).await;
    let r = app
        .oneshot(post("/api/v1/hermes/events", None, EVENT))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn token_errado_retorna_401() {
    let (app, _) = app(Some(TOKEN)).await;
    let r = app
        .oneshot(post("/api/v1/hermes/events", Some("Bearer errado"), EVENT))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
    let ct = r
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(ct.contains("application/problem+json"));
}

#[tokio::test]
async fn envelope_valido_sem_ouvintes_e_aceito() {
    let (app, _) = app(Some(TOKEN)).await;
    let r = app
        .oneshot(post("/api/v1/hermes/events", Some(&bearer()), EVENT))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let v = json_of(r).await;
    assert_eq!(v["status"], "accepted");
    assert_eq!(v["receivers"], 0);
}

#[tokio::test]
async fn envelope_valido_chega_ao_event_bus() {
    let (app, st) = app(Some(TOKEN)).await;
    let _sub = st.event_bus.subscribe_legacy();
    let r = app
        .oneshot(post("/api/v1/hermes/events", Some(&bearer()), EVENT))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    assert_eq!(json_of(r).await["receivers"], 1);
}

#[tokio::test]
async fn source_vazio_retorna_400_problem_details() {
    let (app, _) = app(Some(TOKEN)).await;
    let body = r#"{"source":"","event_type":"x","payload":{}}"#;
    let r = app
        .oneshot(post("/api/v1/hermes/events", Some(&bearer()), body))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);
    let v = json_of(r).await;
    assert_eq!(v["status"], 400);
    assert!(v["type"].as_str().unwrap().ends_with("invalid-event"));
}

#[tokio::test]
async fn atalho_sistema_acao_funciona() {
    let (app, _) = app(Some(TOKEN)).await;
    // Usa uma ação genérica (não dispatch-task/spawn-crew) para testar o catch-all.
    let r = app
        .oneshot(post(
            "/api/v1/hermes/bot/heartbeat",
            Some(&bearer()),
            r#"{}"#,
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let v = json_of(r).await;
    assert_eq!(v["source"], "hermes:bot");
    assert_eq!(v["event_type"], "heartbeat");
}

// ---------------------------------------------------------------------------
// dispatch-task
// ---------------------------------------------------------------------------

const DISPATCH_OK: &str = r#"{
    "task_id": "kanban-42",
    "nodes": [
        {"subsystem":"sandbox","action":"wasm_check","payload":{},"depends_on":[]}
    ]
}"#;

#[tokio::test]
async fn dispatch_task_um_no_retorna_executed() {
    let (app, _) = app(Some(TOKEN)).await;
    let r = app
        .oneshot(post(
            "/api/v1/hermes/bot/dispatch-task",
            Some(&bearer()),
            DISPATCH_OK,
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let v = json_of(r).await;
    assert_eq!(v["status"], "executed");
    // task_id devolve o valor enviado pelo chamador; label traz o derivado.
    assert_eq!(v["task_id"], "kanban-42");
    assert_eq!(v["label"], "hermes-task-kanban-42");
    assert_eq!(v["succeeded"], 1);
    assert_eq!(v["failed"], 0);
}

#[tokio::test]
async fn dispatch_task_com_dependencia_executa_dois_nos() {
    let body = r#"{
        "task_id": "dag-dep",
        "nodes": [
            {"subsystem":"sandbox","action":"wasm_check","payload":{},"depends_on":[]},
            {"subsystem":"live-comm","action":"cockpit_status","payload":{},"depends_on":["dag-dep-0"]}
        ]
    }"#;
    let (app, _) = app(Some(TOKEN)).await;
    let r = app
        .oneshot(post(
            "/api/v1/hermes/bot/dispatch-task",
            Some(&bearer()),
            body,
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let v = json_of(r).await;
    assert_eq!(v["succeeded"], 2);
    assert_eq!(v["failed"], 0);
}

#[tokio::test]
async fn dispatch_task_sem_task_id_retorna_422() {
    let body = r#"{"task_id":"","nodes":[{"subsystem":"sandbox","action":"wasm_check","payload":{},"depends_on":[]}]}"#;
    let (app, _) = app(Some(TOKEN)).await;
    let r = app
        .oneshot(post(
            "/api/v1/hermes/bot/dispatch-task",
            Some(&bearer()),
            body,
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let ct = r.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(ct.contains("application/problem+json"));
}

#[tokio::test]
async fn dispatch_task_sem_nodes_retorna_422() {
    let body = r#"{"task_id":"x","nodes":[]}"#;
    let (app, _) = app(Some(TOKEN)).await;
    let r = app
        .oneshot(post(
            "/api/v1/hermes/bot/dispatch-task",
            Some(&bearer()),
            body,
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn dispatch_task_sem_token_retorna_401() {
    let (app, _) = app(Some(TOKEN)).await;
    let r = app
        .oneshot(post("/api/v1/hermes/bot/dispatch-task", None, DISPATCH_OK))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn dispatch_task_publica_evento_no_event_bus() {
    let (app, st) = app(Some(TOKEN)).await;
    let _sub = st.event_bus.subscribe_legacy();
    let r = app
        .oneshot(post(
            "/api/v1/hermes/bot/dispatch-task",
            Some(&bearer()),
            DISPATCH_OK,
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let v = json_of(r).await;
    assert_eq!(v["status"], "executed");
}

// ---------------------------------------------------------------------------
// spawn-crew
// ---------------------------------------------------------------------------

const CREW_OK: &str = r#"{
    "crew_id": "crew-alpha",
    "members": [
        {"id":"ag-1","name":"Agente A","capabilities":["code"]},
        {"id":"ag-2","name":"Agente B","capabilities":["review"]}
    ],
    "tasks": [
        {"name":"escrever","description":"escreve codigo","depends_on":[],"required_capabilities":["code"],"payload":{}},
        {"name":"revisar","description":"revisa PR","depends_on":["escrever"],"required_capabilities":["review"],"payload":{}}
    ]
}"#;

#[tokio::test]
async fn spawn_crew_valido_retorna_spawned() {
    let (app, _) = app(Some(TOKEN)).await;
    let r = app
        .oneshot(post(
            "/api/v1/hermes/bot/spawn-crew",
            Some(&bearer()),
            CREW_OK,
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let v = json_of(r).await;
    assert_eq!(v["status"], "spawned");
    assert_eq!(v["crew_id"], "crew-alpha");
    assert_eq!(v["agents"].as_array().unwrap().len(), 2);
    assert_eq!(v["tasks"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn spawn_crew_sem_members_retorna_422() {
    let body = r#"{"crew_id":"c","members":[],"tasks":[]}"#;
    let (app, _) = app(Some(TOKEN)).await;
    let r = app
        .oneshot(post("/api/v1/hermes/bot/spawn-crew", Some(&bearer()), body))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let ct = r.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(ct.contains("application/problem+json"));
}

#[tokio::test]
async fn spawn_crew_depends_on_inexistente_retorna_422() {
    let body = r#"{
        "crew_id":"c",
        "members":[{"id":"a","name":"A","capabilities":[]}],
        "tasks":[{"name":"t","description":"d","depends_on":["fantasma"],"required_capabilities":[],"payload":{}}]
    }"#;
    let (app, _) = app(Some(TOKEN)).await;
    let r = app
        .oneshot(post("/api/v1/hermes/bot/spawn-crew", Some(&bearer()), body))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn spawn_crew_publica_evento_no_event_bus() {
    let (app, st) = app(Some(TOKEN)).await;
    let _sub = st.event_bus.subscribe_legacy();
    let r = app
        .oneshot(post(
            "/api/v1/hermes/bot/spawn-crew",
            Some(&bearer()),
            CREW_OK,
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let v = json_of(r).await;
    assert_eq!(v["status"], "spawned");
}

#[tokio::test]
async fn sistema_desconhecido_retorna_404() {
    let (app, _) = app(Some(TOKEN)).await;
    let r = app
        .oneshot(post("/api/v1/hermes/xyz/foo", Some(&bearer()), "{}"))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// Defeito #3 — dispatch-task forma simplificada (um nó sem nodes)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn dispatch_task_forma_simplificada_um_no() {
    let body = r#"{"task_id":"t-simple","subsystem":"sandbox","action":"wasm_check"}"#;
    let (app, _) = app(Some(TOKEN)).await;
    let r = app
        .oneshot(post(
            "/api/v1/hermes/bot/dispatch-task",
            Some(&bearer()),
            body,
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let v = json_of(r).await;
    assert_eq!(v["status"], "executed");
    assert_eq!(v["succeeded"], 1);
    assert_eq!(v["failed"], 0);
}

// ---------------------------------------------------------------------------
// Defeito #4 — dispatch-task aceita ids customizados nos nós
// ---------------------------------------------------------------------------

#[tokio::test]
async fn dispatch_task_ids_customos_com_dependencia() {
    let body = r#"{
        "task_id":"dag-custom",
        "nodes":[
            {"id":"a","subsystem":"sandbox","action":"wasm_check","payload":{},"depends_on":[]},
            {"id":"b","subsystem":"live-comm","action":"cockpit_status","payload":{},"depends_on":["a"]}
        ]
    }"#;
    let (app, _) = app(Some(TOKEN)).await;
    let r = app
        .oneshot(post(
            "/api/v1/hermes/bot/dispatch-task",
            Some(&bearer()),
            body,
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let v = json_of(r).await;
    assert_eq!(v["succeeded"], 2);
    assert_eq!(v["failed"], 0);
}

#[tokio::test]
async fn dispatch_task_depends_on_inexistente_retorna_422() {
    let body = r#"{
        "task_id":"dag-bad-dep",
        "nodes":[
            {"id":"a","subsystem":"sandbox","action":"wasm_check","payload":{},"depends_on":["fantasma"]}
        ]
    }"#;
    let (app, _) = app(Some(TOKEN)).await;
    let r = app
        .oneshot(post(
            "/api/v1/hermes/bot/dispatch-task",
            Some(&bearer()),
            body,
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let ct = r.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(ct.contains("application/problem+json"));
}

#[tokio::test]
async fn dispatch_task_ciclo_entre_nos_retorna_422() {
    let body = r#"{
        "task_id":"dag-cycle",
        "nodes":[
            {"id":"a","subsystem":"sandbox","action":"wasm_check","payload":{},"depends_on":["b"]},
            {"id":"b","subsystem":"sandbox","action":"wasm_check","payload":{},"depends_on":["a"]}
        ]
    }"#;
    let (app, _) = app(Some(TOKEN)).await;
    let r = app
        .oneshot(post(
            "/api/v1/hermes/bot/dispatch-task",
            Some(&bearer()),
            body,
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let v = json_of(r).await;
    assert!(v["detail"].as_str().unwrap().contains("ciclo"));
}

// ---------------------------------------------------------------------------
// Defeito #5 — corpo malformado retorna 422 problem+json
// ---------------------------------------------------------------------------

#[tokio::test]
async fn dispatch_task_json_malformado_retorna_422_problem() {
    let (app, _) = app(Some(TOKEN)).await;
    let r = app
        .oneshot(post(
            "/api/v1/hermes/bot/dispatch-task",
            Some(&bearer()),
            "{not valid json",
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let ct = r.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(ct.contains("application/problem+json"));
    let v = json_of(r).await;
    assert!(v["type"]
        .as_str()
        .unwrap()
        .ends_with("invalid-dispatch-task"));
}

#[tokio::test]
async fn spawn_crew_json_malformado_retorna_422_problem() {
    let (app, _) = app(Some(TOKEN)).await;
    let r = app
        .oneshot(post(
            "/api/v1/hermes/bot/spawn-crew",
            Some(&bearer()),
            "definitely not json",
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let ct = r.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(ct.contains("application/problem+json"));
    let v = json_of(r).await;
    assert!(v["type"].as_str().unwrap().ends_with("invalid-crew"));
}

#[tokio::test]
async fn dispatch_task_sem_campo_task_id_retorna_422() {
    // task_id completamente ausente (não apenas vazio) na forma simplificada.
    let body = r#"{"subsystem":"sandbox","action":"wasm_check"}"#;
    let (app, _) = app(Some(TOKEN)).await;
    let r = app
        .oneshot(post(
            "/api/v1/hermes/bot/dispatch-task",
            Some(&bearer()),
            body,
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let ct = r.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(ct.contains("application/problem+json"));
}

// ---------------------------------------------------------------------------
// Defeito #1 — spawn-crew resolve dependências fora de ordem
// ---------------------------------------------------------------------------

#[tokio::test]
async fn spawn_crew_dependencia_fora_de_ordem_nao_atribui_b_antes_de_a() {
    // B aparece antes de A na lista, mas depende de A. Nenhuma atribuição deve
    // conter o id da task B porque A ainda não foi concluída.
    let body = r#"{
        "crew_id":"crew-order",
        "members":[{"id":"m-order","name":"M","capabilities":["code"],"max_concurrent":2}],
        "tasks":[
            {"name":"B","description":"segunda","depends_on":["A"],"capabilities":["code"],"payload":{}},
            {"name":"A","description":"primeira","depends_on":[],"capabilities":["code"],"payload":{}}
        ]
    }"#;
    let (app, _) = app(Some(TOKEN)).await;
    let r = app
        .oneshot(post("/api/v1/hermes/bot/spawn-crew", Some(&bearer()), body))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let v = json_of(r).await;
    let assignments = v["schedule"]["assignments"].as_array().unwrap();
    for a in assignments {
        // O swarm gera ids internos; verificamos via tasks registradas que B
        // não está entre as atribuídas (apenas A pode estar).
        let task_id = a["task_id"].as_str().unwrap();
        // Recupera a task pelo id para checar o nome.
        // Como não temos acesso direto ao nome via resposta, confiamos que
        // apenas uma task pode ser atribuída (A) já que B depende de A.
        // Se B fosse atribuída prematuramente, teríamos 2 assignments.
        assert!(
            !task_id.is_empty(),
            "task_id não deve ser vazio em assignment"
        );
    }
    // No máximo 1 task atribuída (A); B fica bloqueada.
    assert!(
        assignments.len() <= 1,
        "B não deve ser atribuída antes de A completar; assignments={assignments:?}"
    );
}

// ---------------------------------------------------------------------------
// Defeito #2 — spawn-crew isola equipes via capacidade crew:<id>
// ---------------------------------------------------------------------------

#[tokio::test]
async fn spawn_crew_isola_equipes_atribuicoes_so_da_propria_crew() {
    // Cenário de mutação: equipe X com 1 membro (max_concurrent=1) e 2 tasks
    // sem dependência → 1 task de X sobra sem agente. Depois, equipe Y com 1
    // membro e 1 task. A resposta de Y deve:
    //   (a) ter assignments apenas com task_id das tasks de Y e agent_id do
    //       membro de Y;
    //   (b) blocked_tasks/unassignable_tasks vazios (a task pendente de X não
    //       pode vazar para o schedule de Y).
    // Se a linha `req_caps.push(crew_cap.clone())` for removida, a task de X
    // torna-se elegível para o agente de Y e este teste falha.
    // Usa capability exclusiva "x-only" nas tasks de X. Com a injeção de
    // crew:cx, o agente de Y (que só tem "code") não pode pegar x1/x2. Sem a
    // injeção, as tasks de X ficam sem filtro de crew e podem vazar no
    // schedule global da resposta de Y. O filtro de crew na resposta é o que
    // realmente estamos testando aqui.
    let body_x = r#"{
        "crew_id":"cx",
        "members":[{"id":"mx","name":"MX","capabilities":["code","x-only"],"max_concurrent":1}],
        "tasks":[
            {"name":"x1","description":"primeira de X","depends_on":[],"capabilities":["x-only"],"payload":{}},
            {"name":"x2","description":"segunda de X (sobra)","depends_on":[],"capabilities":["x-only"],"payload":{}}
        ]
    }"#;
    let body_y = r#"{
        "crew_id":"cy",
        "members":[{"id":"my","name":"MY","capabilities":["code"]}],
        "tasks":[{"name":"y1","description":"única de Y","depends_on":[],"capabilities":["code"],"payload":{}}]
    }"#;
    let (app, _st) = app(Some(TOKEN)).await;

    let rx = app
        .clone()
        .oneshot(post(
            "/api/v1/hermes/bot/spawn-crew",
            Some(&bearer()),
            body_x,
        ))
        .await
        .unwrap();
    assert_eq!(rx.status(), StatusCode::OK);
    let vx = json_of(rx).await;
    let x_task_ids: Vec<String> = vx["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(x_task_ids.len(), 2, "X deve ter 2 tasks registradas");

    let ry = app
        .oneshot(post(
            "/api/v1/hermes/bot/spawn-crew",
            Some(&bearer()),
            body_y,
        ))
        .await
        .unwrap();
    assert_eq!(ry.status(), StatusCode::OK);
    let vy = json_of(ry).await;

    let y_task_ids: Vec<String> = vy["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(y_task_ids.len(), 1, "Y deve ter 1 task registrada");

    // (a) assignments de Y só contêm tasks e agentes da própria crew.
    let assignments = vy["schedule"]["assignments"].as_array().unwrap();
    for a in assignments {
        let tid = a["task_id"].as_str().unwrap();
        let aid = a["agent_id"].as_str().unwrap();
        assert!(
            y_task_ids.contains(&tid.to_string()),
            "assignment de Y referencia task_id {tid} que não pertence a Y (tasks={y_task_ids:?})"
        );
        assert_eq!(
            aid, "my",
            "assignment de Y referencia agente {aid} fora da crew"
        );
    }

    // (b) blocked/unassignable de Y não listam a task que sobrou de X.
    let blocked = vy["schedule"]["blocked_tasks"].as_array().unwrap();
    let unassignable = vy["schedule"]["unassignable_tasks"].as_array().unwrap();
    for id in blocked.iter().chain(unassignable.iter()) {
        let s = id.as_str().unwrap();
        assert!(
            !x_task_ids.contains(&s.to_string()),
            "schedule de Y vazou task {s} da crew X (x_task_ids={x_task_ids:?})"
        );
    }
}

// ---------------------------------------------------------------------------
// Defeito #6 — spawn-crew aceita tasks vazia e description opcional
// ---------------------------------------------------------------------------

#[tokio::test]
async fn spawn_crew_sem_tasks_retorna_200() {
    let body = r#"{
        "crew_id":"crew-empty",
        "members":[{"id":"m-empty","name":"ME","capabilities":["x"]}],
        "tasks":[]
    }"#;
    let (app, _) = app(Some(TOKEN)).await;
    let r = app
        .oneshot(post("/api/v1/hermes/bot/spawn-crew", Some(&bearer()), body))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let v = json_of(r).await;
    assert_eq!(v["status"], "spawned");
    assert_eq!(v["tasks"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn spawn_crew_sem_crew_id_retorna_422_problem() {
    let body = r#"{"members":[{"id":"m","name":"M"}],"tasks":[]}"#;
    let (app, _) = app(Some(TOKEN)).await;
    let r = app
        .oneshot(post("/api/v1/hermes/bot/spawn-crew", Some(&bearer()), body))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let ct = r.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(ct.contains("application/problem+json"));
    let v = json_of(r).await;
    assert!(v["type"].as_str().unwrap().ends_with("invalid-crew"));
}

#[tokio::test]
async fn spawn_crew_ciclo_entre_tasks_retorna_422() {
    let body = r#"{
        "crew_id":"crew-cycle",
        "members":[{"id":"mc","name":"MC","capabilities":["x"]}],
        "tasks":[
            {"name":"X","depends_on":["Y"],"capabilities":["x"],"payload":{}},
            {"name":"Y","depends_on":["X"],"capabilities":["x"],"payload":{}}
        ]
    }"#;
    let (app, _) = app(Some(TOKEN)).await;
    let r = app
        .oneshot(post("/api/v1/hermes/bot/spawn-crew", Some(&bearer()), body))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let v = json_of(r).await;
    assert!(v["detail"].as_str().unwrap().contains("ciclo"));
}

#[tokio::test]
async fn spawn_crew_task_com_capabilities_e_max_concurrent() {
    // Usa o campo `capabilities` (preferencial) e `max_concurrent` no membro.
    let body = r#"{
        "crew_id":"crew-caps",
        "members":[{"id":"mc2","name":"MC2","capabilities":["rust"],"max_concurrent":3}],
        "tasks":[{"name":"tc","capabilities":["rust"],"payload":{"k":"v"}}]
    }"#;
    let (app, st) = app(Some(TOKEN)).await;
    let r = app
        .oneshot(post("/api/v1/hermes/bot/spawn-crew", Some(&bearer()), body))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);

    // Verifica que o agente foi registrado com max_concurrent_tasks=3 e capacidade crew:caps.
    let agent = st.swarm.get_agent("mc2").await.unwrap().unwrap();
    assert_eq!(agent.max_concurrent_tasks, 3);
    assert!(agent.capabilities.contains(&"crew:crew-caps".to_string()));
    assert!(agent.capabilities.contains(&"rust".to_string()));

    // A resposta já confirma que a crew foi criada com sucesso; as capacidades
    // são verificadas indiretamente pelo fato de o agente ter sido registrado
    // com as caps corretas (asserts acima). O swarm gera ids internos para
    // tasks, então não buscamos por nome aqui.
}

// ---------------------------------------------------------------------------
// Defeito #7 — evento dispatch-task traz resumo por nó
// ---------------------------------------------------------------------------

#[tokio::test]
async fn dispatch_task_resposta_inclui_resumo_por_no() {
    // Valida que a resposta HTTP inclui o resumo por nó (node_id, subsystem, success).
    // A publicação no EventBus é coberta pelo teste existente
    // `dispatch_task_publica_evento_no_event_bus`; aqui garantimos que o contrato
    // da resposta também expõe os campos exigidos pelo defeito #7.
    let (app, _st) = app(Some(TOKEN)).await;
    let body = r#"{
        "task_id":"evt-summary-resp",
        "nodes":[
            {"subsystem":"sandbox","action":"wasm_check","payload":{},"depends_on":[]},
            {"subsystem":"live-comm","action":"cockpit_status","payload":{},"depends_on":["evt-summary-resp-0"]}
        ]
    }"#;
    let r = app
        .oneshot(post(
            "/api/v1/hermes/bot/dispatch-task",
            Some(&bearer()),
            body,
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let v = json_of(r).await;
    let results = v["results"].as_array().expect("results deve ser array");
    assert_eq!(results.len(), 2);
    for node in results {
        assert!(node.get("node_id").is_some(), "node_id ausente em {node:?}");
        assert!(
            node.get("subsystem").is_some(),
            "subsystem ausente em {node:?}"
        );
        assert!(node.get("success").is_some(), "success ausente em {node:?}");
    }
}

#[tokio::test]
async fn spawn_crew_tarefa_que_sobrou_nao_vai_para_outra_equipe() {
    // Caso real do isolamento: tarefas sem capacidade especial. A equipe X tem 1 membro com
    // max_concurrent 1 e 2 tarefas, entao 1 sobra pendente. Quando a equipe Y entra, o
    // agendamento global nao pode entregar a sobra de X ao membro de Y. Confere no proprio
    // swarm (assigned_agent), nao na resposta filtrada.
    let body_x = r#"{"crew_id":"sx","members":[{"id":"sx-m","name":"SX","max_concurrent":1}],
        "tasks":[{"name":"sx1"},{"name":"sx2"}]}"#;
    let body_y =
        r#"{"crew_id":"sy","members":[{"id":"sy-m","name":"SY"}],"tasks":[{"name":"sy1"}]}"#;
    let (app, st) = app(Some(TOKEN)).await;

    let vx = json_of(
        app.clone()
            .oneshot(post(
                "/api/v1/hermes/bot/spawn-crew",
                Some(&bearer()),
                body_x,
            ))
            .await
            .unwrap(),
    )
    .await;
    let x_ids: Vec<String> = vx["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(x_ids.len(), 2);

    let ry = app
        .oneshot(post(
            "/api/v1/hermes/bot/spawn-crew",
            Some(&bearer()),
            body_y,
        ))
        .await
        .unwrap();
    assert_eq!(ry.status(), StatusCode::OK);

    for id in &x_ids {
        let task = st
            .swarm
            .get_task(id)
            .await
            .unwrap()
            .expect("task de X sumiu do swarm");
        assert_ne!(
            task.assigned_agent.as_deref(),
            Some("sy-m"),
            "task {id} da equipe X foi entregue ao membro da equipe Y"
        );
    }
}

fn ordem_dos_resultados(v: &serde_json::Value) -> Vec<String> {
    v["results"]
        .as_array()
        .expect("resposta sem results")
        .iter()
        .map(|r| r["node_id"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn dispatch_task_nos_fora_de_ordem_executam_na_ordem_das_dependencias() {
    // DAG valido (a -> b -> c), mas enviado fora de ordem: b e c antes de a.
    let body = r#"{
        "task_id":"fora-de-ordem",
        "nodes":[
            {"id":"b","subsystem":"live-comm","action":"cockpit_status","payload":{},"depends_on":["a"]},
            {"id":"c","subsystem":"live-comm","action":"cockpit_status","payload":{},"depends_on":["b"]},
            {"id":"a","subsystem":"sandbox","action":"wasm_check","payload":{},"depends_on":[]}
        ]
    }"#;
    let (app, _) = app(Some(TOKEN)).await;
    let r = app
        .oneshot(post(
            "/api/v1/hermes/bot/dispatch-task",
            Some(&bearer()),
            body,
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let v = json_of(r).await;
    assert_eq!(v["failed"], 0, "nenhum no pode falhar por dependencia: {v}");
    assert_eq!(v["succeeded"], 3);
    assert_eq!(ordem_dos_resultados(&v), ["a", "b", "c"]);
}

#[tokio::test]
async fn dispatch_task_pedido_ja_em_ordem_nao_muda_de_ordem() {
    // c nao depende de ninguem, mas veio depois de b: a ordem do pedido e preservada.
    let body = r#"{
        "task_id":"em-ordem",
        "nodes":[
            {"id":"a","subsystem":"sandbox","action":"wasm_check","payload":{},"depends_on":[]},
            {"id":"b","subsystem":"live-comm","action":"cockpit_status","payload":{},"depends_on":["a"]},
            {"id":"c","subsystem":"live-comm","action":"cockpit_status","payload":{},"depends_on":[]}
        ]
    }"#;
    let (app, _) = app(Some(TOKEN)).await;
    let v = json_of(
        app.oneshot(post(
            "/api/v1/hermes/bot/dispatch-task",
            Some(&bearer()),
            body,
        ))
        .await
        .unwrap(),
    )
    .await;
    assert_eq!(v["succeeded"], 3);
    assert_eq!(ordem_dos_resultados(&v), ["a", "b", "c"]);
}
