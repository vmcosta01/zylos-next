//! Entrada remota pelo WebSocket do relay, passando pelo middleware de autenticação de VERDADE
//! (JWT real, usuários diferentes) e com um injetor FALSO: nada mexe no mouse nem no teclado.
//!
//! Regra testada: só injeta quem (1) está com a sessão em controle humano válido E (2) é, segundo o
//! JWT, o dono da concessão. O `user_id` do corpo de "assumir controle" não conta: o gateway grava
//! o `sub` do token.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;
use zylos_auth::tokens::CreateTokenRequest;
use zylos_auth::{TokenConfig, TokenService};
use zylos_computer_view::schema::{CreateSessionRequest, SessionState};
use zylos_gateway::api_auth::{exigir_auth, ApiAuth};
use zylos_gateway::computer_view_api::{create_computer_view_router, ComputerViewHandle};
use zylos_gateway::remote_input::{Entrada, InjetorEntrada};
use zylos_gateway::webrtc_relay::{create_webrtc_relay_router, WebRtcRelayState};

const SEGREDO: &str = "remote-input-test-secret-0123456789abcdef";

#[derive(Default)]
struct Falso(Mutex<Vec<String>>);

impl InjetorEntrada for Falso {
    fn clique(&self, x: i32, y: i32, botao: &str) -> Result<(), String> {
        self.0
            .lock()
            .unwrap()
            .push(format!("clique {x},{y} {botao}"));
        Ok(())
    }
    fn tecla(&self, tecla: &str, apertada: bool) -> Result<(), String> {
        self.0
            .lock()
            .unwrap()
            .push(format!("tecla {tecla} {apertada}"));
        Ok(())
    }
}

struct Ambiente {
    http: String,
    ws: String,
    handle: ComputerViewHandle,
    falso: Arc<Falso>,
    svc: TokenService,
    _srv: tokio::task::JoinHandle<()>,
}

/// `exigir_auth`: com segredo (auth obrigatória) ou sem (modo local, ninguém tem identidade).
async fn subir(com_entrada: bool, auth_obrigatoria: bool) -> Ambiente {
    let svc = TokenService::new(TokenConfig {
        secret: SEGREDO.into(),
        ..TokenConfig::default()
    })
    .unwrap();
    let handle = ComputerViewHandle::new();
    let falso = Arc::new(Falso::default());
    let mut relay = WebRtcRelayState::new();
    if com_entrada {
        relay = relay.with_entrada(Arc::new(Entrada::new(
            falso.clone(),
            handle.manager.clone(),
        )));
    }
    let h = handle.clone().with_relay(relay.clone());
    let app = axum::Router::new()
        .nest(
            "/api/v1",
            create_computer_view_router(h).merge(create_webrtc_relay_router(relay)),
        )
        .layer(axum::middleware::from_fn_with_state(
            ApiAuth::new(auth_obrigatoria.then(|| {
                TokenService::new(TokenConfig {
                    secret: SEGREDO.into(),
                    ..TokenConfig::default()
                })
                .unwrap()
            })),
            exigir_auth,
        ));
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let porta = l.local_addr().unwrap().port();
    let srv = tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
    Ambiente {
        http: format!("http://127.0.0.1:{porta}"),
        ws: format!("ws://127.0.0.1:{porta}"),
        handle,
        falso,
        svc,
        _srv: srv,
    }
}

fn token(a: &Ambiente, usuario: &str, roles: &[&str]) -> String {
    a.svc
        .create_token(&CreateTokenRequest {
            user_id: usuario.into(),
            tenant_id: "t1".into(),
            roles: roles.iter().map(|r| r.to_string()).collect(),
            name: None,
        })
        .unwrap()
        .access_token
}

fn nova_sessao(h: &ComputerViewHandle) -> String {
    let id = h
        .manager
        .create_session(CreateSessionRequest {
            bot_id: "b".into(),
            vnc_endpoint: None,
            metadata: None,
        })
        .id;
    h.manager.update_state(&id, SessionState::Active).unwrap();
    id
}

async fn assumir(a: &Ambiente, id: &str, tok: &str, corpo: &str) -> u16 {
    reqwest::Client::new()
        .post(format!(
            "{}/api/v1/computer-view/sessions/{id}/control",
            a.http
        ))
        .bearer_auth(tok)
        .header("content-type", "application/json")
        .body(corpo.to_string())
        .send()
        .await
        .unwrap()
        .status()
        .as_u16()
}

type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn conectar(a: &Ambiente, id: &str, tok: &str) -> Ws {
    tokio_tungstenite::connect_async(format!(
        "{}/api/v1/ws/computer-view/{id}?access_token={tok}",
        a.ws
    ))
    .await
    .unwrap()
    .0
}

