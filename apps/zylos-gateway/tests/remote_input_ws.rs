//! Entrada remota pelo WebSocket do relay, com um injetor FALSO (nada mexe no mouse nem no
//! teclado de verdade): sem controle humano nada passa; com controle, só o que a lista permite.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;
use zylos_computer_view::schema::{CreateSessionRequest, SessionState, TakeControlRequest};
use zylos_gateway::computer_view_api::ComputerViewHandle;
use zylos_gateway::remote_input::{Entrada, InjetorEntrada};
use zylos_gateway::webrtc_relay::{create_webrtc_relay_router, WebRtcRelayState};

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

async fn subir(
    com_entrada: bool,
) -> (
    String,
    ComputerViewHandle,
    Arc<Falso>,
    tokio::task::JoinHandle<()>,
) {
    let handle = ComputerViewHandle::new();
    let falso = Arc::new(Falso::default());
    let mut relay = WebRtcRelayState::new();
    if com_entrada {
        relay = relay.with_entrada(Arc::new(Entrada::new(
            falso.clone(),
            handle.manager.clone(),
        )));
    }
    let app = create_webrtc_relay_router(relay);
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let porta = l.local_addr().unwrap().port();
    let srv = tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
    (format!("ws://127.0.0.1:{porta}"), handle, falso, srv)
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

type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

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

async fn sem_aviso(ws: &mut Ws) {
    assert!(
        tokio::time::timeout(Duration::from_millis(300), ws.next())
            .await
            .is_err(),
        "não devia haver aviso"
    );
}

async fn enviar(ws: &mut Ws, json: serde_json::Value) {
    ws.send(Message::Text(json.to_string())).await.unwrap();
}

#[tokio::test]
async fn sem_controle_nada_passa_e_com_controle_so_o_permitido() {
    let (url, handle, falso, _srv) = subir(true).await;
    let id = nova_sessao(&handle);
    let (mut ws, _) = tokio_tungstenite::connect_async(format!("{url}/ws/computer-view/{id}"))
        .await
        .unwrap();

    // 1. Sem controle assumido: recusa, avisa, e o injetor não recebe nada.
    enviar(
        &mut ws,
        serde_json::json!({"type":"click","x":10,"y":20,"button":"left"}),
    )
    .await;
    assert!(aviso(&mut ws).await.contains("Assuma o controle"));
    enviar(
        &mut ws,
        serde_json::json!({"type":"key","code":"a","pressed":true}),
    )
    .await;
    sem_aviso(&mut ws).await; // mesmo aviso repetido não é reenviado
    assert!(falso.0.lock().unwrap().is_empty());

    // 2. Assumiu o controle: clique e tecla passam.
    handle
        .manager
        .take_control(
            &id,
            TakeControlRequest {
                user_id: "u1".into(),
                duration_minutes: Some(5),
                reason: None,
            },
        )
        .unwrap();
    enviar(
        &mut ws,
        serde_json::json!({"type":"click","x":10,"y":20,"button":"left"}),
    )
    .await;
    enviar(
        &mut ws,
        serde_json::json!({"type":"key","code":"a","pressed":true}),
    )
    .await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        *falso.0.lock().unwrap(),
        vec!["clique 10,20 left".to_string(), "tecla a true".to_string()]
    );

    // 3. Tecla Windows (Meta), botão inventado e coordenada absurda: recusados.
    enviar(
        &mut ws,
        serde_json::json!({"type":"key","code":"Meta","pressed":true}),
    )
    .await;
    assert!(aviso(&mut ws).await.contains("Tecla não permitida"));
    enviar(
        &mut ws,
        serde_json::json!({"type":"click","x":1,"y":1,"button":"back"}),
    )
    .await;
    assert!(aviso(&mut ws).await.contains("Botão"));
    enviar(
        &mut ws,
        serde_json::json!({"type":"click","x":99999,"y":1,"button":"left"}),
    )
    .await;
    assert!(aviso(&mut ws).await.contains("Coordenada"));
    assert_eq!(falso.0.lock().unwrap().len(), 2, "nada novo foi injetado");

    // 4. Liberou o controle: volta a recusar.
    handle.manager.release_control(&id, "u1").unwrap();
    enviar(
        &mut ws,
        serde_json::json!({"type":"click","x":5,"y":5,"button":"left"}),
    )
    .await;
    assert!(aviso(&mut ws).await.contains("Assuma o controle"));
    assert_eq!(falso.0.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn enxurrada_de_eventos_e_cortada_pelo_limite_de_taxa() {
    let (url, handle, falso, _srv) = subir(true).await;
    let id = nova_sessao(&handle);
    handle
        .manager
        .take_control(
            &id,
            TakeControlRequest {
                user_id: "u1".into(),
                duration_minutes: Some(5),
                reason: None,
            },
        )
        .unwrap();
    let (mut ws, _) = tokio_tungstenite::connect_async(format!("{url}/ws/computer-view/{id}"))
        .await
        .unwrap();
    for i in 0..200 {
        enviar(
            &mut ws,
            serde_json::json!({"type":"click","x":i,"y":1,"button":"left"}),
        )
        .await;
    }
    assert!(aviso(&mut ws).await.contains("demais por segundo"));
    let n = falso.0.lock().unwrap().len();
    assert!(
        (1..=60).contains(&n),
        "no máximo 60 eventos por segundo chegam ao injetor (chegaram {n})"
    );
}

#[tokio::test]
async fn gateway_sem_entrada_ligada_avisa_e_nao_injeta() {
    let (url, handle, falso, _srv) = subir(false).await;
    let id = nova_sessao(&handle);
    handle
        .manager
        .take_control(
            &id,
            TakeControlRequest {
                user_id: "u1".into(),
                duration_minutes: Some(5),
                reason: None,
            },
        )
        .unwrap();
    let (mut ws, _) = tokio_tungstenite::connect_async(format!("{url}/ws/computer-view/{id}"))
        .await
        .unwrap();
    enviar(
        &mut ws,
        serde_json::json!({"type":"click","x":1,"y":1,"button":"left"}),
    )
    .await;
    assert!(aviso(&mut ws).await.contains("desligada"));
    assert!(falso.0.lock().unwrap().is_empty());
}
