//! `POST /api/v1/bots/:nome/perguntar` — o bot pensa sozinho (zylos-pensar).
//!
//! Sem token, por decisão do Victor (2026-11-09); fica atrás da trava
//! só-desta-máquina (`local_guard`), como as outras rotas de ação.

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
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
        SemModelo(_) => (StatusCode::UNPROCESSABLE_ENTITY, "bot-sem-modelo", "Bot sem modelo"),
        ChaveFaltando { .. } => (StatusCode::FAILED_DEPENDENCY, "chave-faltando", "Chave de API faltando"),
        ChaveInvalida { .. } => (StatusCode::FAILED_DEPENDENCY, "chave-invalida", "Chave de API não pode ser usada"),
        ProvedorRecusou { .. } => (StatusCode::BAD_GATEWAY, "provedor-recusou", "O provedor recusou"),
        Rede(_) => (StatusCode::BAD_GATEWAY, "provedor-inalcancavel", "Falha de rede com o provedor"),
        RespostaEstranha(_) => (StatusCode::BAD_GATEWAY, "resposta-inesperada", "Resposta inesperada do provedor"),
        TempoEsgotado(_) => (StatusCode::GATEWAY_TIMEOUT, "tempo-esgotado", "O provedor não respondeu a tempo"),
    }
}

async fn post_perguntar(
    State(st): State<BotsState>,
    Path(nome): Path<String>,
    Json(pedido): Json<PedidoPergunta>,
) -> Response {
    let ambiente = |n: &str| std::env::var(n).ok();
    match zylos_pensar::perguntar(&st.cfg, &st.cliente, &ambiente, &nome, &pedido.pergunta).await {
        Ok(r) => Json(r).into_response(),
        Err(e) => {
            let (status, tipo, titulo) = status_e_tipo(&e);
            let corpo = serde_json::json!({
                "type": format!("https://zylos.dev/errors/{tipo}"),
                "title": titulo,
                "status": status.as_u16(),
                "detail": e.to_string(),
                "instance": format!("/api/v1/bots/{nome}/perguntar"),
            });
            (status, [(header::CONTENT_TYPE, "application/problem+json")], Json(corpo)).into_response()
        }
    }
}
