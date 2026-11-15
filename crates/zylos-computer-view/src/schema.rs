//! Schema de dados para Computer View Panel
//!
//! Tipos serializáveis para gerenciamento de sessões de visualização remota
//! e controle humano do desktop do agente.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Estado de uma sessão de computer view
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    /// Sessão criada, aguardando conexão VNC
    Connecting,
    /// Stream ativo, agente operando
    Active,
    /// Usuário assumiu controle
    UserControl,
    /// Sessão pausada (agente esperando)
    Paused,
    /// Sessão encerrada
    Terminated,
    /// Erro na conexão
    Error,
}

impl SessionState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Connecting => "connecting",
            Self::Active => "active",
            Self::UserControl => "user_control",
            Self::Paused => "paused",
            Self::Terminated => "terminated",
            Self::Error => "error",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "connecting" => Some(Self::Connecting),
            "active" => Some(Self::Active),
            "user_control" => Some(Self::UserControl),
            "paused" => Some(Self::Paused),
            "terminated" => Some(Self::Terminated),
            "error" => Some(Self::Error),
            _ => None,
        }
    }

    /// Alias for [`Self::parse`] — used by tests and external callers expecting `from_str` naming.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        Self::parse(s)
    }

    pub fn is_active(&self) -> bool {
        matches!(self, Self::Active | Self::UserControl)
    }
}

impl std::str::FromStr for SessionState {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).ok_or_else(|| format!("unknown session state: {s}"))
    }
}

impl fmt::Display for SessionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Uma sessão de computer view
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputerSession {
    pub id: String,
    pub bot_id: String,
    pub state: String,
    pub vnc_url: Option<String>,
    pub screenshot_url: Option<String>,
    pub control_lease: Option<ControlLease>,
    pub created_at: String,
    pub updated_at: String,
    pub metadata: Option<serde_json::Value>,
}

/// Lease de controle humano sobre o desktop do agente
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlLease {
    pub user_id: String,
    pub taken_at: String,
    pub expires_at: String,
    pub reason: Option<String>,
}

impl ControlLease {
    pub fn is_expired(&self) -> bool {
        let now = chrono::Utc::now().to_rfc3339();
        self.expires_at < now
    }

    pub fn remaining_seconds(&self) -> i64 {
        if let Ok(expires) = chrono::DateTime::parse_from_rfc3339(&self.expires_at) {
            let now = chrono::Utc::now();
            let diff = expires.with_timezone(&chrono::Utc) - now;
            diff.num_seconds().max(0)
        } else {
            0
        }
    }
}

/// Request para criar nova sessão
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSessionRequest {
    pub bot_id: String,
    pub vnc_endpoint: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Request para assumir controle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TakeControlRequest {
    pub user_id: String,
    pub duration_minutes: Option<u64>,
    pub reason: Option<String>,
}

/// Response de sessão
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionResponse {
    pub session: ComputerSession,
    pub stream_url: Option<String>,
    pub screenshot_url: Option<String>,
}

/// Screenshot estático (fallback quando VNC indisponível)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenshotData {
    pub session_id: String,
    pub image_base64: String,
    pub width: u32,
    pub height: u32,
    pub captured_at: String,
    pub format: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_state_roundtrip() {
        let states = [
            SessionState::Connecting,
            SessionState::Active,
            SessionState::UserControl,
            SessionState::Paused,
            SessionState::Terminated,
            SessionState::Error,
        ];
        for s in states {
            let str_val = s.as_str();
            let parsed = SessionState::from_str(str_val).unwrap();
            assert_eq!(s, parsed);
        }
    }

    #[test]
    fn test_session_state_is_active() {
        assert!(SessionState::Active.is_active());
        assert!(SessionState::UserControl.is_active());
        assert!(!SessionState::Connecting.is_active());
        assert!(!SessionState::Paused.is_active());
        assert!(!SessionState::Terminated.is_active());
        assert!(!SessionState::Error.is_active());
    }

    #[test]
    fn test_invalid_state_returns_none() {
        assert!(SessionState::from_str("invalid").is_none());
    }

    #[test]
    fn test_control_lease_expiry() {
        let lease = ControlLease {
            user_id: "user-1".into(),
            taken_at: chrono::Utc::now().to_rfc3339(),
            expires_at: "2020-01-01T00:00:00Z".into(),
            reason: None,
        };
        assert!(lease.is_expired());
        assert_eq!(lease.remaining_seconds(), 0);
    }

    #[test]
    fn test_control_lease_not_expired() {
        let future = chrono::Utc::now() + chrono::Duration::hours(1);
        let lease = ControlLease {
            user_id: "user-1".into(),
            taken_at: chrono::Utc::now().to_rfc3339(),
            expires_at: future.to_rfc3339(),
            reason: Some("Debugging".into()),
        };
        assert!(!lease.is_expired());
        assert!(lease.remaining_seconds() > 3500);
    }

    #[test]
    fn test_session_serialization() {
        let session = ComputerSession {
            id: "sess-1".into(),
            bot_id: "bot-1".into(),
            state: SessionState::Active.as_str().to_string(),
            vnc_url: Some("ws://localhost:5900".into()),
            screenshot_url: Some("/api/v1/computer/sessions/sess-1/screenshot".into()),
            control_lease: None,
            created_at: "2026-11-11T10:00:00Z".into(),
            updated_at: "2026-11-11T10:00:00Z".into(),
            metadata: None,
        };
        let json = serde_json::to_string(&session).unwrap();
        assert!(json.contains("sess-1"));
        assert!(json.contains("active"));
    }

    #[test]
    fn test_screenshot_data_serialization() {
        let screenshot = ScreenshotData {
            session_id: "sess-1".into(),
            image_base64: "iVBORw0KGgo=".into(),
            width: 1920,
            height: 1080,
            captured_at: "2026-11-11T10:00:00Z".into(),
            format: "png".into(),
        };
        let json = serde_json::to_string(&screenshot).unwrap();
        assert!(json.contains("1920"));
        assert!(json.contains("png"));
    }
}
