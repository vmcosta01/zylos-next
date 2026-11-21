//! Computer View API — endpoints HTTP para o crate `zylos-computer-view`.
//!
//! Expõe sessões de desktop/browser remoto via REST.
//!
//! ## Endpoints
//! - `GET /computer-view/sessions` — lista sessões ativas
//! - `GET /computer-view/sessions/:id` — detalhes de uma sessão
//! - `POST /computer-view/sessions` — cria nova sessão
//! - `DELETE /computer-view/sessions/:id` — encerra sessão
//! - `POST /computer-view/sessions/:id/control` — solicita controle (lease)
//! - `DELETE /computer-view/sessions/:id/control` — libera controle
//! - `POST /computer-view/sessions/:id/frame` — o agente empurra um frame (PNG/JPEG) para quem assiste
//!
//! ## Related Documents
//! - Crate: `crates/zylos-computer-view/`
//! - Playbook: `playbooks/pb-computer-view-setup.md`
//! - Runbook: `runbooks/rb-zylos-computer-view.md`

use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use zylos_computer_view::{
    schema::{CreateSessionRequest, SessionState, TakeControlRequest},
    session::ComputerSessionManager,
};

/// Handle Send+Sync-safe para o session manager.
#[derive(Clone)]
pub struct ComputerViewHandle {
    pub manager: Arc<ComputerSessionManager>,
    /// Servidores VNC (`host:porta`) a que o gateway pode se conectar para uma sessão com
    /// `vnc_endpoint`. Vazio (padrão) = o gateway nunca abre conexão VNC: o endpoint fica só guardado.
    pub vnc_permitidos: Arc<Vec<String>>,
    /// Espera antes da 1ª nova tentativa quando um VNC que já funcionava cai (dobra a cada
    /// tentativa, até 30 s). Existe como campo para os testes não esperarem segundos.
    pub espera_reconexao: std::time::Duration,
    /// Relay por onde os frames chegam ao cockpit; sem ele a rota de frames responde 503.
    pub relay: Option<crate::webrtc_relay::WebRtcRelayState>,
}

// Safety: ComputerSessionManager wraps a DashMap which is Sync.
unsafe impl Send for ComputerViewHandle {}
unsafe impl Sync for ComputerViewHandle {}

impl ComputerViewHandle {
    pub fn new() -> Self {
        Self {
            manager: Arc::new(ComputerSessionManager::new(30)), // 30 min default lease
            vnc_permitidos: Arc::new(Vec::new()),
            espera_reconexao: std::time::Duration::from_secs(1),
            relay: None,
        }
    }

    /// Servidores VNC permitidos (`host:porta`, sem diferenciar maiúsculas).
    pub fn with_vnc_permitidos(mut self, hosts: Vec<String>) -> Self {
        self.vnc_permitidos = Arc::new(hosts);
        self
    }

    /// Espera inicial entre tentativas de reconexão do VNC.
    pub fn with_espera_reconexao(mut self, espera: std::time::Duration) -> Self {
        self.espera_reconexao = espera;
        self
    }

    /// Liga o relay (o mesmo estado do `webrtc_relay`) para a rota de frames funcionar.
    pub fn with_relay(mut self, relay: crate::webrtc_relay::WebRtcRelayState) -> Self {
        self.relay = Some(relay);
        self
    }
}

/// Variável que liga a captura da tela LOCAL do gateway (padrão: desligada).
pub const CAPTURA_LOCAL_ENV: &str = "ZYLOS_COMPUTER_VIEW_CAPTURE";
/// Intervalo da captura local, em ms (padrão 1500, entre 500 e 10000).
pub const CAPTURA_LOCAL_MS_ENV: &str = "ZYLOS_COMPUTER_VIEW_CAPTURE_MS";

/// A captura local só liga com `ZYLOS_COMPUTER_VIEW_CAPTURE=1`: ela fotografa o monitor
/// primário da máquina onde o gateway roda e manda o quadro a quem assiste a sessão.
pub fn captura_local_ligada() -> bool {
    std::env::var(CAPTURA_LOCAL_ENV).is_ok_and(|v| v.trim() == "1")
}

