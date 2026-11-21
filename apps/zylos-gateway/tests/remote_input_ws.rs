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
    // O MESMO serviço (e a mesma lista de revogados) no middleware e no relay, como no gateway real.
    let mut relay = WebRtcRelayState::new().with_verificador(auth_obrigatoria.then(|| svc.clone()));
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
            ApiAuth::new(auth_obrigatoria.then(|| svc.clone())),
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
    token_em(a, "t1", usuario, roles)
}

fn token_em(a: &Ambiente, tenant: &str, usuario: &str, roles: &[&str]) -> String {
    a.svc
        .create_token(&CreateTokenRequest {
            user_id: usuario.into(),
            tenant_id: tenant.into(),
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
    assert_eq!(
        lease.user_id, "t1:alice",
        "o dono (tenant:sub) vem do JWT, não do corpo"
    );

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
    a.handle.manager.release_control(&id, "t1:alice").unwrap();
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

async fn empurrar_quadro(a: &Ambiente, id: &str, tok: &str, corpo: &'static [u8]) -> u16 {
    reqwest::Client::new()
        .post(format!(
            "{}/api/v1/computer-view/sessions/{id}/frame",
            a.http
        ))
        .bearer_auth(tok)
        .body(corpo)
        .send()
        .await
        .unwrap()
        .status()
        .as_u16()
}

/// PNG mínimo: a rota só confere os bytes iniciais.
const PNG_FALSO: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3];

fn sessao_vnc(h: &ComputerViewHandle) -> String {
    let id = h
        .manager
        .create_session(CreateSessionRequest {
            bot_id: "b".into(),
            vnc_endpoint: Some("vnc://10.0.0.5:5900".into()),
            metadata: None,
        })
        .id;
    h.manager.update_state(&id, SessionState::Active).unwrap();
    id
}

#[tokio::test]
async fn com_a_entrada_ligada_quadro_de_fora_nao_chega_a_quem_tem_o_controle() {
    let a = subir(true, true).await;
    let id = nova_sessao(&a.handle);
    let alice = token(&a, "alice", &["admin"]);
    let mallory = token(&a, "mallory", &[]); // leitor, sem admin
    assert_eq!(assumir(&a, &id, &alice, "{}").await, 200);
    let mut ws_alice = conectar(&a, &id, &alice).await;

    // O leitor tenta mostrar uma tela falsa à dona do controle: recusado.
    assert_eq!(empurrar_quadro(&a, &id, &mallory, PNG_FALSO).await, 403);
    // Nem o admin consegue empurrar: nessa sessão a tela vem só da captura do gateway.
    assert_eq!(empurrar_quadro(&a, &id, &alice, PNG_FALSO).await, 403);
    let recebido = tokio::time::timeout(Duration::from_millis(500), ws_alice.next()).await;
    assert!(recebido.is_err(), "Alice recebeu um quadro: {recebido:?}");
}

#[tokio::test]
async fn quadro_de_fora_segue_valendo_em_sessao_vnc_e_com_a_entrada_desligada() {
    // Entrada ligada, mas a sessão é VNC: a entrada nunca injeta nela, então o quadro é aceito.
    let a = subir(true, true).await;
    let vnc = sessao_vnc(&a.handle);
    let tok = token(&a, "alice", &["admin"]);
    assert_eq!(empurrar_quadro(&a, &vnc, &tok, PNG_FALSO).await, 200);
    // Entrada desligada: nada é injetado, o agente pode empurrar quadros como antes.
    let b = subir(false, true).await;
    let id = nova_sessao(&b.handle);
    let tok_b = token(&b, "bob", &[]);
    assert_eq!(empurrar_quadro(&b, &id, &tok_b, PNG_FALSO).await, 200);
}

#[tokio::test]
async fn logout_derruba_a_conexao_que_ja_estava_aberta() {
    let a = subir(true, true).await;
    let id = nova_sessao(&a.handle);
    let alice = token(&a, "alice", &["admin"]);
    assert_eq!(assumir(&a, &id, &alice, "{}").await, 200);
    let mut ws = conectar(&a, &id, &alice).await;
    clicar(&mut ws, 1).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(registrado(&a), vec!["clique 1,1 left".to_string()]);

    // Logout: o token entra na lista de revogados.
    a.svc.revoke_token(&alice).unwrap();
    clicar(&mut ws, 2).await;
    tecla(&mut ws, "a", true).await;
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert_eq!(
        registrado(&a),
        vec!["clique 1,1 left".to_string()],
        "depois do logout nada mais é injetado"
    );
    // E a conexão foi fechada pelo gateway.
    let fim = tokio::time::timeout(Duration::from_secs(2), async {
        while let Some(m) = ws.next().await {
            if m.is_err() {
                return;
            }
        }
    })
    .await;
    assert!(fim.is_ok(), "a conexão deveria ter sido fechada");
}

#[tokio::test]
async fn so_uma_sessao_por_vez_usa_o_mouse_e_o_teclado_da_maquina() {
    let a = subir(true, true).await;
    let s1 = nova_sessao(&a.handle);
    let s2 = nova_sessao(&a.handle);
    let alice = token(&a, "alice", &["admin"]);
    let bob = token(&a, "bob", &["admin"]);
    assert_eq!(assumir(&a, &s1, &alice, "{}").await, 200);
    assert_eq!(assumir(&a, &s2, &bob, "{}").await, 200);
    let mut ws_a = conectar(&a, &s1, &alice).await;
    let mut ws_b = conectar(&a, &s2, &bob).await;

    clicar(&mut ws_a, 1).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    // Bob tem controle válido na outra sessão, mas a máquina está com a Alice.
    clicar(&mut ws_b, 2).await;
    assert!(aviso(&mut ws_b).await.contains("Outra sessão"));
    assert_eq!(registrado(&a), vec!["clique 1,1 left".to_string()]);

    // Alice larga o controle: a vez passa ao Bob.
    a.handle.manager.release_control(&s1, "t1:alice").unwrap();
    clicar(&mut ws_b, 3).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        registrado(&a),
        vec!["clique 1,1 left".to_string(), "clique 3,1 left".to_string()]
    );
}

#[tokio::test]
async fn varias_conexoes_do_mesmo_dono_dividem_o_limite_de_60_por_segundo() {
    let a = subir(true, true).await;
    let id = nova_sessao(&a.handle);
    let alice = token(&a, "alice", &["admin"]);
    assert_eq!(assumir(&a, &id, &alice, "{}").await, 200);
    let mut conexoes = Vec::new();
    for _ in 0..5 {
        conexoes.push(conectar(&a, &id, &alice).await);
    }
    for ws in conexoes.iter_mut() {
        for i in 0..100 {
            clicar(ws, i).await;
        }
    }
    tokio::time::sleep(Duration::from_millis(600)).await;
    let n = registrado(&a).len();
    assert!(
        (1..=60).contains(&n),
        "5 conexões não podem multiplicar o limite (foram {n})"
    );
}

#[tokio::test]
async fn atalhos_que_abrem_menus_do_sistema_nao_passam_pelo_websocket() {
    let a = subir(true, true).await;
    let id = nova_sessao(&a.handle);
    let alice = token(&a, "alice", &["admin"]);
    assert_eq!(assumir(&a, &id, &alice, "{}").await, 200);
    let mut ws = conectar(&a, &id, &alice).await;
    tecla(&mut ws, "Control", true).await;
    tecla(&mut ws, "Shift", true).await;
    tecla(&mut ws, "Escape", true).await;
    assert!(aviso(&mut ws).await.contains("Atalho bloqueado"));
    tecla(&mut ws, "Shift", false).await;
    tecla(&mut ws, "Control", false).await;
    tecla(&mut ws, "Alt", true).await;
    tecla(&mut ws, "Tab", true).await;
    tokio::time::sleep(Duration::from_millis(400)).await;
    let r = registrado(&a);
    assert!(!r.contains(&"tecla Escape true".to_string()), "{r:?}");
    assert!(!r.contains(&"tecla Tab true".to_string()), "{r:?}");
}

#[tokio::test]
async fn screenshot_nao_fotografa_o_computador_sem_a_captura_local_nem_em_sessao_vnc() {
    // Só este teste mexe nesta variável (cada arquivo de teste é um processo próprio).
    std::env::remove_var(zylos_gateway::computer_view_api::CAPTURA_LOCAL_ENV);
    let a = subir(false, false).await;
    let local = nova_sessao(&a.handle);
    let vnc = sessao_vnc(&a.handle);
    let get = |id: String| {
        let url = format!("{}/api/v1/computer-view/sessions/{id}/screenshot", a.http);
        async move { reqwest::get(url).await.unwrap().status().as_u16() }
    };
    assert_eq!(get(local).await, 403, "captura local desligada");
    assert_eq!(
        get(vnc).await,
        409,
        "a tela da sessão VNC não é a deste computador"
    );
    assert_eq!(get("nao-existe".into()).await, 404);
}

#[tokio::test]
async fn ping_depois_do_logout_tambem_derruba_a_conexao() {
    let a = subir(true, true).await;
    let id = nova_sessao(&a.handle);
    let alice = token(&a, "alice", &["admin"]);
    assert_eq!(assumir(&a, &id, &alice, "{}").await, 200);
    let mut ws = conectar(&a, &id, &alice).await;
    a.svc.revoke_token(&alice).unwrap();
    // Só ping: antes do conserto, isto mantinha a conexão viva para sempre.
    ws.send(Message::Text(
        serde_json::json!({"type":"ping","ts":1}).to_string(),
    ))
    .await
    .unwrap();
    let fim = tokio::time::timeout(Duration::from_secs(2), async {
        while let Some(m) = ws.next().await {
            if m.is_err() {
                return;
            }
        }
    })
    .await;
    assert!(
        fim.is_ok(),
        "ping com token revogado deveria fechar a conexão"
    );
}

#[tokio::test]
async fn toque_avisa_que_nao_e_suportado_e_nao_injeta() {
    let a = subir(true, true).await;
    let id = nova_sessao(&a.handle);
    let alice = token(&a, "alice", &["admin"]);
    assert_eq!(assumir(&a, &id, &alice, "{}").await, 200);
    let mut ws = conectar(&a, &id, &alice).await;
    ws.send(Message::Text(
        serde_json::json!({"type":"touch","x":1,"y":1,"action":"start"}).to_string(),
    ))
    .await
    .unwrap();
    assert!(aviso(&mut ws).await.to_lowercase().contains("toque"));
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(registrado(&a).is_empty(), "toque não vira clique");
}

#[tokio::test]
async fn mesmo_sub_em_outro_tenant_nao_e_o_mesmo_dono() {
    let a = subir(true, true).await;
    let id = nova_sessao(&a.handle);
    let alice_t1 = token_em(&a, "t1", "alice", &["admin"]);
    let alice_t2 = token_em(&a, "t2", "alice", &["admin"]);
    assert_eq!(assumir(&a, &id, &alice_t1, "{}").await, 200);

    // A "alice" do tenant t2 tem o mesmo `sub` e é admin, mas não é a dona da concessão do t1.
    let mut ws_t2 = conectar(&a, &id, &alice_t2).await;
    clicar(&mut ws_t2, 7).await;
    assert!(aviso(&mut ws_t2).await.contains("outra pessoa"));
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(registrado(&a).is_empty(), "outro tenant não injeta");

    // A dona de verdade segue injetando.
    let mut ws_t1 = conectar(&a, &id, &alice_t1).await;
    clicar(&mut ws_t1, 10).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(registrado(&a), vec!["clique 10,1 left".to_string()]);
}

#[tokio::test]
async fn mesmo_dono_sem_papel_admin_nao_injeta() {
    let a = subir(true, true).await;
    let id = nova_sessao(&a.handle);
    let admin = token(&a, "alice", &["admin"]);
    // Mesmo tenant e mesmo `sub`, mas um token emitido só com papel de leitura.
    let leitor = token(&a, "alice", &["viewer"]);
    assert_eq!(assumir(&a, &id, &admin, "{}").await, 200);

    let mut ws = conectar(&a, &id, &leitor).await;
    clicar(&mut ws, 5).await;
    assert!(aviso(&mut ws)
        .await
        .to_lowercase()
        .contains("administrador"));
    tecla(&mut ws, "a", true).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(registrado(&a).is_empty(), "sem papel admin nada é injetado");
}
