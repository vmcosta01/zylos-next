//! Sessão com `vnc_endpoint`: o gateway lê o framebuffer de um servidor VNC (aqui um servidor RFB
//! falso em TCP de verdade) e entrega o quadro, como PNG, a quem assiste no relay.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::Request;
use base64::Engine as _;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tower::ServiceExt;
use zylos_gateway::computer_view_api::{create_computer_view_router, ComputerViewHandle};
use zylos_gateway::webrtc_relay::{RelayServerMsg, WebRtcRelayState};

const LARGURA: u16 = 8;
const ALTURA: u16 = 4;

/// Servidor RFB 3.8, sem autenticação, Raw: a cada pedido manda a tela toda em verde.
/// Conta as conexões aceitas.
async fn servidor_vnc(conexoes: Arc<AtomicUsize>) -> String {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let porta = l.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let Ok((mut s, _)) = l.accept().await else {
                return;
            };
            conexoes.fetch_add(1, Ordering::SeqCst);
            tokio::spawn(async move {
                let _ = s.write_all(b"RFB 003.008\n").await;
                let mut b = [0u8; 12];
                if s.read_exact(&mut b).await.is_err() {
                    return;
                }
                let _ = s.write_all(&[1, 1]).await; // um tipo: "nenhuma"
                let mut x = [0u8; 1];
                let _ = s.read_exact(&mut x).await; // escolha
                let _ = s.write_all(&0u32.to_be_bytes()).await; // SecurityResult ok
                let _ = s.read_exact(&mut x).await; // ClientInit
                let _ = s.write_all(&LARGURA.to_be_bytes()).await;
                let _ = s.write_all(&ALTURA.to_be_bytes()).await;
                let _ = s
                    .write_all(&[32, 24, 0, 1, 0, 255, 0, 255, 0, 255, 16, 8, 0, 0, 0, 0])
                    .await;
                let _ = s.write_all(&4u32.to_be_bytes()).await;
                let _ = s.write_all(b"fake").await;
                let mut lixo = [0u8; 28];
                if s.read_exact(&mut lixo).await.is_err() {
                    return;
                }
                loop {
                    let mut req = [0u8; 10];
                    if s.read_exact(&mut req).await.is_err() {
                        return;
                    }
                    let mut m = vec![0u8, 0, 0, 1];
                    for v in [0u16, 0, LARGURA, ALTURA] {
                        m.extend_from_slice(&v.to_be_bytes());
                    }
                    m.extend_from_slice(&0i32.to_be_bytes());
                    // B=0, G=255, R=0: verde.
                    m.extend([0u8, 255, 0, 0].repeat(usize::from(LARGURA) * usize::from(ALTURA)));
                    if s.write_all(&m).await.is_err() {
                        return;
                    }
                }
            });
        }
    });
    format!("127.0.0.1:{porta}")
}

async fn criar(app: &axum::Router, corpo: &str) -> String {
    let r = app
        .clone()
        .oneshot(
            Request::post("/computer-view/sessions")
                .header("content-type", "application/json")
                .body(Body::from(corpo.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let b = axum::body::to_bytes(r.into_body(), 1 << 20).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&b).unwrap();
    v["session"]["id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn sessao_com_vnc_permitido_vira_quadros_png_no_relay_e_fica_ativa() {
    let conexoes = Arc::new(AtomicUsize::new(0));
    let alvo = servidor_vnc(conexoes.clone()).await;
    let relay = WebRtcRelayState::new();
    let handle = ComputerViewHandle::new()
        .with_vnc_permitidos(vec![alvo.clone()])
        .with_relay(relay.clone());
    let app = create_computer_view_router(handle.clone());

    // O canal do relay é criado por quem se inscreve; o quadro chega assim que o VNC responde.
    let id = criar(
        &app,
        &format!(r#"{{"bot_id":"b","vnc_endpoint":"vnc://{alvo}"}}"#),
    )
    .await;
    let mut rx = relay.get_or_create_channel(&id).unwrap().subscribe();

    let png = loop {
        match tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("nenhum quadro chegou ao relay")
            .unwrap()
        {
            RelayServerMsg::Frame { data, .. } => {
                break base64::engine::general_purpose::STANDARD
                    .decode(data)
                    .unwrap();
            }
            RelayServerMsg::Error { message } => panic!("erro do relay: {message}"),
            _ => {}
        }
    };
    // PNG de verdade, com as dimensões da tela do servidor (IHDR: largura e altura em 16..24).
    assert_eq!(&png[..4], &[0x89, b'P', b'N', b'G']);
    assert_eq!(
        u32::from_be_bytes(png[16..20].try_into().unwrap()),
        u32::from(LARGURA)
    );
    assert_eq!(
        u32::from_be_bytes(png[20..24].try_into().unwrap()),
        u32::from(ALTURA)
    );

    // O primeiro quadro ativou a sessão.
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(handle.manager.get_session(&id).unwrap().state, "active");
    assert_eq!(conexoes.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn vnc_fora_da_lista_nunca_e_conectado() {
    let conexoes = Arc::new(AtomicUsize::new(0));
    let alvo = servidor_vnc(conexoes.clone()).await;
    let relay = WebRtcRelayState::new();
    // Lista vazia (o padrão do gateway): o endpoint fica só guardado.
    let handle = ComputerViewHandle::new().with_relay(relay);
    let app = create_computer_view_router(handle.clone());
    let id = criar(
        &app,
        &format!(r#"{{"bot_id":"b","vnc_endpoint":"vnc://{alvo}"}}"#),
    )
    .await;
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert_eq!(
        conexoes.load(Ordering::SeqCst),
        0,
        "o gateway não pode conectar em endereço que o operador não listou"
    );
    assert_eq!(handle.manager.get_session(&id).unwrap().state, "connecting");
}

#[tokio::test]
async fn servidor_vnc_que_cai_vira_aviso_e_sessao_em_erro() {
    // Porta onde ninguém escuta.
    let alvo = {
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        format!("127.0.0.1:{}", l.local_addr().unwrap().port())
    };
    let relay = WebRtcRelayState::new();
    let handle = ComputerViewHandle::new()
        .with_vnc_permitidos(vec![alvo.clone()])
        .with_relay(relay.clone());
    let app = create_computer_view_router(handle.clone());
    let id = criar(
        &app,
        &format!(r#"{{"bot_id":"b","vnc_endpoint":"vnc://{alvo}"}}"#),
    )
    .await;
    let mut rx = relay.get_or_create_channel(&id).unwrap().subscribe();
    let mut estado = String::new();
    for _ in 0..80 {
        estado = handle.manager.get_session(&id).unwrap().state;
        if estado == "error" {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(estado, "error");
    // O aviso (se o espectador já estava ligado) é uma categoria, nunca o endereço.
    if let Ok(RelayServerMsg::Error { message }) = rx.try_recv() {
        assert!(message.starts_with("VNC:"), "{message}");
        assert!(
            !message.contains(&alvo),
            "o aviso não pode trazer o endereço"
        );
    }
}
