//! Phone Relay — pareamento real de dispositivos móveis (store SQLite).
//!
//! Rotas (montadas sob `/api/v1`):
//! - `POST   /hermes/desktop/relay/pair-code` — gera código de pareamento (5 min) + QR SVG
//! - `POST   /hermes/desktop/relay/pair` — dispositivo confirma o código; recebe token (uma vez)
//! - `GET    /hermes/desktop/relay/devices` — lista dispositivos pareados
//! - `DELETE /hermes/desktop/relay/tokens/revoke` — revoga (apaga) o dispositivo e seu token
//!
//! Só o hash SHA-256 do token e do código é persistido. Banco: `ZYLOS_RELAY_DB`
//! (padrão `data/relay.db`; `:memory:` nos testes).

use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use rand::Rng;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

/// Validade do código de pareamento, em segundos.
pub const PAIR_CODE_TTL_SECS: u64 = 300;

/// Alfabeto do código (sem 0/O/1/I para leitura fácil).
const CODE_ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

/// Dispositivo pareado (nunca inclui o token).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RelayDevice {
    pub id: String,
    pub name: String,
    pub platform: String,
    /// Unix seconds.
    pub paired_at: u64,
    /// Unix seconds.
    pub last_seen: u64,
}

/// Código emitido para o QR.
#[derive(Debug, Clone, Serialize)]
pub struct PairCode {
    pub code: String,
    pub expires_at: u64,
    /// Conteúdo codificado no QR.
    pub payload: String,
    /// QR code renderizado em SVG.
    pub qr_svg: String,
}

/// Erros do store.
#[derive(Debug)]
pub enum RelayError {
    InvalidCode,
    NotFound,
    Db(String),
}

impl From<rusqlite::Error> for RelayError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Db(e.to_string())
    }
}

