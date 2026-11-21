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
    servidor_vnc_com(conexoes, false, None, false).await
}

/// Com `ignora_incremental`, só responde a pedido de quadro inteiro (como um servidor real com a
/// tela parada: não responde a um pedido incremental enquanto nada muda).
///
/// `quadros_por_conexao`: depois de mandar esse tanto de quadros, derruba a conexão (servidor que
/// reinicia). `so_uma_conexao`: depois da 1ª conexão fecha o listener (o servidor "morre").
async fn servidor_vnc_com(
    conexoes: Arc<AtomicUsize>,
    ignora_incremental: bool,
    quadros_por_conexao: Option<usize>,
    so_uma_conexao: bool,
) -> String {
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
                let mut enviados = 0usize;
                loop {
                    let mut req = [0u8; 10];
                    if s.read_exact(&mut req).await.is_err() {
                        return;
                    }
                    if ignora_incremental && req[1] != 0 {
                        continue;
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
                    enviados += 1;
                    if quadros_por_conexao.is_some_and(|n| enviados >= n) {
                        return;
                    }
                }
            });
            if so_uma_conexao {
                return; // o listener é descartado: novas conexões são recusadas
            }
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
async fn tela_parada_nao_derruba_a_sessao() {
    // O servidor só responde a pedido de quadro inteiro. O cliente tem de continuar recebendo
    // quadros bem além do limite de leitura de 10 s do pedido incremental.
    let conexoes = Arc::new(AtomicUsize::new(0));
    let alvo = servidor_vnc_com(conexoes, true, None, false).await;
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
    let mut quadros = 0;
    while quadros < 3 {
        match tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("o servidor parou de receber quadros")
            .unwrap()
        {
            RelayServerMsg::Frame { .. } => quadros += 1,
            RelayServerMsg::Error { message } => panic!("erro do relay: {message}"),
            _ => {}
        }
    }
    assert_eq!(handle.manager.get_session(&id).unwrap().state, "active");
}

#[tokio::test]
async fn vnc_que_reinicia_e_reconectado_e_a_sessao_continua_ativa() {
    // O servidor derruba a conexão a cada 2 quadros. Como o VNC já tinha funcionado, o gateway
    // reconecta (com espera curta no teste) e os quadros continuam chegando; a sessão não vira erro.
    let conexoes = Arc::new(AtomicUsize::new(0));
    let alvo = servidor_vnc_com(conexoes.clone(), false, Some(2), false).await;
    let relay = WebRtcRelayState::new();
    let handle = ComputerViewHandle::new()
        .with_vnc_permitidos(vec![alvo.clone()])
        .with_espera_reconexao(Duration::from_millis(20))
        .with_relay(relay.clone());
    let app = create_computer_view_router(handle.clone());
    let id = criar(
        &app,
        &format!(r#"{{"bot_id":"b","vnc_endpoint":"vnc://{alvo}"}}"#),
    )
    .await;
    let mut rx = relay.get_or_create_channel(&id).unwrap().subscribe();
    let (mut quadros, mut avisos_de_reconexao) = (0, 0);
    // 14 quadros = 7 conexões de 2 quadros: mais que as 5 tentativas seguidas permitidas, então só
    // passa se a contagem de tentativas zera quando uma conexão volta a entregar quadros.
    while quadros < 14 {
        match tokio::time::timeout(Duration::from_secs(8), rx.recv())
            .await
            .expect("os quadros pararam de chegar")
            .unwrap()
        {
            RelayServerMsg::Frame { .. } => quadros += 1,
            RelayServerMsg::Error { message } => {
                assert!(message.contains("reconectando"), "{message}");
                assert!(
                    !message.contains(&alvo),
                    "o aviso não pode trazer o endereço"
                );
                avisos_de_reconexao += 1;
            }
            _ => {}
        }
    }
    assert!(
        conexoes.load(Ordering::SeqCst) >= 7,
        "reconectou ao menos 6 vezes"
    );
    assert!(avisos_de_reconexao >= 6);
    assert_eq!(handle.manager.get_session(&id).unwrap().state, "active");
}

#[tokio::test]
async fn vnc_que_morre_de_vez_desiste_apos_as_tentativas_e_a_sessao_vira_erro() {
    let conexoes = Arc::new(AtomicUsize::new(0));
    let alvo = servidor_vnc_com(conexoes.clone(), false, Some(1), true).await;
    let relay = WebRtcRelayState::new();
    let handle = ComputerViewHandle::new()
        .with_vnc_permitidos(vec![alvo.clone()])
        .with_espera_reconexao(Duration::from_millis(10))
        .with_relay(relay.clone());
    let app = create_computer_view_router(handle.clone());
    let id = criar(
        &app,
        &format!(r#"{{"bot_id":"b","vnc_endpoint":"vnc://{alvo}"}}"#),
    )
    .await;
    let mut rx = relay.get_or_create_channel(&id).unwrap().subscribe();
    let mut ultimo_aviso = String::new();
    let mut estado = String::new();
    // No Windows, conectar numa porta fechada leva ~2 s por tentativa (5 tentativas).
    for _ in 0..300 {
        while let Ok(m) = rx.try_recv() {
            if let RelayServerMsg::Error { message } = m {
                ultimo_aviso = message;
            }
        }
        estado = handle.manager.get_session(&id).unwrap().state;
        if estado == "error" {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(estado, "error", "último aviso: {ultimo_aviso}");
    while let Ok(m) = rx.try_recv() {
        if let RelayServerMsg::Error { message } = m {
            ultimo_aviso = message;
        }
    }
    assert!(ultimo_aviso.contains("desisti"), "{ultimo_aviso}");
    assert!(!ultimo_aviso.contains(&alvo));
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
    // O aviso é publicado antes de a sessão virar `error`; o espectador já estava inscrito.
    match rx.try_recv() {
        Ok(RelayServerMsg::Error { message }) => {
            assert!(message.starts_with("VNC:"), "{message}");
            assert!(
                !message.contains(&alvo),
                "o aviso não pode trazer o endereço"
            );
        }
        outro => panic!("esperava o aviso de erro do VNC, veio {outro:?}"),
    }
}