/// Intervalo entre capturas, com o limite de 500 ms a 10 s.
pub fn intervalo_captura() -> std::time::Duration {
    let ms = std::env::var(CAPTURA_LOCAL_MS_ENV)
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .unwrap_or(1500)
        .clamp(500, 10_000);
    std::time::Duration::from_millis(ms)
}

/// A sessão nasce `connecting`; quando o primeiro frame é entregue ela passa a `active`. Sem isso
/// nada nunca ativava a sessão, e "Assumir controle" (que exige sessão ativa) respondia 409 sempre.
/// Só mexe em `connecting`: não desfaz `user_control`, `paused` nem `terminated`.
fn ativar_se_conectando(handle: &ComputerViewHandle, id: &str) {
    let conectando = handle
        .manager
        .get_session(id)
        .is_some_and(|s| s.state == SessionState::Connecting.as_str());
    if conectando {
        let _ = handle.manager.update_state(id, SessionState::Active);
    }
}

/// Uma rodada da captura local: para cada sessão **local** (sem `vnc_url`: uma sessão que
/// aponta para outro computador não é fotografada aqui) e ativa que tenha pelo menos um
/// espectador, captura o quadro e o publica no relay. Devolve quantos frames publicou.
/// `capturar` devolve os bytes PNG; falha de captura (sem monitor, sem permissão) só pula a sessão.
pub fn rodada_de_captura<F>(handle: &ComputerViewHandle, capturar: F) -> usize
where
    F: Fn(&str) -> Result<Vec<u8>, String>,
{
    let Some(relay) = handle.relay.as_ref() else {
        return 0;
    };
    let mut publicados = 0;
    for s in handle.manager.list_active_sessions() {
        if s.vnc_url.is_some() || relay.viewers(&s.id) == 0 {
            continue;
        }
        match capturar(&s.id) {
            Ok(png) => {
                if relay.publish_frame(&s.id, &png).is_some() {
                    publicados += 1;
                    ativar_se_conectando(handle, &s.id);
                }
            }
            Err(e) => tracing::debug!(session = %s.id, error = %e, "captura local falhou"),
        }
    }
    publicados
}

/// Liga o laço da captura local (tarefa em segundo plano). Só é chamado quando
/// [`captura_local_ligada`]; a captura em si roda fora do executor async.
pub fn spawn_captura_local(handle: ComputerViewHandle, intervalo: std::time::Duration) {
    tracing::warn!(
        intervalo_ms = intervalo.as_millis() as u64,
        "captura da tela local LIGADA ({CAPTURA_LOCAL_ENV}=1): quem assiste uma sessão local vê o monitor primário deste computador"
    );
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(intervalo).await;
            let h = handle.clone();
            let _ = tokio::task::spawn_blocking(move || {
                rodada_de_captura(&h, |id| {
                    let d = h.manager.capture_screenshot(id)?;
                    base64::engine::general_purpose::STANDARD
                        .decode(d.image_base64)
                        .map_err(|e| e.to_string())
                })
            })
            .await;
        }
    });
}

/// Variável com a lista de servidores VNC permitidos (`host:porta`, separados por vírgula).
pub const VNC_PERMITIDOS_ENV: &str = "ZYLOS_VNC_ALLOW_HOSTS";

/// Lê `ZYLOS_VNC_ALLOW_HOSTS` (padrão: vazia = nenhuma conexão VNC).
pub fn vnc_permitidos_do_ambiente() -> Vec<String> {
    std::env::var(VNC_PERMITIDOS_ENV)
        .unwrap_or_default()
        .split(',')
        .map(|h| h.trim().to_ascii_lowercase())
        .filter(|h| !h.is_empty())
        .collect()
}

