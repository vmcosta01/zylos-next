//! `POST /api/v1/bots/:nome/perguntar` — o bot pensa sozinho (zylos-pensar).
//!
//! Sem token, por decisão do Victor (2026-11-09); fica atrás da trava
//! só-desta-máquina (`local_guard`), como as outras rotas de ação.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use zylos_pensar::{Config, ErroPensar};

#[derive(Clone)]
pub struct BotsState {
    pub cfg: Arc<Config>,
    pub cliente: reqwest::Client,
    pub egress: zylos_agent_shield::network_egress::NetworkEgressFilter,
    /// Barramento SSE: quando o bot falha por culpa do provedor/chave/rede, o cockpit recebe
    /// um `AgentStateChanged` com `new_state = "error"`.
    pub bus: Option<zylos_live_comm::EventBus>,
    /// Quantas perguntas cada bot está respondendo agora. Só em memória: some quando o gateway
    /// reinicia, e o bot volta a `ocioso`.
    pub ativos: Arc<Mutex<HashMap<String, usize>>>,
}

/// Marca o bot como `trabalhando` enquanto vive; ao sair do escopo (resposta, erro ou pedido
/// cancelado pelo cliente) desmarca. Por ser `Drop`, o cancelamento não deixa o bot preso.
struct EmTrabalho {
    ativos: Arc<Mutex<HashMap<String, usize>>>,
    nome: String,
}

impl EmTrabalho {
    fn novo(ativos: &Arc<Mutex<HashMap<String, usize>>>, nome: &str) -> Self {
        if let Ok(mut m) = ativos.lock() {
            *m.entry(nome.to_string()).or_insert(0) += 1;
        }
        Self {
            ativos: Arc::clone(ativos),
            nome: nome.to_string(),
        }
    }
}

impl Drop for EmTrabalho {
    fn drop(&mut self) {
        if let Ok(mut m) = self.ativos.lock() {
            if let Some(n) = m.get_mut(&self.nome) {
                *n = n.saturating_sub(1);
                if *n == 0 {
                    m.remove(&self.nome);
                }
            }
        }
    }
}

/// Um bot da lista com o que ele está fazendo agora (`trabalhando` ou `ocioso`).
#[derive(Debug, Serialize)]
struct BotComEstado {
    #[serde(flatten)]
    info: zylos_pensar::bots::InfoBot,
    estado: &'static str,
}

#[derive(Debug, Deserialize)]
pub struct PedidoPergunta {
    pub pergunta: String,
}

/// Roteador já com estado próprio (vira `Router<()>` para o `.merge`).
pub fn create_bots_router(cfg: Config) -> Router {
    create_bots_router_with_bus(cfg, None)
}

/// Como [`create_bots_router`], publicando falhas do bot no barramento de eventos.
pub fn create_bots_router_with_bus(cfg: Config, bus: Option<zylos_live_comm::EventBus>) -> Router {
    use zylos_agent_shield::network_egress::{EgressConfig, EgressRule, NetworkEgressFilter};
    let mut rules = std::collections::HashMap::new();
    rules.insert(
        "zylos-pensar".to_string(),
        EgressRule {
            allowed_hosts: vec![
                "api.anthropic.com".into(),
                "api.openai.com".into(),
                "integrate.api.nvidia.com".into(),
                "*.anthropic.com".into(),
                "*.openai.com".into(),
                "*.nvidia.com".into(),
            ],
            allowed_ports: vec![443],
            deny_all: false,
        },
    );
    let egress = NetworkEgressFilter::new(EgressConfig {
        egress_rules: rules,
        log_blocked: true,
        allow_dns_resolution: true,
    });
    Router::new()
        .route("/bots", get(get_bots))
        .route("/bots/:nome/perguntar", post(post_perguntar))
        .route("/provedores", get(get_provedores))
        .route("/provedores/:nome/modelos", get(get_modelos))
        .with_state(BotsState {
            cfg: Arc::new(cfg),
            cliente: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            egress,
            bus,
            ativos: Arc::new(Mutex::new(HashMap::new())),
        })
}

/// Status HTTP e tipo de problema de cada erro.
pub fn status_e_tipo(e: &ErroPensar) -> (StatusCode, &'static str, &'static str) {
    use ErroPensar::*;
    match e {
        NomeInvalido => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "nome-de-bot-invalido",
            "Nome de bot inválido",
        ),
        PerguntaVazia | PerguntaGrande => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "pergunta-invalida",
            "Pergunta inválida",
        ),
        BotInexistente(_) => (StatusCode::NOT_FOUND, "bot-inexistente", "Bot não existe"),
        BotMalFeito { .. } => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "bot-mal-configurado",
            "Bot mal configurado",
        ),
        Catalogo(_) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "fichas-de-provedor-invalidas",
            "Fichas de provedor com problema",
        ),
        ProvedorDesconhecido { .. } => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "provedor-desconhecido",
            "Provedor desconhecido",
        ),
        ProvedorNaoExiste { .. } => (
            StatusCode::NOT_FOUND,
            "provedor-inexistente",
            "Provedor não existe",
        ),
        SemModelo(_) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "bot-sem-modelo",
            "Bot sem modelo",
        ),
        ChaveFaltando { .. } => (
            StatusCode::FAILED_DEPENDENCY,
            "chave-faltando",
            "Chave de API faltando",
        ),
        ChaveInvalida { .. } => (
            StatusCode::FAILED_DEPENDENCY,
            "chave-invalida",
            "Chave de API não pode ser usada",
        ),
        ProvedorRecusou { .. } => (
            StatusCode::BAD_GATEWAY,
            "provedor-recusou",
            "O provedor recusou",
        ),
        Rede(_) => (
            StatusCode::BAD_GATEWAY,
            "provedor-inalcancavel",
            "Falha de rede com o provedor",
        ),
        RespostaEstranha(_) => (
            StatusCode::BAD_GATEWAY,
            "resposta-inesperada",
            "Resposta inesperada do provedor",
        ),
        TempoEsgotado(_) => (
            StatusCode::GATEWAY_TIMEOUT,
            "tempo-esgotado",
            "O provedor não respondeu a tempo",
        ),
    }
}

