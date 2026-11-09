//! `POST /api/v1/bots/:nome/perguntar` — o bot pensa sozinho (zylos-pensar).
//!
//! Sem token, por decisão do Victor (2026-11-09); fica atrás da trava
//! só-desta-máquina (`local_guard`), como as outras rotas de ação.

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use zylos_pensar::{Config, ErroPensar};

#[derive(Clone)]
pub struct BotsState {
    pub cfg: Arc<Config>,
    pub cliente: reqwest::Client,
}

#[derive(Debug, Deserialize)]
pub struct PedidoPergunta {
    pub pergunta: String,
}

/// Roteador já com estado próprio (vira `Router<()>` para o `.merge`).
pub fn create_bots_router(cfg: Config) -> Router {
    Router::new()
        .route("/bots/:nome/perguntar", post(post_perguntar))
        .route("/provedores", get(get_provedores))
        .route("/provedores/:nome/modelos", get(get_modelos))
        .with_state(BotsState { cfg: Arc::new(cfg), cliente: reqwest::Client::new() })
}

/// Status HTTP e tipo de problema de cada erro.
pub fn status_e_tipo(e: &ErroPensar) -> (StatusCode, &'static str, &'static str) {
    use ErroPensar::*;
    match e {
        NomeInvalido => (StatusCode::UNPROCESSABLE_ENTITY, "nome-de-bot-invalido", "Nome de bot inválido"),
        PerguntaVazia | PerguntaGrande => (StatusCode::UNPROCESSABLE_ENTITY, "pergunta-invalida", "Pergunta inválida"),
        BotInexistente(_) => (StatusCode::NOT_FOUND, "bot-inexistente", "Bot não existe"),
        BotMalFeito { .. } => (StatusCode::UNPROCESSABLE_ENTITY, "bot-mal-configurado", "Bot mal configurado"),
        Catalogo(_) => (StatusCode::UNPROCESSABLE_ENTITY, "fichas-de-provedor-invalidas", "Fichas de provedor com problema"),
        ProvedorDesconhecido { .. } => (StatusCode::UNPROCESSABLE_ENTITY, "provedor-desconhecido", "Provedor desconhecido"),
        ProvedorNaoExiste { .. } => (StatusCode::NOT_FOUND, "provedor-inexistente", "Provedor não existe"),
        SemModelo(_) => (StatusCode::UNPROCESSABLE_ENTITY, "bot-sem-modelo", "Bot sem modelo"),
        ChaveFaltando { .. } => (StatusCode::FAILED_DEPENDENCY, "chave-faltando", "Chave de API faltando"),
        ChaveInvalida { .. } => (StatusCode::FAILED_DEPENDENCY, "chave-invalida", "Chave de API não pode ser usada"),
        ProvedorRecusou { .. } => (StatusCode::BAD_GATEWAY, "provedor-recusou", "O provedor recusou"),
        Rede(_) => (StatusCode::BAD_GATEWAY, "provedor-inalcancavel", "Falha de rede com o provedor"),
        RespostaEstranha(_) => (StatusCode::BAD_GATEWAY, "resposta-inesperada", "Resposta inesperada do provedor"),
        TempoEsgotado(_) => (StatusCode::GATEWAY_TIMEOUT, "tempo-esgotado", "O provedor não respondeu a tempo"),
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
    (status, [(header::CONTENT_TYPE, "application/problem+json")], Json(corpo)).into_response()
}

fn ambiente(n: &str) -> Option<String> {
    std::env::var(n).ok()
}

async fn post_perguntar(
    State(st): State<BotsState>,
    Path(nome): Path<String>,
    Json(pedido): Json<PedidoPergunta>,
) -> Response {
    match zylos_pensar::perguntar(&st.cfg, &st.cliente, &ambiente, &nome, &pedido.pergunta).await {
        Ok(r) => Json(r).into_response(),
        Err(e) => problema(&e, format!("/api/v1/bots/{nome}/perguntar")),
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