/// `host:porta` de um `vnc_endpoint` (`vnc://host`, `vnc://host:porta`), se estiver na lista de
/// permitidos. A URL vem do cliente que cria a sessão, então **só se conecta a quem o operador
/// listou**: sem isso este seria um jeito de fazer o gateway conectar em qualquer endereço (SSRF).
pub fn alvo_vnc(endpoint: &str, permitidos: &[String]) -> Option<String> {
    let resto = endpoint.strip_prefix("vnc://")?;
    let resto = resto.trim_end_matches('/');
    if resto.is_empty() || resto.contains(['/', '@', '?', '#', ' ']) {
        return None;
    }
    let hp = if resto.contains(':') {
        resto.to_ascii_lowercase()
    } else {
        format!("{}:5900", resto.to_ascii_lowercase())
    };
    permitidos.contains(&hp).then_some(hp)
}

/// Intervalo mínimo entre quadros publicados (o VNC pode mandar muito mais rápido que o cockpit
/// precisa).
const INTERVALO_QUADRO: std::time::Duration = std::time::Duration::from_millis(500);

/// Quantas vezes seguidas o gateway tenta reconectar um VNC que já tinha funcionado.
const TENTATIVAS_RECONEXAO: u32 = 5;
/// Teto da espera entre tentativas.
const ESPERA_MAXIMA: std::time::Duration = std::time::Duration::from_secs(30);

/// Como terminou uma conexão VNC.
enum FimDaConexao {
    /// A sessão foi encerrada (ou sumiu): não há o que reconectar.
    SessaoAcabou,
    /// Falhou; `houve_quadro` diz se esta conexão chegou a entregar algum quadro.
    Falha { motivo: String, houve_quadro: bool },
}

/// Uma conexão VNC: conecta, lê quadros e os publica até a sessão acabar ou algo falhar.
async fn uma_conexao_vnc(
    handle: &ComputerViewHandle,
    relay: &crate::webrtc_relay::WebRtcRelayState,
    id: &str,
    alvo: &str,
) -> FimDaConexao {
    use zylos_computer_view::rfb::ClienteRfb;
    let falha = |motivo: String, houve_quadro: bool| FimDaConexao::Falha {
        motivo,
        houve_quadro,
    };
    let tcp = match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        tokio::net::TcpStream::connect(alvo),
    )
    .await
    {
        Ok(Ok(t)) => t,
        Ok(Err(e)) => return falha(format!("não conectou ({})", e.kind()), false),
        Err(_) => return falha("tempo esgotado ao conectar".into(), false),
    };
    let mut cli = match ClienteRfb::conectar(tcp).await {
        Ok(c) => c,
        Err(e) => return falha(e.to_string(), false),
    };
    let mut houve_quadro = false;
    loop {
        // A sessão foi encerrada (ou sumiu): para de ler.
        match handle.manager.get_session(id) {
            Some(s) if s.state != SessionState::Terminated.as_str() => {}
            _ => return FimDaConexao::SessaoAcabou,
        }
        let inicio = std::time::Instant::now();
        let quadro = match cli.proximo_quadro().await {
            Ok(q) => q,
            Err(e) => return falha(e.to_string(), houve_quadro),
        };
        let png = match tokio::task::spawn_blocking(move || quadro.png()).await {
            Ok(Ok(p)) => p,
            _ => return falha("falha ao codificar o quadro".into(), houve_quadro),
        };
        if relay.publish_frame(id, &png).is_some() {
            ativar_se_conectando(handle, id);
        }
        houve_quadro = true;
        if let Some(resta) = INTERVALO_QUADRO.checked_sub(inicio.elapsed()) {
            tokio::time::sleep(resta).await;
        }
    }
}