fn problema(e: &ErroPensar, instancia: String) -> Response {
    let (status, tipo, titulo) = status_e_tipo(e);
    let corpo = serde_json::json!({
        "type": format!("https://zylos.dev/errors/{tipo}"),
        "title": titulo,
        "status": status.as_u16(),
        "detail": e.to_string(),
        "instance": instancia,
    });
    (
        status,
        [(header::CONTENT_TYPE, "application/problem+json")],
        Json(corpo),
    )
        .into_response()
}

fn ambiente(n: &str) -> Option<String> {
    std::env::var(n).ok()
}

async fn post_perguntar(
    State(st): State<BotsState>,
    Path(nome): Path<String>,
    Json(pedido): Json<PedidoPergunta>,
) -> Response {
    let _em_trabalho = EmTrabalho::novo(&st.ativos, &nome);
    match zylos_pensar::perguntar(&st.cfg, &st.cliente, &ambiente, &nome, &pedido.pergunta).await {
        Ok(r) => Json(r).into_response(),
        Err(e) => {
            avisa_falha(&st, &nome, &e);
            problema(&e, format!("/api/v1/bots/{nome}/perguntar"))
        }
    }
}

/// Erro de entrada do usuário (nome, pergunta, bot ou provedor inexistente/mal configurado)?
/// Esses não são falha do agente: só chave, provedor e rede viram aviso.
fn erro_de_entrada(status: StatusCode) -> bool {
    status == StatusCode::NOT_FOUND || status == StatusCode::UNPROCESSABLE_ENTITY
}

/// Publica `AgentStateChanged` (`running` → `error`) para o toast do cockpit.
/// O motivo é o título curto do problema, nunca o detalhe (pode citar chave ou URL).
fn avisa_falha(st: &BotsState, nome: &str, e: &ErroPensar) {
    let (status, _, titulo) = status_e_tipo(e);
    if erro_de_entrada(status) {
        return;
    }
    let Some(bus) = st.bus.clone() else { return };
    let evento = zylos_live_comm::ZylosEvent::AgentStateChanged(
        zylos_live_comm::event_types::AgentStateChanged {
            agent_id: nome.to_string(),
            previous_state: "running".to_string(),
            new_state: "error".to_string(),
            reason: Some(titulo.to_string()),
        },
    );
    tokio::spawn(async move {
        if let Err(err) = bus.publish("agent.state", evento, 1).await {
            tracing::warn!(error = %err, "falha ao publicar AgentStateChanged");
        }
    });
}

/// `GET /api/v1/bots` — os bots da pasta de bots (nome, provedor, modelo, estado; nunca a `ALMA.md`).
async fn get_bots(State(st): State<BotsState>) -> Response {
    let pasta = st.cfg.pasta_bots.clone();
    // Lê o disco fora do executor async.
    match tokio::task::spawn_blocking(move || zylos_pensar::bots::listar(&pasta)).await {
        Ok(bots) => {
            let ativos = st.ativos.lock().map(|m| m.clone()).unwrap_or_default();
            let bots: Vec<BotComEstado> = bots
                .into_iter()
                .map(|info| {
                    let estado = if ativos.contains_key(&info.nome) {
                        "trabalhando"
                    } else {
                        "ocioso"
                    };
                    BotComEstado { info, estado }
                })
                .collect();
            Json(serde_json::json!({ "bots": bots })).into_response()
        }
        Err(_) => problema(
            &ErroPensar::Rede("leitura da pasta de bots falhou".into()),
            "/api/v1/bots".into(),
        ),
    }
}

/// `GET /api/v1/provedores` — fichas + se a chave de cada uma está pronta (nunca o valor).
async fn get_provedores(State(st): State<BotsState>) -> Response {
    match zylos_pensar::listar_provedores(&st.cfg, &ambiente) {
        Ok(l) => Json(serde_json::json!({ "provedores": l })).into_response(),
        Err(e) => problema(&e, "/api/v1/provedores".into()),
    }
}

/// `GET /api/v1/provedores/:nome/modelos` — pergunta ao provedor quais modelos ele oferece.
async fn get_modelos(State(st): State<BotsState>, Path(nome): Path<String>) -> Response {
    match zylos_pensar::listar_modelos(&st.cfg, &st.cliente, &ambiente, &nome).await {
        Ok(m) => Json(serde_json::json!({ "provedor": nome, "modelos": m })).into_response(),
        Err(e) => problema(&e, format!("/api/v1/provedores/{nome}/modelos")),
    }
}
