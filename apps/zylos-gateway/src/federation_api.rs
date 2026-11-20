//! Federação: o que o gateway sabe de verdade sobre os outros gateways ("nós") configurados.
//!
//! O operador lista os nós em `ZYLOS_FEDERATION_PEERS` (`nome=url` ou só `url`, separados por
//! vírgula; até 16). A cada consulta o gateway pergunta `GET <url>/api/v1/health` a cada nó, em
//! paralelo, com limite de 2 s e sem seguir redirecionamentos, e devolve se está no ar, em quanto
//! tempo respondeu e que versão diz ser. O resultado fica em cache por 5 s.
//!
//! Não há relógio híbrido, vetor de clock nem conflitos aqui: o gateway não tem essas peças, e a
//! rota não as inventa.
//!
//! As URLs vêm só da configuração do operador; a rota não aceita URL nenhuma do cliente (nada de
//! SSRF por parâmetro) e o texto de erro devolvido é só a categoria (timeout, conexão, HTTP N),
//! nunca o corpo da resposta nem a mensagem do sistema.
//!
//! ## Endpoint (sob `/api/v1`)
//! - `GET /federation` -> `{ self, peers: [{ id, url, status, latency_ms, version, error }], checked_at }`

use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::{extract::State, response::IntoResponse, routing::get, Json, Router};
use serde::Serialize;
use tokio::sync::Mutex;

/// Máximo de nós aceitos da configuração.
pub const MAX_PEERS: usize = 16;
const TIMEOUT: Duration = Duration::from_secs(2);
const CACHE: Duration = Duration::from_secs(5);
const MAX_BODY: usize = 16 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Peer {
    pub id: String,
    pub url: String,
}

/// Lê `nome=url,url,...`. Ignora o que não for `http(s)://host`, repete nomes com sufixo e para em
/// [`MAX_PEERS`]. Barra final da URL é tirada.
pub fn parse_peers(bruto: &str) -> Vec<Peer> {
    let mut peers: Vec<Peer> = Vec::new();
    for item in bruto.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        if peers.len() >= MAX_PEERS {
            break;
        }
        let (nome, url) = match item.split_once('=') {
            Some((n, u)) => (n.trim().to_string(), u.trim()),
            None => (String::new(), item),
        };
        let Ok(u) = reqwest::Url::parse(url) else {
            continue;
        };
        if !matches!(u.scheme(), "http" | "https") || u.host_str().is_none() {
            continue;
        }
        let url = url.trim_end_matches('/').to_string();
        let base = if nome.is_empty() {
            u.host_str().unwrap_or("no").to_string()
        } else {
            nome
        };
        let mut id = base.clone();
        let mut n = 2;
        while peers.iter().any(|p| p.id == id) {
            id = format!("{base}-{n}");
            n += 1;
        }
        peers.push(Peer { id, url });
    }
    peers
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PeerStatus {
    pub id: String,
    pub url: String,
    /// `online`, `degraded` (respondeu, mas não saudável) ou `offline`.
    pub status: &'static str,
    pub latency_ms: Option<u64>,
    pub version: Option<String>,
    /// Categoria do problema, nunca o corpo da resposta.
    pub error: Option<String>,
}

/// Última sondagem: quando foi feita e o que achou.
type UltimaSondagem = Option<(Instant, Vec<PeerStatus>)>;

#[derive(Clone)]
pub struct FederationState {
    peers: Arc<Vec<Peer>>,
    cliente: reqwest::Client,
    cache: Arc<Mutex<UltimaSondagem>>,
}

impl FederationState {
    pub fn new(peers: Vec<Peer>) -> Self {
        Self {
            peers: Arc::new(peers),
            cliente: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(TIMEOUT)
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            cache: Arc::new(Mutex::new(None)),
        }
    }

    /// Nós de `ZYLOS_FEDERATION_PEERS` (vazio se não definida).
    pub fn do_ambiente() -> Self {
        Self::new(parse_peers(
            &std::env::var("ZYLOS_FEDERATION_PEERS").unwrap_or_default(),
        ))
    }