/// Lê o framebuffer de um servidor VNC e publica os quadros no relay até a sessão acabar.
///
/// - Falha **antes de qualquer quadro** (endereço errado, servidor que recusa): a sessão vai para
///   `error` na hora, porque é problema de configuração e tentar de novo não resolve.
/// - Falha **depois de o VNC já ter funcionado** (servidor reiniciou, rede caiu): reconecta até
///   `TENTATIVAS_RECONEXAO` vezes com espera crescente, avisando o cockpit a cada tentativa; só se
///   todas falharem a sessão vai para `error`. Uma conexão que volta a entregar quadros zera a conta.
async fn rfb_para_o_relay(handle: ComputerViewHandle, id: String, alvo: String) {
    let Some(relay) = handle.relay.clone() else {
        return;
    };
    let avisa = |texto: String| {
        if let Some(tx) = relay.get_or_create_channel(&id) {
            let _ = tx.send(crate::webrtc_relay::RelayServerMsg::Error { message: texto });
        }
    };
    let falhou = |motivo: String| {
        tracing::warn!(session = %id, motivo = %motivo, "sessão VNC encerrada por erro");
        avisa(format!("VNC: {motivo}"));
        let _ = handle.manager.update_state(&id, SessionState::Error);
    };
    let mut ja_funcionou = false;
    let mut tentativas = 0u32;
    loop {
        match uma_conexao_vnc(&handle, &relay, &id, &alvo).await {
            FimDaConexao::SessaoAcabou => return,
            FimDaConexao::Falha {
                motivo,
                houve_quadro,
            } => {
                if houve_quadro {
                    ja_funcionou = true;
                    tentativas = 0; // voltou a funcionar: a contagem recomeça
                }
                if !ja_funcionou {
                    return falhou(motivo);
                }
                tentativas += 1;
                if tentativas > TENTATIVAS_RECONEXAO {
                    return falhou(format!(
                        "{motivo} (desisti após {TENTATIVAS_RECONEXAO} tentativas)"
                    ));
                }
                avisa(format!(
                    "VNC: conexão perdida ({motivo}); reconectando ({tentativas}/{TENTATIVAS_RECONEXAO})"
                ));
                let espera = handle
                    .espera_reconexao
                    .saturating_mul(1u32 << (tentativas - 1).min(10))
                    .min(ESPERA_MAXIMA);
                tokio::time::sleep(espera).await;
                // A sessão pode ter sido encerrada durante a espera.
                if !handle
                    .manager
                    .get_session(&id)
                    .is_some_and(|s| s.state != SessionState::Terminated.as_str())
                {
                    return;
                }
            }
        }
    }
}

/// Maior frame aceito (PNG/JPEG de uma tela inteira cabe com folga).
const MAX_FRAME_BYTES: usize = 4 * 1024 * 1024;

/// PNG ou JPEG pelos bytes iniciais (o `Content-Type` do cliente não é confiado).
fn parece_imagem(b: &[u8]) -> bool {
    b.starts_with(&[0x89, b'P', b'N', b'G']) || b.starts_with(&[0xFF, 0xD8, 0xFF])
}

/// `POST /computer-view/sessions/:id/frame` — o agente empurra o quadro atual da tela (corpo =
/// bytes de um PNG ou JPEG). O gateway não captura nada: só entrega ao cockpit pelo relay.
async fn push_frame(
    State(handle): State<ComputerViewHandle>,
    Path(id): Path<String>,
    body: Bytes,
) -> Response {
    let Some(relay) = handle.relay.as_ref() else {
        return error_response(StatusCode::SERVICE_UNAVAILABLE, "relay_unavailable", None)
            .into_response();
    };
    match handle.manager.get_session(&id) {
        Some(s) if s.state != SessionState::Terminated.as_str() => {}
        _ => {
            return error_response(
                StatusCode::NOT_FOUND,
                "session_not_found",
                Some(format!("Session '{id}' not found")),
            )
            .into_response()
        }
    }
    if !parece_imagem(&body) {
        return error_response(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "not_an_image",
            Some("o corpo deve ser um PNG ou JPEG".into()),
        )
        .into_response();
    }
    match relay.publish_frame(&id, &body) {
        Some(viewers) => {
            ativar_se_conectando(&handle, &id);
            Json(serde_json::json!({ "viewers": viewers })).into_response()
        }
        None => error_response(StatusCode::SERVICE_UNAVAILABLE, "relay_session_limit", None)
            .into_response(),
    }
}

impl Default for ComputerViewHandle {
    fn default() -> Self {
        Self::new()
    }
}

/// Resposta de erro padronizada.
#[derive(Serialize)]
struct ErrorResponse {
    error: String,
    detail: Option<String>,
}