async fn aviso(ws: &mut Ws) -> String {
    let m = tokio::time::timeout(Duration::from_secs(2), ws.next())
        .await
        .expect("o aviso não chegou")
        .unwrap()
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(m.to_text().unwrap()).unwrap();
    assert_eq!(v["type"], "error", "{v}");
    v["message"].as_str().unwrap().to_string()
}

async fn clicar(ws: &mut Ws, x: i32) {
    ws.send(Message::Text(
        serde_json::json!({"type":"click","x":x,"y":1,"button":"left"}).to_string(),
    ))
    .await
    .unwrap();
}

#[tokio::test]
async fn so_o_dono_da_concessao_injeta_e_o_dono_vem_do_jwt() {
    let a = subir(true, true).await;
    let id = nova_sessao(&a.handle);
    let alice = token(&a, "alice", &["admin"]);
    let bob = token(&a, "bob", &["admin"]);

    // Sem o token, nem o WebSocket abre (o middleware recusa).
    assert!(
        tokio_tungstenite::connect_async(format!("{}/api/v1/ws/computer-view/{id}", a.ws))
            .await
            .is_err()
    );

    // Quem não é admin não assume controle (403).
    let leitor = token(&a, "carol", &[]);
    assert_eq!(
        assumir(&a, &id, &leitor, r#"{"user_id":"carol"}"#).await,
        403
    );

    // Alice assume tentando se passar por Bob no corpo: o dono gravado é o `sub` do token.
    assert_eq!(assumir(&a, &id, &alice, r#"{"user_id":"bob"}"#).await, 200);
    let lease = a
        .handle
        .manager
        .get_session(&id)
        .unwrap()
        .control_lease
        .unwrap();
    assert_eq!(lease.user_id, "alice", "o dono vem do JWT, não do corpo");

    // Bob (outro admin, com a conexão dele) tenta clicar: recusado, nada injetado.
    let mut ws_bob = conectar(&a, &id, &bob).await;
    clicar(&mut ws_bob, 7).await;
    assert!(aviso(&mut ws_bob).await.contains("outra pessoa"));
    assert!(a.falso.0.lock().unwrap().is_empty());

    // Alice clica: passa.
    let mut ws_alice = conectar(&a, &id, &alice).await;
    clicar(&mut ws_alice, 10).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        *a.falso.0.lock().unwrap(),
        vec!["clique 10,1 left".to_string()]
    );

    // Bob não consegue tomar o controle de Alice (a concessão está com ela).
    assert_eq!(assumir(&a, &id, &bob, r#"{"user_id":"bob"}"#).await, 409);
    clicar(&mut ws_bob, 8).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(a.falso.0.lock().unwrap().len(), 1, "Bob segue sem injetar");
}

#[tokio::test]
async fn sem_login_obrigatorio_ninguem_tem_identidade_e_nada_e_injetado() {
    // Gateway em modo local (sem segredo): o token nem é verificado, então não há usuário.
    let a = subir(true, false).await;
    let id = nova_sessao(&a.handle);
    // Assumir/liberar controle é ação de administrador logado: sem segredo, o gateway recusa (503).
    assert_eq!(
        reqwest::Client::new()
            .post(format!(
                "{}/api/v1/computer-view/sessions/{id}/control",
                a.http
            ))
            .header("content-type", "application/json")
            .body(r#"{"user_id":"qualquer"}"#)
            .send()
            .await
            .unwrap()
            .status()
            .as_u16(),
        503
    );
    // Mesmo que alguém consiga uma concessão por outro caminho (aqui, direto no gerenciador), a
    // ENTRADA exige identidade verificada: conexão sem login não injeta nada.
    a.handle
        .manager
        .take_control(
            &id,
            zylos_computer_view::schema::TakeControlRequest {
                user_id: "qualquer".into(),
                duration_minutes: Some(5),
                reason: None,
            },
        )
        .unwrap();
    let tok = token(&a, "qualquer", &["admin"]);
    let mut ws = conectar(&a, &id, &tok).await;
    clicar(&mut ws, 1).await;
    assert!(aviso(&mut ws).await.contains("não está logado"));
    assert!(a.falso.0.lock().unwrap().is_empty());
}

#[tokio::test]
async fn gateway_sem_entrada_ligada_avisa_e_nao_injeta() {
    let a = subir(false, true).await;
    let id = nova_sessao(&a.handle);
    let alice = token(&a, "alice", &["admin"]);
    assert_eq!(assumir(&a, &id, &alice, "{}").await, 200);
    let mut ws = conectar(&a, &id, &alice).await;
    clicar(&mut ws, 1).await;
    assert!(aviso(&mut ws).await.contains("desligada"));
    assert!(a.falso.0.lock().unwrap().is_empty());
}

#[tokio::test]
async fn teclas_ruins_e_enxurrada_continuam_cortadas_para_o_dono() {
    let a = subir(true, true).await;
    let id = nova_sessao(&a.handle);
    let alice = token(&a, "alice", &["admin"]);
    assert_eq!(assumir(&a, &id, &alice, "{}").await, 200);
    let mut ws = conectar(&a, &id, &alice).await;

    ws.send(Message::Text(
        serde_json::json!({"type":"key","code":"Meta","pressed":true}).to_string(),
    ))
    .await
    .unwrap();
    assert!(aviso(&mut ws).await.contains("Tecla não permitida"));
    assert!(a.falso.0.lock().unwrap().is_empty());

    for i in 0..200 {
        clicar(&mut ws, i).await;
    }
    assert!(aviso(&mut ws).await.contains("demais por segundo"));
    let n = a.falso.0.lock().unwrap().len();
    assert!(
        (1..=60).contains(&n),
        "no máximo 60 por segundo (foram {n})"
    );
}

async fn tecla(ws: &mut Ws, code: &str, apertada: bool) {
    ws.send(Message::Text(
        serde_json::json!({"type":"key","code":code,"pressed":apertada}).to_string(),
    ))
    .await
    .unwrap();
}

fn registrado(a: &Ambiente) -> Vec<String> {
    a.falso.0.lock().unwrap().clone()
}

#[tokio::test]
async fn soltar_tecla_passa_depois_da_enxurrada_e_depois_de_perder_o_controle() {
    let a = subir(true, true).await;
    let id = nova_sessao(&a.handle);
    let alice = token(&a, "alice", &["admin"]);
    assert_eq!(assumir(&a, &id, &alice, "{}").await, 200);
    let mut ws = conectar(&a, &id, &alice).await;

    // Aperta Control, estoura o limite de 60 eventos/s, e só então solta: o soltar não pode ser
    // descartado pelo limite (a tecla ficaria presa no sistema).
    tecla(&mut ws, "Control", true).await;
    for i in 0..150 {
        clicar(&mut ws, i).await;
    }
    tecla(&mut ws, "Control", false).await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    let r = registrado(&a);
    assert!(r.contains(&"tecla Control true".to_string()), "{r:?}");
    assert!(r.contains(&"tecla Control false".to_string()), "{r:?}");

    // Aperta Shift, perde o controle, solta: o soltar passa mesmo assim.
    tokio::time::sleep(Duration::from_millis(1200)).await; // nova janela de taxa
    tecla(&mut ws, "Shift", true).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(registrado(&a).contains(&"tecla Shift true".to_string()));
    a.handle.manager.release_control(&id, "alice").unwrap();
    tecla(&mut ws, "Shift", false).await;
    tokio::time::sleep(Duration::from_millis(400)).await;
    let r = registrado(&a);
    assert!(r.contains(&"tecla Shift false".to_string()), "{r:?}");
}

#[tokio::test]
async fn conexao_que_cai_solta_as_teclas_que_deixou_apertadas() {
    let a = subir(true, true).await;
    let id = nova_sessao(&a.handle);
    let alice = token(&a, "alice", &["admin"]);
    assert_eq!(assumir(&a, &id, &alice, "{}").await, 200);
    let mut ws = conectar(&a, &id, &alice).await;
    tecla(&mut ws, "Alt", true).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(registrado(&a), vec!["tecla Alt true".to_string()]);
    drop(ws); // a conexão cai sem soltar a tecla
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert_eq!(
        registrado(&a),
        vec!["tecla Alt true".to_string(), "tecla Alt false".to_string()]
    );
}

#[tokio::test]
async fn aviso_de_recusa_vai_so_para_quem_mandou_o_evento() {
    let a = subir(true, true).await;
    let id = nova_sessao(&a.handle);
    let alice = token(&a, "alice", &["admin"]);
    let bob = token(&a, "bob", &["admin"]);
    assert_eq!(assumir(&a, &id, &alice, "{}").await, 200);
    let mut ws_alice = conectar(&a, &id, &alice).await;
    let mut ws_bob = conectar(&a, &id, &bob).await;
    clicar(&mut ws_bob, 5).await;
    assert!(aviso(&mut ws_bob).await.contains("outra pessoa"));
    // Alice (a dona) não recebe nada por causa do clique do Bob.
    let da_alice = tokio::time::timeout(Duration::from_millis(500), ws_alice.next()).await;
    assert!(da_alice.is_err(), "Alice recebeu: {da_alice:?}");
}