fn sha256_hex(s: &str) -> String {
    hex::encode(Sha256::digest(s.as_bytes()))
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Normaliza o código digitado (maiúsculas, sem hífen/espaço).
fn normalize_code(code: &str) -> String {
    code.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

fn random_code() -> String {
    let mut rng = rand::thread_rng();
    let raw: String = (0..8)
        .map(|_| CODE_ALPHABET[rng.gen_range(0..CODE_ALPHABET.len())] as char)
        .collect();
    format!("{}-{}", &raw[..4], &raw[4..])
}

fn random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    rand::thread_rng().fill(&mut buf[..]);
    hex::encode(buf)
}

/// Renderiza `data` como QR code SVG.
pub fn qr_svg(data: &str) -> Result<String, String> {
    let code = qrcode::QrCode::new(data.as_bytes()).map_err(|e| e.to_string())?;
    Ok(code
        .render::<qrcode::render::svg::Color>()
        .min_dimensions(200, 200)
        .quiet_zone(true)
        .build())
}

/// Store SQLite de dispositivos e códigos de pareamento.
pub struct RelayStore {
    conn: Mutex<Connection>,
}

impl RelayStore {
    /// Abre (ou cria) o banco. `":memory:"` abre em memória.
    pub fn open(path: &str) -> Result<Self, RelayError> {
        let conn = if path == ":memory:" {
            Connection::open_in_memory()?
        } else {
            if let Some(parent) = std::path::Path::new(path).parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            Connection::open(path)?
        };
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS relay_devices (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                platform TEXT NOT NULL,
                token_hash TEXT NOT NULL UNIQUE,
                paired_at INTEGER NOT NULL,
                last_seen INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS relay_pair_codes (
                code_hash TEXT PRIMARY KEY,
                expires_at INTEGER NOT NULL
            );",
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Emite um código novo válido até `now + PAIR_CODE_TTL_SECS`.
    pub fn issue_code(&self, now: u64) -> Result<(String, u64), RelayError> {
        let code = random_code();
        let expires_at = now + PAIR_CODE_TTL_SECS;
        let c = self.conn();
        c.execute(
            "DELETE FROM relay_pair_codes WHERE expires_at <= ?1",
            params![now as i64],
        )?;
        c.execute(
            "INSERT INTO relay_pair_codes (code_hash, expires_at) VALUES (?1, ?2)",
            params![sha256_hex(&normalize_code(&code)), expires_at as i64],
        )?;
        Ok((code, expires_at))
    }

    /// Consome o código (uso único) e cria o dispositivo. Retorna `(device, token)`.
    pub fn pair(
        &self,
        code: &str,
        name: &str,
        platform: &str,
        now: u64,
    ) -> Result<(RelayDevice, String), RelayError> {
        let hash = sha256_hex(&normalize_code(code));
        let mut c = self.conn();
        let tx = c.transaction()?;
        let expires: Option<i64> = tx
            .query_row(
                "SELECT expires_at FROM relay_pair_codes WHERE code_hash = ?1",
                params![hash],
                |r| r.get(0),
            )
            .optional()?;
        tx.execute(
            "DELETE FROM relay_pair_codes WHERE code_hash = ?1",
            params![hash],
        )?;
        match expires {
            Some(exp) if (exp as u64) > now => {}
            _ => {
                tx.commit()?;
                return Err(RelayError::InvalidCode);
            }
        }
        let token = random_hex(32);
        let device = RelayDevice {
            id: format!("dev-{}", random_hex(8)),
            name: name.to_string(),
            platform: platform.to_string(),
            paired_at: now,
            last_seen: now,
        };
        tx.execute(
            "INSERT INTO relay_devices (id, name, platform, token_hash, paired_at, last_seen)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            params![
                device.id,
                device.name,
                device.platform,
                sha256_hex(&token),
                now as i64
            ],
        )?;
        tx.commit()?;
        Ok((device, token))
    }

    /// Dispositivos pareados, mais recentes primeiro.
    pub fn list(&self) -> Result<Vec<RelayDevice>, RelayError> {
        let c = self.conn();
        let mut st = c.prepare(
            "SELECT id, name, platform, paired_at, last_seen FROM relay_devices
             ORDER BY paired_at DESC, id",
        )?;
        let rows = st.query_map([], |r| {
            Ok(RelayDevice {
                id: r.get(0)?,
                name: r.get(1)?,
                platform: r.get(2)?,
                paired_at: r.get::<_, i64>(3)? as u64,
                last_seen: r.get::<_, i64>(4)? as u64,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Valida um token de dispositivo e atualiza `last_seen`.
    pub fn authenticate(&self, token: &str, now: u64) -> Result<Option<String>, RelayError> {
        let c = self.conn();
        let id: Option<String> = c
            .query_row(
                "SELECT id FROM relay_devices WHERE token_hash = ?1",
                params![sha256_hex(token)],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(id) = &id {
            c.execute(
                "UPDATE relay_devices SET last_seen = ?1 WHERE id = ?2",
                params![now as i64, id],
            )?;
        }
        Ok(id)
    }

    /// Revoga o dispositivo (token deixa de valer).
    pub fn revoke(&self, device_id: &str) -> Result<(), RelayError> {
        let n = self.conn().execute(
            "DELETE FROM relay_devices WHERE id = ?1",
            params![device_id],
        )?;
        if n == 0 {
            Err(RelayError::NotFound)
        } else {
            Ok(())
        }
    }
}

/// Estado do router de relay.
#[derive(Clone)]
pub struct RelayState {
    pub store: Arc<RelayStore>,
}

impl RelayState {
    /// Abre `ZYLOS_RELAY_DB` (padrão `data/relay.db`); se falhar, cai para memória.
    pub fn from_env() -> Self {
        let path = std::env::var("ZYLOS_RELAY_DB").unwrap_or_else(|_| "data/relay.db".into());
        let store = RelayStore::open(&path).unwrap_or_else(|e| {
            tracing::warn!(error = ?e, path = %path, "relay: falha ao abrir store, usando memória");
            RelayStore::open(":memory:").expect("sqlite em memória")
        });
        Self {
            store: Arc::new(store),
        }
    }

    /// Store em memória (testes).
    pub fn in_memory() -> Self {
        Self {
            store: Arc::new(RelayStore::open(":memory:").expect("sqlite em memória")),
        }
    }
}

fn err(status: StatusCode, msg: &str) -> Response {
    (status, Json(json!({ "error": msg }))).into_response()
}

fn db_err(e: RelayError) -> Response {
    match e {
        RelayError::InvalidCode => err(StatusCode::BAD_REQUEST, "invalid_or_expired_code"),
        RelayError::NotFound => err(StatusCode::NOT_FOUND, "device_not_found"),
        RelayError::Db(d) => {
            tracing::error!(error = %d, "relay store");
            err(StatusCode::INTERNAL_SERVER_ERROR, "relay_store_error")
        }
    }
}

async fn post_pair_code(State(st): State<RelayState>) -> Response {
    let (code, expires_at) = match st.store.issue_code(now_secs()) {
        Ok(v) => v,
        Err(e) => return db_err(e),
    };
    let payload = format!("zylos-relay://pair?code={code}");
    match qr_svg(&payload) {
        Ok(svg) => (
            StatusCode::CREATED,
            Json(PairCode {
                code,
                expires_at,
                payload,
                qr_svg: svg,
            }),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "relay: falha ao gerar QR");
            err(StatusCode::INTERNAL_SERVER_ERROR, "qr_error")
        }
    }
}

#[derive(Deserialize)]
struct PairRequest {
    code: String,
    device_name: String,
    #[serde(default)]
    platform: Option<String>,
}

async fn post_pair(State(st): State<RelayState>, Json(req): Json<PairRequest>) -> Response {
    let name = req.device_name.trim();
    if name.is_empty() || name.len() > 100 {
        return err(StatusCode::BAD_REQUEST, "invalid_device_name");
    }
    let platform = match req.platform.as_deref().map(str::to_ascii_lowercase).as_deref() {
        Some("android") => "android",
        Some("ios") => "ios",
        _ => "other",
    };
    match st.store.pair(&req.code, name, platform, now_secs()) {
        Ok((device, token)) => (
            StatusCode::CREATED,
            Json(json!({ "device": device, "token": token })),
        )
            .into_response(),
        Err(e) => db_err(e),
    }
}

async fn get_devices(State(st): State<RelayState>) -> Response {
    match st.store.list() {
        Ok(devices) => Json(json!({ "total": devices.len(), "devices": devices })).into_response(),
        Err(e) => db_err(e),
    }
}

#[derive(Deserialize)]
struct RevokeRequest {
    /// Aceita `device_id` ou o nome antigo `token_id`.
    #[serde(alias = "token_id")]
    device_id: String,
}

async fn delete_revoke(State(st): State<RelayState>, Json(req): Json<RevokeRequest>) -> Response {
    match st.store.revoke(&req.device_id) {
        Ok(()) => Json(json!({ "revoked": true, "device_id": req.device_id })).into_response(),
        Err(e) => db_err(e),
    }
}

/// Router do relay (paths completos sob `/api/v1`).
pub fn create_relay_router(state: RelayState) -> Router {
    Router::new()
        .route("/hermes/desktop/relay/pair-code", post(post_pair_code))
        .route("/hermes/desktop/relay/pair", post(post_pair))
        .route("/hermes/desktop/relay/devices", get(get_devices))
        .route("/hermes/desktop/relay/tokens/revoke", delete(delete_revoke))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    async fn call(
        app: &Router,
        method: &str,
        uri: &str,
        body: Option<serde_json::Value>,
    ) -> (StatusCode, serde_json::Value) {
        let mut b = Request::builder().method(method).uri(uri);
        if body.is_some() {
            b = b.header("content-type", "application/json");
        }
        let req = b
            .body(body.map(|v| Body::from(v.to_string())).unwrap_or_else(Body::empty))
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        let st = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20).await.unwrap();
        (st, serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null))
    }

    #[test]
    fn codigo_expira_e_e_uso_unico() {
        let s = RelayStore::open(":memory:").unwrap();
        let (code, exp) = s.issue_code(1000).unwrap();
        assert_eq!(exp, 1000 + PAIR_CODE_TTL_SECS);
        assert!(matches!(
            s.pair(&code, "x", "ios", exp),
            Err(RelayError::InvalidCode)
        ));
        let (code, _) = s.issue_code(1000).unwrap();
        let lower = code.to_lowercase().replace('-', "");
        let (dev, token) = s.pair(&lower, "Pixel", "android", 1001).unwrap();
        assert_eq!(token.len(), 64);
        assert!(matches!(
            s.pair(&code, "y", "ios", 1002),
            Err(RelayError::InvalidCode)
        ));
        assert_eq!(s.authenticate(&token, 1500).unwrap(), Some(dev.id.clone()));
        assert_eq!(s.list().unwrap()[0].last_seen, 1500);
        s.revoke(&dev.id).unwrap();
        assert_eq!(s.authenticate(&token, 1600).unwrap(), None);
        assert!(matches!(s.revoke(&dev.id), Err(RelayError::NotFound)));
    }

    #[test]
    fn token_nao_e_persistido_em_claro() {
        let s = RelayStore::open(":memory:").unwrap();
        let (code, _) = s.issue_code(10).unwrap();
        let (_, token) = s.pair(&code, "a", "ios", 11).unwrap();
        let stored: String = s
            .conn()
            .query_row("SELECT token_hash FROM relay_devices", [], |r| r.get(0))
            .unwrap();
        assert_ne!(stored, token);
        assert_eq!(stored, sha256_hex(&token));
    }

    #[test]
    fn persiste_em_arquivo() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub").join("relay.db");
        let p = path.to_str().unwrap();
        {
            let s = RelayStore::open(p).unwrap();
            let (code, _) = s.issue_code(10).unwrap();
            s.pair(&code, "a", "ios", 11).unwrap();
        }
        assert_eq!(RelayStore::open(p).unwrap().list().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn rotas_pair_code_pair_devices_revoke() {
        let app = create_relay_router(RelayState::in_memory());
        let (st, v) = call(&app, "GET", "/hermes/desktop/relay/devices", None).await;
        assert_eq!(st, StatusCode::OK);
        assert_eq!(v["total"], 0);

        let (st, v) = call(&app, "POST", "/hermes/desktop/relay/pair-code", None).await;
        assert_eq!(st, StatusCode::CREATED);
        let code = v["code"].as_str().unwrap().to_string();
        assert_eq!(code.len(), 9);
        assert!(v["qr_svg"].as_str().unwrap().contains("<svg"));
        assert!(v["payload"].as_str().unwrap().ends_with(&code));

        let (st, v) = call(
            &app,
            "POST",
            "/hermes/desktop/relay/pair",
            Some(json!({"code": "ZZZZ-ZZZZ", "device_name": "x"})),
        )
        .await;
        assert_eq!(st, StatusCode::BAD_REQUEST);
        assert_eq!(v["error"], "invalid_or_expired_code");

        let (st, v) = call(
            &app,
            "POST",
            "/hermes/desktop/relay/pair",
            Some(json!({"code": code, "device_name": "Pixel 8", "platform": "Android"})),
        )
        .await;
        assert_eq!(st, StatusCode::CREATED);
        assert_eq!(v["device"]["platform"], "android");
        assert_eq!(v["token"].as_str().unwrap().len(), 64);
        let id = v["device"]["id"].as_str().unwrap().to_string();

        let (_, v) = call(&app, "GET", "/hermes/desktop/relay/devices", None).await;
        assert_eq!(v["total"], 1);
        assert_eq!(v["devices"][0]["name"], "Pixel 8");
        assert!(v["devices"][0].get("token_hash").is_none());

        let revoke = "/hermes/desktop/relay/tokens/revoke";
        let (st, _) = call(&app, "DELETE", revoke, Some(json!({"device_id": id}))).await;
        assert_eq!(st, StatusCode::OK);
        let (st, _) = call(&app, "DELETE", revoke, Some(json!({"token_id": id}))).await;
        assert_eq!(st, StatusCode::NOT_FOUND);
        let (_, v) = call(&app, "GET", "/hermes/desktop/relay/devices", None).await;
        assert_eq!(v["total"], 0);
    }
}