fn error_response(status: StatusCode, error: &str, detail: Option<String>) -> impl IntoResponse {
    (
        status,
        Json(ErrorResponse {
            error: error.to_string(),
            detail,
        }),
    )
}

/// `GET /computer-view/sessions` — lista todas as sessões ativas.
async fn list_sessions(State(handle): State<ComputerViewHandle>) -> impl IntoResponse {
    let sessions = handle.manager.list_active_sessions();
    Json(serde_json::json!({
        "sessions": sessions,
        "total": sessions.len(),
    }))
}

/// `GET /computer-view/sessions/:id` — detalhes de uma sessão específica.
async fn get_session(
    State(handle): State<ComputerViewHandle>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match handle.manager.get_session(&id) {
        Some(session) => Json(serde_json::json!({ "session": session })).into_response(),
        None => error_response(
            StatusCode::NOT_FOUND,
            "session_not_found",
            Some(format!("Session '{}' not found", id)),
        )
        .into_response(),
    }
}

/// Body para criar uma nova sessão (mapeia para CreateSessionRequest do crate).
#[derive(Deserialize)]
struct CreateSessionBody {
    bot_id: String,
    vnc_endpoint: Option<String>,
    #[serde(default)]
    metadata: serde_json::Value,
}

/// `POST /computer-view/sessions` — cria uma nova sessão de computer view.
async fn create_session(
    State(handle): State<ComputerViewHandle>,
    Json(body): Json<CreateSessionBody>,
) -> impl IntoResponse {
    let req = CreateSessionRequest {
        bot_id: body.bot_id,
        vnc_endpoint: body.vnc_endpoint,
        metadata: Some(body.metadata),
    };
    let vnc = req.vnc_endpoint.clone();
    let session = handle.manager.create_session(req);
    // Sessão com servidor VNC permitido: o gateway passa a ler o framebuffer dele.
    if let Some(alvo) = vnc.and_then(|u| alvo_vnc(&u, &handle.vnc_permitidos)) {
        tokio::spawn(rfb_para_o_relay(handle.clone(), session.id.clone(), alvo));
    }
    (
        StatusCode::CREATED,
        Json(serde_json::json!({ "session": session })),
    )
        .into_response()
}

/// `DELETE /computer-view/sessions/:id` — encerra uma sessão.
async fn terminate_session(
    State(handle): State<ComputerViewHandle>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match handle.manager.update_state(&id, SessionState::Terminated) {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => {
            error_response(StatusCode::NOT_FOUND, "session_not_found", Some(e)).into_response()
        }
    }
}

/// `POST /computer-view/sessions/:id/control` — solicita controle da sessão (lease).
async fn take_control(
    State(handle): State<ComputerViewHandle>,
    Path(id): Path<String>,
    Json(req): Json<TakeControlRequest>,
) -> impl IntoResponse {
    match handle.manager.take_control(&id, req) {
        Ok(session) => (
            StatusCode::OK,
            Json(serde_json::json!({ "session": session })),
        )
            .into_response(),
        Err(e) => {
            error_response(StatusCode::CONFLICT, "control_unavailable", Some(e)).into_response()
        }
    }
}

/// Body para liberar controle.
#[derive(Deserialize)]
struct ReleaseControlBody {
    user_id: String,
}

/// `DELETE /computer-view/sessions/:id/control` — libera o controle da sessão.
async fn release_control(
    State(handle): State<ComputerViewHandle>,
    Path(id): Path<String>,
    Json(body): Json<ReleaseControlBody>,
) -> impl IntoResponse {
    match handle.manager.release_control(&id, &body.user_id) {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => error_response(StatusCode::CONFLICT, "release_failed", Some(e)).into_response(),
    }
}

