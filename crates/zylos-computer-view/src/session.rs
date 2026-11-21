//! ComputerSessionManager — gerenciamento de sessões de computer view em memória
//!
//! Gerencia sessões ativas, leases de controle com timeout automático,
//! e captura screenshots reais da tela local (xcap) quando VNC indisponível.
//! Sessões são armazenadas em DashMap para acesso concorrente seguro.

use crate::schema::*;
use base64::Engine as _;
use dashmap::DashMap;
use std::sync::Arc;
use uuid::Uuid;

/// Gerenciador de sessões de computer view
#[derive(Clone)]
pub struct ComputerSessionManager {
    sessions: Arc<DashMap<String, ComputerSession>>,
    default_lease_minutes: u64,
}

impl ComputerSessionManager {
    pub fn new(default_lease_minutes: u64) -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
            default_lease_minutes,
        }
    }

    /// Cria nova sessão de computer view
    pub fn create_session(&self, req: CreateSessionRequest) -> ComputerSession {
        let now = chrono::Utc::now().to_rfc3339();
        let id = Uuid::new_v4().to_string();

        let session = ComputerSession {
            id: id.clone(),
            bot_id: req.bot_id,
            state: SessionState::Connecting.as_str().to_string(),
            vnc_url: req.vnc_endpoint,
            screenshot_url: Some(format!("/api/v1/computer-view/sessions/{}/screenshot", id)),
            control_lease: None,
            created_at: now.clone(),
            updated_at: now,
            metadata: req.metadata,
        };

        self.sessions.insert(id.clone(), session.clone());

        tracing::info!(
            session_id = %id,
            bot_id = %session.bot_id,
            "Computer view session created"
        );

        session
    }

    /// Busca sessão por ID
    pub fn get_session(&self, session_id: &str) -> Option<ComputerSession> {
        self.sessions.get(session_id).map(|s| s.value().clone())
    }

    /// Lista todas as sessões ativas (não terminadas)
    pub fn list_active_sessions(&self) -> Vec<ComputerSession> {
        self.sessions
            .iter()
            .filter(|entry| {
                let state = SessionState::parse(&entry.value().state);
                matches!(state, Some(s) if s.is_active() || s == SessionState::Connecting)
            })
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Lista sessões de um bot específico
    pub fn list_bot_sessions(&self, bot_id: &str) -> Vec<ComputerSession> {
        self.sessions
            .iter()
            .filter(|entry| entry.value().bot_id == bot_id)
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Atualiza estado da sessão
    pub fn update_state(
        &self,
        session_id: &str,
        new_state: SessionState,
    ) -> Result<ComputerSession, String> {
        if let Some(mut entry) = self.sessions.get_mut(session_id) {
            let session = entry.value_mut();
            session.state = new_state.as_str().to_string();
            session.updated_at = chrono::Utc::now().to_rfc3339();
            Ok(session.clone())
        } else {
            Err(format!("Session not found: {}", session_id))
        }
    }

    /// Usuário assume controle do desktop
    pub fn take_control(
        &self,
        session_id: &str,
        req: TakeControlRequest,
    ) -> Result<ComputerSession, String> {
        if let Some(mut entry) = self.sessions.get_mut(session_id) {
            let session = entry.value_mut();

            // Verificar se sessão está ativa
            let current_state = SessionState::parse(&session.state);
            if !matches!(current_state, Some(s) if s.is_active()) {
                return Err(format!(
                    "Session {} is not active (state: {})",
                    session_id, session.state
                ));
            }

            // Verificar se já tem lease ativo e não expirado
            if let Some(ref lease) = session.control_lease {
                if !lease.is_expired() && lease.user_id != req.user_id {
                    return Err(format!(
                        "Session {} already controlled by user {} (expires: {})",
                        session_id, lease.user_id, lease.expires_at
                    ));
                }
            }

            let duration = req.duration_minutes.unwrap_or(self.default_lease_minutes);
            let expires = chrono::Utc::now() + chrono::Duration::minutes(duration as i64);

            session.control_lease = Some(ControlLease {
                user_id: req.user_id.clone(),
                taken_at: chrono::Utc::now().to_rfc3339(),
                expires_at: expires.to_rfc3339(),
                reason: req.reason,
            });
            session.state = SessionState::UserControl.as_str().to_string();
            session.updated_at = chrono::Utc::now().to_rfc3339();

            tracing::info!(
                session_id = %session_id,
                user_id = %req.user_id,
                duration_minutes = duration,
                "Control lease granted"
            );

            Ok(session.clone())
        } else {
            Err(format!("Session not found: {}", session_id))
        }
    }

    /// Usuário devolve controle ao agente
    pub fn release_control(
        &self,
        session_id: &str,
        user_id: &str,
    ) -> Result<ComputerSession, String> {
        if let Some(mut entry) = self.sessions.get_mut(session_id) {
            let session = entry.value_mut();

            // Verificar se o usuário é quem tem o lease
            if let Some(ref lease) = session.control_lease {
                if lease.user_id != user_id {
                    return Err(format!(
                        "Session {} controlled by {}, not {}",
                        session_id, lease.user_id, user_id
                    ));
                }
            } else {
                return Err(format!(
                    "Session {} has no active control lease",
                    session_id
                ));
            }

            session.control_lease = None;
            session.state = SessionState::Active.as_str().to_string();
            session.updated_at = chrono::Utc::now().to_rfc3339();

            tracing::info!(
                session_id = %session_id,
                user_id = %user_id,
                "Control lease released"
            );

            Ok(session.clone())
        } else {
            Err(format!("Session not found: {}", session_id))
        }
    }

    /// Encerra sessão
    pub fn terminate_session(&self, session_id: &str) -> Result<(), String> {
        if let Some(mut entry) = self.sessions.get_mut(session_id) {
            let session = entry.value_mut();
            session.state = SessionState::Terminated.as_str().to_string();
            session.control_lease = None;
            session.updated_at = chrono::Utc::now().to_rfc3339();

            tracing::info!(session_id = %session_id, "Session terminated");
            Ok(())
        } else {
            Err(format!("Session not found: {}", session_id))
        }
    }

    /// Remove sessões terminadas ou com erro (cleanup)
    pub fn cleanup_stale_sessions(&self) -> usize {
        let before = self.sessions.len();
        self.sessions.retain(|_, session| {
            let state = SessionState::parse(&session.state);
            !matches!(state, Some(SessionState::Terminated | SessionState::Error))
        });
        let removed = before - self.sessions.len();
        if removed > 0 {
            tracing::info!(removed = removed, "Stale sessions cleaned up");
        }
        removed
    }

    /// Expira leases de controle automaticamente
    pub fn expire_leases(&self) -> usize {
        let mut expired_count = 0;
        for mut entry in self.sessions.iter_mut() {
            let session = entry.value_mut();
            // Clonar dados do lease antes de mutar para evitar borrow conflict
            if let Some(lease) = session.control_lease.clone() {
                if lease.is_expired() {
                    session.control_lease = None;
                    session.state = SessionState::Active.as_str().to_string();
                    session.updated_at = chrono::Utc::now().to_rfc3339();
                    expired_count += 1;

                    tracing::info!(
                    session_id = %session.id,
                    user_id = %lease.user_id,
                    "Control lease expired automatically"
                    );
                }
            }
        }
        expired_count
    }

    /// Captura a tela local real (monitor primário) e devolve PNG em base64.
    ///
    /// Usa `xcap` (Windows/macOS/Linux X11/Wayland). Em ambientes sem display
    /// (headless/CI) ou sem permissão de captura retorna `Err` descritivo —
    /// nunca uma imagem falsa.
    pub fn capture_screenshot(&self, session_id: &str) -> Result<ScreenshotData, String> {
        let _session = self
            .get_session(session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?;

        let image = capture_primary_monitor()?;
        let (width, height) = image.dimensions();
        let png = encode_png(&image)?;

        Ok(ScreenshotData {
            session_id: session_id.to_string(),
            image_base64: base64::engine::general_purpose::STANDARD.encode(png),
            width,
            height,
            captured_at: chrono::Utc::now().to_rfc3339(),
            format: "png".into(),
        })
    }

    /// Conta sessões ativas
    pub fn active_count(&self) -> usize {
        self.sessions
            .iter()
            .filter(|entry| {
                let state = SessionState::parse(&entry.value().state);
                matches!(state, Some(s) if s.is_active())
            })
            .count()
    }

    /// Conta total de sessões
    pub fn total_count(&self) -> usize {
        self.sessions.len()
    }
}

impl Default for ComputerSessionManager {
    fn default() -> Self {
        Self::new(5) // 5 minutos de lease padrão
    }
}

/// Captura o monitor primário (ou o primeiro disponível).
fn capture_primary_monitor() -> Result<image::RgbaImage, String> {
    let monitors = xcap::Monitor::all()
        .map_err(|e| format!("screen capture unavailable (no display?): {e}"))?;
    let monitor = monitors
        .iter()
        .find(|m| m.is_primary().unwrap_or(false))
        .or_else(|| monitors.first())
        .ok_or_else(|| "screen capture unavailable: no monitor found".to_string())?;
    let img = monitor
        .capture_image()
        .map_err(|e| format!("screen capture failed: {e}"))?;
    if img.width() == 0 || img.height() == 0 {
        return Err("screen capture returned an empty image".into());
    }
    Ok(img)
}

/// Codifica uma imagem RGBA como PNG.
pub fn encode_png(img: &image::RgbaImage) -> Result<Vec<u8>, String> {
    use image::ImageEncoder;
    let mut out = Vec::new();
    image::codecs::png::PngEncoder::new(&mut out)
        .write_image(
            img.as_raw(),
            img.width(),
            img.height(),
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|e| format!("PNG encode failed: {e}"))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_create_request() -> CreateSessionRequest {
        CreateSessionRequest {
            bot_id: "bot-1".into(),
            vnc_endpoint: Some("ws://localhost:5900".into()),
            metadata: None,
        }
    }

    #[test]
    fn test_create_and_get_session() {
        let mgr = ComputerSessionManager::default();
        let session = mgr.create_session(sample_create_request());

        assert_eq!(session.bot_id, "bot-1");
        assert_eq!(session.state, "connecting");
        assert!(session.vnc_url.is_some());

        let retrieved = mgr.get_session(&session.id).unwrap();
        assert_eq!(retrieved.id, session.id);
    }

    #[test]
    fn test_get_nonexistent_returns_none() {
        let mgr = ComputerSessionManager::default();
        assert!(mgr.get_session("nonexistent").is_none());
    }

    #[test]
    fn test_update_state() {
        let mgr = ComputerSessionManager::default();
        let session = mgr.create_session(sample_create_request());

        let updated = mgr.update_state(&session.id, SessionState::Active).unwrap();
        assert_eq!(updated.state, "active");
    }

    #[test]
    fn test_update_nonexistent_returns_error() {
        let mgr = ComputerSessionManager::default();
        let result = mgr.update_state("nonexistent", SessionState::Active);
        assert!(result.is_err());
    }

    #[test]
    fn test_take_control() {
        let mgr = ComputerSessionManager::default();
        let session = mgr.create_session(sample_create_request());
        mgr.update_state(&session.id, SessionState::Active).unwrap();

        let req = TakeControlRequest {
            user_id: "user-1".into(),
            duration_minutes: Some(10),
            reason: Some("Debugging".into()),
        };

        let controlled = mgr.take_control(&session.id, req).unwrap();
        assert_eq!(controlled.state, "user_control");
        assert!(controlled.control_lease.is_some());
        let lease = controlled.control_lease.unwrap();
        assert_eq!(lease.user_id, "user-1");
        assert_eq!(lease.reason.as_deref(), Some("Debugging"));
    }

    #[test]
    fn test_take_control_on_inactive_session_fails() {
        let mgr = ComputerSessionManager::default();
        let session = mgr.create_session(sample_create_request());
        // Estado é "connecting", não "active"

        let req = TakeControlRequest {
            user_id: "user-1".into(),
            duration_minutes: None,
            reason: None,
        };

        let result = mgr.take_control(&session.id, req);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not active"));
    }

    #[test]
    fn test_release_control() {
        let mgr = ComputerSessionManager::default();
        let session = mgr.create_session(sample_create_request());
        mgr.update_state(&session.id, SessionState::Active).unwrap();

        let req = TakeControlRequest {
            user_id: "user-1".into(),
            duration_minutes: Some(10),
            reason: None,
        };
        mgr.take_control(&session.id, req).unwrap();

        let released = mgr.release_control(&session.id, "user-1").unwrap();
        assert_eq!(released.state, "active");
        assert!(released.control_lease.is_none());
    }

    #[test]
    fn test_release_control_wrong_user_fails() {
        let mgr = ComputerSessionManager::default();
        let session = mgr.create_session(sample_create_request());
        mgr.update_state(&session.id, SessionState::Active).unwrap();

        let req = TakeControlRequest {
            user_id: "user-1".into(),
            duration_minutes: Some(10),
            reason: None,
        };
        mgr.take_control(&session.id, req).unwrap();

        let result = mgr.release_control(&session.id, "user-2");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not user-2"));
    }

    #[test]
    fn test_terminate_session() {
        let mgr = ComputerSessionManager::default();
        let session = mgr.create_session(sample_create_request());

        mgr.terminate_session(&session.id).unwrap();

        let terminated = mgr.get_session(&session.id).unwrap();
        assert_eq!(terminated.state, "terminated");
        assert!(terminated.control_lease.is_none());
    }

    #[test]
    fn test_list_active_sessions() {
        let mgr = ComputerSessionManager::default();
        let s1 = mgr.create_session(sample_create_request());
        let s2 = mgr.create_session(sample_create_request());
        let _s3 = mgr.create_session(sample_create_request());

        mgr.update_state(&s1.id, SessionState::Active).unwrap();
        mgr.update_state(&s2.id, SessionState::Terminated).unwrap();
        // s3 fica em "connecting"

        let active = mgr.list_active_sessions();
        // Active + Connecting = 2 (s1 + s3), s2 é Terminated
        assert_eq!(active.len(), 2);
    }

    #[test]
    fn test_list_bot_sessions() {
        let mgr = ComputerSessionManager::default();
        mgr.create_session(CreateSessionRequest {
            bot_id: "bot-1".into(),
            vnc_endpoint: None,
            metadata: None,
        });
        mgr.create_session(CreateSessionRequest {
            bot_id: "bot-2".into(),
            vnc_endpoint: None,
            metadata: None,
        });
        mgr.create_session(CreateSessionRequest {
            bot_id: "bot-1".into(),
            vnc_endpoint: None,
            metadata: None,
        });

        let bot1_sessions = mgr.list_bot_sessions("bot-1");
        assert_eq!(bot1_sessions.len(), 2);

        let bot2_sessions = mgr.list_bot_sessions("bot-2");
        assert_eq!(bot2_sessions.len(), 1);
    }

    #[test]
    fn test_cleanup_stale_sessions() {
        let mgr = ComputerSessionManager::default();
        let s1 = mgr.create_session(sample_create_request());
        let s2 = mgr.create_session(sample_create_request());
        let s3 = mgr.create_session(sample_create_request());

        mgr.update_state(&s1.id, SessionState::Terminated).unwrap();
        mgr.update_state(&s2.id, SessionState::Error).unwrap();
        // s3 fica ativa

        let removed = mgr.cleanup_stale_sessions();
        assert_eq!(removed, 2);
        assert_eq!(mgr.total_count(), 1);
        assert!(mgr.get_session(&s3.id).is_some());
    }

    #[test]
    fn test_capture_screenshot_real_or_clear_error() {
        let mgr = ComputerSessionManager::default();
        let session = mgr.create_session(sample_create_request());

        match mgr.capture_screenshot(&session.id) {
            Ok(shot) => {
                // Display disponível: deve ser um PNG real com as dimensões da tela.
                assert_eq!(shot.session_id, session.id);
                assert_eq!(shot.format, "png");
                assert!(shot.width > 1 && shot.height > 1);
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(&shot.image_base64)
                    .unwrap();
                assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
                let decoded = image::load_from_memory(&bytes).unwrap();
                assert_eq!(
                    (decoded.width(), decoded.height()),
                    (shot.width, shot.height)
                );
            }
            // Headless/CI: erro explícito, sem imagem falsa.
            Err(e) => assert!(e.contains("screen capture"), "{e}"),
        }
    }

    #[test]
    fn test_encode_png_roundtrip() {
        let mut img = image::RgbaImage::new(3, 2);
        img.put_pixel(2, 1, image::Rgba([10, 20, 30, 255]));
        let png = encode_png(&img).unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        let back = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!(back.dimensions(), (3, 2));
        assert_eq!(back.get_pixel(2, 1), &image::Rgba([10, 20, 30, 255]));
    }

    #[test]
    fn test_capture_screenshot_nonexistent_fails() {
        let mgr = ComputerSessionManager::default();
        let result = mgr.capture_screenshot("nonexistent");
        assert_eq!(result.unwrap_err(), "Session not found: nonexistent");
    }

    #[test]
    fn test_active_count() {
        let mgr = ComputerSessionManager::default();
        let s1 = mgr.create_session(sample_create_request());
        let _s2 = mgr.create_session(sample_create_request());

        mgr.update_state(&s1.id, SessionState::Active).unwrap();
        // s2 está em "connecting"

        assert_eq!(mgr.active_count(), 1);
    }

    #[test]
    fn test_default_lease_duration() {
        let mgr = ComputerSessionManager::new(15); // 15 min default
        let session = mgr.create_session(sample_create_request());
        mgr.update_state(&session.id, SessionState::Active).unwrap();

        let req = TakeControlRequest {
            user_id: "user-1".into(),
            duration_minutes: None, // Deve usar default de 15 min
            reason: None,
        };

        let controlled = mgr.take_control(&session.id, req).unwrap();
        let lease = controlled.control_lease.unwrap();
        // Verificar que expires_at está ~15 min no futuro
        let remaining = lease.remaining_seconds();
        assert!(remaining > 800 && remaining <= 900); // 15 min = 900s
    }
}