    async fn sondar(&self, p: &Peer) -> PeerStatus {
        let url = format!("{}/api/v1/health", p.url);
        let inicio = Instant::now();
        let base = PeerStatus {
            id: p.id.clone(),
            url: p.url.clone(),
            status: "offline",
            latency_ms: None,
            version: None,
            error: None,
        };
        let resp = match self.cliente.get(&url).send().await {
            Ok(r) => r,
            Err(e) => {
                let erro = if e.is_timeout() {
                    "tempo esgotado"
                } else {
                    "sem conexão"
                };
                return PeerStatus {
                    error: Some(erro.into()),
                    ..base
                };
            }
        };
        let latencia = inicio.elapsed().as_millis() as u64;
        let http = resp.status();
        if !http.is_success() {
            return PeerStatus {
                latency_ms: Some(latencia),
                error: Some(format!("HTTP {}", http.as_u16())),
                ..base
            };
        }
        // Corpo limitado: um nó malicioso não faz o gateway ler sem fim.
        let corpo = resp.bytes().await.unwrap_or_default();
        let json: serde_json::Value = if corpo.len() <= MAX_BODY {
            serde_json::from_slice(&corpo).unwrap_or_default()
        } else {
            serde_json::Value::Null
        };
        let saudavel = matches!(
            json.get("status").and_then(|s| s.as_str()),
            Some("healthy" | "ok" | "ready")
        );
        let version = json
            .get("version")
            .and_then(|v| v.as_str())
            .map(|v| v.chars().take(40).collect::<String>());
        PeerStatus {
            status: if saudavel { "online" } else { "degraded" },
            latency_ms: Some(latencia),
            version,
            error: (!saudavel).then(|| "resposta sem status saudável".to_string()),
            ..base
        }
    }

    /// Estado atual dos nós (do cache se tiver menos de 5 s).
    pub async fn estado(&self) -> Vec<PeerStatus> {
        let mut cache = self.cache.lock().await;
        if let Some((quando, lista)) = cache.as_ref() {
            if quando.elapsed() < CACHE {
                return lista.clone();
            }
        }
        let sondas = self.peers.iter().map(|p| self.sondar(p));
        let lista = futures_util::future::join_all(sondas).await;
        *cache = Some((Instant::now(), lista.clone()));
        lista
    }
}

async fn get_federation(State(st): State<FederationState>) -> impl IntoResponse {
    let peers = st.estado().await;
    Json(serde_json::json!({
        "self": { "version": env!("CARGO_PKG_VERSION") },
        "peers": peers,
        "checked_at": chrono::Utc::now().to_rfc3339(),
    }))
}

/// Roteador com estado próprio (vira `Router<()>` para o `.merge`).
pub fn create_federation_router(state: FederationState) -> Router {
    Router::new()
        .route("/federation", get(get_federation))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_lista_de_nos_e_ignora_o_que_nao_serve() {
        let p = parse_peers(
            "leste=http://10.0.0.5:3000/, https://b.exemplo.com ,ftp://x, sem-esquema:3000, =http://c.exemplo.com, lixo",
        );
        let ids: Vec<_> = p.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["leste", "b.exemplo.com", "c.exemplo.com"]);
        assert_eq!(p[0].url, "http://10.0.0.5:3000");
        assert_eq!(p[1].url, "https://b.exemplo.com");
    }

    #[test]
    fn nomes_repetidos_ganham_sufixo_e_ha_limite() {
        let p = parse_peers("a=http://x:1,a=http://y:1,a=http://z:1");
        let ids: Vec<_> = p.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["a", "a-2", "a-3"]);
        let muitos: String = (0..40)
            .map(|i| format!("n{i}=http://h{i}:1"))
            .collect::<Vec<_>>()
            .join(",");
        assert_eq!(parse_peers(&muitos).len(), MAX_PEERS);
        assert!(parse_peers("").is_empty());
    }
}