/// `GET /computer-view/sessions/{id}/screenshot` — captura tela atual como PNG binário.
async fn get_screenshot(
    State(handle): State<ComputerViewHandle>,
    Path(id): Path<String>,
) -> Response {
    match handle.manager.capture_screenshot(&id) {
        Ok(data) => {
            let bytes: Vec<u8> =
                match base64::engine::general_purpose::STANDARD.decode(&data.image_base64) {
                    Ok(b) => b,
                    Err(e) => {
                        return error_response(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "screenshot_decode_failed",
                            Some(e.to_string()),
                        )
                        .into_response();
                    }
                };
            (StatusCode::OK, [(header::CONTENT_TYPE, "image/png")], bytes).into_response()
        }
        Err(e) if e.contains("not found") => {
            error_response(StatusCode::NOT_FOUND, "session_not_found", Some(e)).into_response()
        }
        Err(e) => error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "screenshot_unavailable",
            Some(e),
        )
        .into_response(),
    }
}

/// Cria o router de computer-view com seu próprio handle.
pub fn create_computer_view_router(handle: ComputerViewHandle) -> Router {
    Router::new()
        .route(
            "/computer-view/sessions",
            get(list_sessions).post(create_session),
        )
        .route(
            "/computer-view/sessions/:id",
            get(get_session).delete(terminate_session),
        )
        .route(
            "/computer-view/sessions/:id/control",
            post(take_control).delete(release_control),
        )
        .route(
            "/computer-view/sessions/:id/screenshot",
            get(get_screenshot),
        )
        .route(
            "/computer-view/sessions/:id/frame",
            post(push_frame).layer(DefaultBodyLimit::max(MAX_FRAME_BYTES)),
        )
        .with_state(handle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    fn test_app() -> Router {
        let handle = ComputerViewHandle::new();
        create_computer_view_router(handle)
    }

    #[tokio::test]
    async fn test_list_sessions_empty() {
        let app = test_app();
        let req = Request::builder()
            .uri("/computer-view/sessions")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_get_session_not_found() {
        let app = test_app();
        let req = Request::builder()
            .uri("/computer-view/sessions/nonexistent")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    // Regressão: no axum 0.7 `{id}` é texto literal; as rotas por sessão respondiam sempre 404
    // (o teste "not_found" acima passava por esse motivo errado).
    async fn create(app: &Router) -> String {
        let resp = app
            .clone()
            .oneshot(
                Request::post("/computer-view/sessions")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"bot_id":"backend"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        v["session"]["id"].as_str().unwrap().to_string()
    }

    #[tokio::test]
    async fn rotas_por_sessao_encontram_a_sessao_criada() {
        let app = test_app();
        let id = create(&app).await;

        let get = app
            .clone()
            .oneshot(
                Request::get(format!("/computer-view/sessions/{id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(get.status(), StatusCode::OK);

        let take = app
            .clone()
            .oneshot(
                Request::post(format!("/computer-view/sessions/{id}/control"))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"user_id":"victor"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        // Sessão recém-criada ainda está "connecting": o domínio responde 409, não 404.
        assert_ne!(take.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn screenshot_e_listagem_apontam_para_a_mesma_rota() {
        let app = test_app();
        let id = create(&app).await;
        let list = app
            .clone()
            .oneshot(
                Request::get("/computer-view/sessions")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let bytes = axum::body::to_bytes(list.into_body(), 1 << 20)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let url = v["sessions"][0]["screenshot_url"].as_str().unwrap();
        assert_eq!(
            url,
            format!("/api/v1/computer-view/sessions/{id}/screenshot")
        );
    }
    // PNG mínimo (assinatura + lixo): a rota só confere os bytes iniciais, não decodifica.
    const PNG: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3];

    fn app_com_relay() -> (Router, crate::webrtc_relay::WebRtcRelayState) {
        let relay = crate::webrtc_relay::WebRtcRelayState::new();
        let app = create_computer_view_router(ComputerViewHandle::new().with_relay(relay.clone()));
        (app, relay)
    }

    async fn empurrar(app: &Router, id: &str, corpo: Vec<u8>) -> StatusCode {
        app.clone()
            .oneshot(
                Request::post(format!("/computer-view/sessions/{id}/frame"))
                    .header("content-type", "application/octet-stream")
                    .body(Body::from(corpo))
                    .unwrap(),
            )
            .await
            .unwrap()
            .status()
    }

    #[tokio::test]
    async fn captura_local_so_fotografa_sessao_local_com_espectador() {
        let relay = crate::webrtc_relay::WebRtcRelayState::new();
        let handle = ComputerViewHandle::new().with_relay(relay.clone());
        let nova = |vnc: Option<&str>| {
            handle
                .manager
                .create_session(zylos_computer_view::schema::CreateSessionRequest {
                    bot_id: "b".into(),
                    vnc_endpoint: vnc.map(str::to_string),
                    metadata: None,
                })
                .id
        };
        let local_com = nova(None);
        let local_sem = nova(None);
        let remota = nova(Some("vnc://outro-pc:5900"));
        let chamadas = std::sync::Mutex::new(Vec::<String>::new());
        let capturar = |id: &str| -> Result<Vec<u8>, String> {
            chamadas.lock().unwrap().push(id.to_string());
            Ok(PNG.to_vec())
        };

        // Sem nenhum espectador: não captura nada (nem fotografa à toa).
        assert_eq!(rodada_de_captura(&handle, capturar), 0);
        assert!(chamadas.lock().unwrap().is_empty());

        // Um espectador na sessão local e outro na remota: só a local é fotografada.
        let mut rx = relay.get_or_create_channel(&local_com).unwrap().subscribe();
        let _rx_remota = relay.get_or_create_channel(&remota).unwrap().subscribe();
        assert_eq!(rodada_de_captura(&handle, capturar), 1);
        assert_eq!(*chamadas.lock().unwrap(), vec![local_com.clone()]);
        assert!(rx.try_recv().is_ok(), "o frame chegou ao espectador");
        assert_eq!(relay.viewers(&local_sem), 0);

        // Falha de captura (sem monitor): pula, não publica, não derruba a rodada.
        let falha = |_: &str| -> Result<Vec<u8>, String> { Err("sem monitor".into()) };
        assert_eq!(rodada_de_captura(&handle, falha), 0);
    }

    #[test]
    fn captura_local_vem_desligada_e_o_intervalo_tem_limite() {
        // Sem a variável, desligada (o teste não mexe no ambiente: lê o que houver).
        if std::env::var(CAPTURA_LOCAL_ENV).is_err() {
            assert!(!captura_local_ligada());
        }
        let i = intervalo_captura();
        assert!(
            i >= std::time::Duration::from_millis(500) && i <= std::time::Duration::from_secs(10)
        );
    }

    #[tokio::test]
    async fn primeiro_frame_ativa_a_sessao_e_assumir_controle_passa_a_funcionar() {
        let (app, _relay) = app_com_relay();
        let id = create(&app).await;
        let assumir = |app: &Router, id: &str| {
            let app = app.clone();
            let id = id.to_string();
            async move {
                app.oneshot(
                    Request::post(format!("/computer-view/sessions/{id}/control"))
                        .header("content-type", "application/json")
                        .body(Body::from(r#"{"user_id":"u1"}"#))
                        .unwrap(),
                )
                .await
                .unwrap()
                .status()
            }
        };
        let estado = |app: &Router, id: &str| {
            let app = app.clone();
            let id = id.to_string();
            async move {
                let r = app
                    .oneshot(
                        Request::get(format!("/computer-view/sessions/{id}"))
                            .body(Body::empty())
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                let bytes = axum::body::to_bytes(r.into_body(), 1 << 20).await.unwrap();
                let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
                v["session"]["state"].as_str().unwrap().to_string()
            }
        };

        // Antes de qualquer frame a sessão está `connecting` e não pode ser assumida.
        assert_eq!(estado(&app, &id).await, "connecting");
        assert_eq!(assumir(&app, &id).await, StatusCode::CONFLICT);

        // O primeiro frame a ativa; aí dá para assumir o controle.
        assert_eq!(empurrar(&app, &id, PNG.to_vec()).await, StatusCode::OK);
        assert_eq!(estado(&app, &id).await, "active");
        assert_eq!(assumir(&app, &id).await, StatusCode::OK);
        assert_eq!(estado(&app, &id).await, "user_control");

        // Outro frame não desfaz o controle humano.
        assert_eq!(empurrar(&app, &id, PNG.to_vec()).await, StatusCode::OK);
        assert_eq!(estado(&app, &id).await, "user_control");
    }

    #[test]
    fn so_conecta_em_vnc_que_o_operador_listou() {
        let p = vec!["10.0.0.5:5900".to_string(), "sala.local:5901".to_string()];
        assert_eq!(
            alvo_vnc("vnc://10.0.0.5", &p).as_deref(),
            Some("10.0.0.5:5900")
        );
        assert_eq!(
            alvo_vnc("vnc://10.0.0.5:5900/", &p).as_deref(),
            Some("10.0.0.5:5900")
        );
        assert_eq!(
            alvo_vnc("vnc://SALA.local:5901", &p).as_deref(),
            Some("sala.local:5901")
        );
        // Fora da lista, porta diferente, outro esquema, partes extras: nada.
        assert_eq!(alvo_vnc("vnc://10.0.0.6", &p), None);
        assert_eq!(alvo_vnc("vnc://10.0.0.5:5901", &p), None);
        assert_eq!(alvo_vnc("http://10.0.0.5:5900", &p), None);
        assert_eq!(alvo_vnc("vnc://user@10.0.0.5", &p), None);
        assert_eq!(alvo_vnc("vnc://10.0.0.5:5900/x", &p), None);
        assert_eq!(alvo_vnc("vnc://", &p), None);
        // Lista vazia (padrão): nunca.
        assert_eq!(alvo_vnc("vnc://10.0.0.5", &[]), None);
    }

    #[tokio::test]
    async fn frame_empurrado_chega_ao_espectador_do_relay() {
        use base64::Engine as _;
        let (app, relay) = app_com_relay();
        let id = create(&app).await;
        // Um espectador ligado ao canal da sessão (o que o WebSocket do cockpit faz).
        let mut rx = relay.get_or_create_channel(&id).unwrap().subscribe();

        assert_eq!(empurrar(&app, &id, PNG.to_vec()).await, StatusCode::OK);

        match rx.try_recv().expect("o frame não chegou ao relay") {
            crate::webrtc_relay::RelayServerMsg::Frame { data, .. } => {
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(data)
                    .unwrap();
                assert_eq!(bytes, PNG);
            }
            outro => panic!("mensagem inesperada: {outro:?}"),
        }
    }

    #[tokio::test]
    async fn frame_recusa_o_que_nao_e_imagem_e_sessao_inexistente() {
        let (app, _relay) = app_com_relay();
        let id = create(&app).await;
        assert_eq!(
            empurrar(&app, &id, b"<html>nao sou imagem</html>".to_vec()).await,
            StatusCode::UNSUPPORTED_MEDIA_TYPE
        );
        assert_eq!(
            empurrar(&app, "nao-existe", PNG.to_vec()).await,
            StatusCode::NOT_FOUND
        );
        // Sessão encerrada também deixa de aceitar frames.
        let del = app
            .clone()
            .oneshot(
                Request::delete(format!("/computer-view/sessions/{id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(del.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            empurrar(&app, &id, PNG.to_vec()).await,
            StatusCode::NOT_FOUND
        );
    }

    #[tokio::test]
    async fn frame_passa_do_limite_e_relay_desligado() {
        let (app, _relay) = app_com_relay();
        let id = create(&app).await;
        let mut grande = PNG.to_vec();
        grande.resize(MAX_FRAME_BYTES + 1, 0);
        assert_eq!(
            empurrar(&app, &id, grande).await,
            StatusCode::PAYLOAD_TOO_LARGE
        );
        // Sem relay ligado (ComputerViewHandle::new() puro), a rota diz que não há para onde mandar.
        let sem = test_app();
        let id2 = create(&sem).await;
        assert_eq!(
            empurrar(&sem, &id2, PNG.to_vec()).await,
            StatusCode::SERVICE_UNAVAILABLE
        );
    }
}
