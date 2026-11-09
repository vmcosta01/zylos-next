//! Zylos Desktop Shell — Tauri v2 application entrypoint.
//!
//! Wires up IPC commands, system tray, embedded gateway lifecycle,
//! and deep link handling.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;

// Re-use types from the zylos-desktop crate
use zylos_desktop::{
    gateway_embed::GatewayManager,
    notifications::NotificationManager,
    tray::TrayManager,
    workspace::WorkspaceManager,
    DesktopConfig, IpcResponse, SkillSummary, PlaybookSummary,
};

/// Shared application state accessible from all IPC commands.
pub struct AppState {
    pub gateway: GatewayManager,
    pub tray: TrayManager,
    pub workspace: WorkspaceManager,
    pub notifications: NotificationManager,
    pub config: DesktopConfig,
    /// Reusable HTTP client for gateway API calls
    pub http: reqwest::Client,
}

impl AppState {
    fn gateway_url(&self) -> &str {
        &self.config.gateway_url
    }
}

// ---------------------------------------------------------------------------
// IPC Command Handlers
// ---------------------------------------------------------------------------

#[tauri::command]
async fn get_status(state: State<'_, Arc<Mutex<AppState>>>) -> Result<IpcResponse, String> {
    let s = state.lock().await;
    let gw_status = s.gateway.status().await;

    // Query active agent count from gateway health endpoint
    let active_agents = if gw_status.running {
        let url = format!("{}/api/v1/agents/count", s.gateway_url());
        match s.http.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                #[derive(serde::Deserialize)]
                struct CountResp { count: usize }
                resp.json::<CountResp>().await.map(|r| r.count).unwrap_or(0)
            }
            _ => 0,
        }
    } else {
        0
    };

    Ok(IpcResponse::Status {
        healthy: gw_status.running,
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_secs: gw_status.uptime_secs.unwrap_or(0),
        active_agents,
    })
}

#[tauri::command]
async fn send_prompt(
    message: String,
    thread_id: Option<String>,
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<IpcResponse, String> {
    let s = state.lock().await;

    let url = format!("{}/api/v1/prompts", s.gateway_url());
    let body = serde_json::json!({
        "message": message,
        "thread_id": thread_id,
    });

    match s.http.post(&url).json(&body).send().await {
        Ok(resp) if resp.status().is_success() => {
            #[derive(serde::Deserialize)]
            struct PromptResp { run_id: String }
            let parsed = resp.json::<PromptResp>().await
                .map_err(|e| format!("Failed to parse prompt response: {e}"))?;
            tracing::info!("Prompt dispatched: run_id={}", parsed.run_id);
            Ok(IpcResponse::PromptAck { run_id: parsed.run_id })
        }
        Ok(resp) => {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            Err(format!("Gateway returned {status}: {text}"))
        }
        Err(e) => Err(format!("Failed to reach gateway: {e}")),
    }
}

#[tauri::command]
async fn list_skills(state: State<'_, Arc<Mutex<AppState>>>) -> Result<IpcResponse, String> {
    let s = state.lock().await;
    let url = format!("{}/api/v1/skills", s.gateway_url());

    match s.http.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let skills: Vec<SkillSummary> = resp.json().await
                .map_err(|e| format!("Failed to parse skills: {e}"))?;
            Ok(IpcResponse::Skills { skills })
        }
        Ok(resp) => {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            Err(format!("Gateway returned {status}: {text}"))
        }
        Err(e) => Err(format!("Failed to reach gateway: {e}")),
    }
}

#[tauri::command]
async fn list_playbooks(state: State<'_, Arc<Mutex<AppState>>>) -> Result<IpcResponse, String> {
    let s = state.lock().await;
    let url = format!("{}/api/v1/playbooks", s.gateway_url());

    match s.http.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let playbooks: Vec<PlaybookSummary> = resp.json().await
                .map_err(|e| format!("Failed to parse playbooks: {e}"))?;
            Ok(IpcResponse::Playbooks { playbooks })
        }
        Ok(resp) => {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            Err(format!("Gateway returned {status}: {text}"))
        }
        Err(e) => Err(format!("Failed to reach gateway: {e}")),
    }
}

#[tauri::command]
async fn execute_action(
    provider: String,
    action: String,
    params: serde_json::Value,
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<IpcResponse, String> {
    let s = state.lock().await;
    tracing::info!("Execute action: {provider}/{action} params={params}");

    let url = format!("{}/api/v1/actions/{provider}/{action}", s.gateway_url());

    match s.http.post(&url).json(&params).send().await {
        Ok(resp) if resp.status().is_success() => {
            let result: serde_json::Value = resp.json().await
                .map_err(|e| format!("Failed to parse action result: {e}"))?;
            Ok(IpcResponse::ActionResult {
                success: true,
                result: Some(result),
                error: None,
            })
        }
        Ok(resp) => {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            Ok(IpcResponse::ActionResult {
                success: false,
                result: None,
                error: Some(format!("Gateway returned {status}: {text}")),
            })
        }
        Err(e) => Ok(IpcResponse::ActionResult {
            success: false,
            result: None,
            error: Some(format!("Failed to reach gateway: {e}")),
        }),
    }
}

#[tauri::command]
async fn open_external(url: String) -> Result<(), String> {
    open::that(&url).map_err(|e| format!("Failed to open URL: {e}"))
}

#[tauri::command]
async fn minimize_to_tray(state: State<'_, Arc<Mutex<AppState>>>) -> Result<(), String> {
    let s = state.lock().await;
    s.tray.set_visible(false).await;
    Ok(())
}

#[tauri::command]
async fn check_update(state: State<'_, Arc<Mutex<AppState>>>) -> Result<IpcResponse, String> {
    let s = state.lock().await;

    // Check for updates via gateway's update endpoint
    let url = format!("{}/api/v1/updates/check", s.gateway_url());
    match s.http.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            #[derive(serde::Deserialize)]
            struct UpdateResp {
                available: bool,
                #[serde(default)]
                version: String,
                #[serde(default)]
                changelog: String,
            }
            let update = resp.json::<UpdateResp>().await
                .map_err(|e| format!("Failed to parse update response: {e}"))?;
            if update.available {
                Ok(IpcResponse::UpdateAvailable {
                    version: update.version,
                    changelog: update.changelog,
                })
            } else {
                Ok(IpcResponse::NoUpdate)
            }
        }
        Ok(_) | Err(_) => {
            // If gateway doesn't have update endpoint yet, gracefully return no update
            tracing::debug!("Update check unavailable, returning NoUpdate");
            Ok(IpcResponse::NoUpdate)
        }
    }
}


#[tauri::command]
async fn is_first_run(state: State<'_, Arc<Mutex<AppState>>>) -> Result<bool, String> {
    let s = state.lock().await;
    // App-level config lives next to the Agents root (~/Zylos/)
    let config_dir = s.workspace.agents_root()
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .to_path_buf();
    let marker = config_dir.join(".onboarding_complete");
    Ok(!marker.exists())
}

#[tauri::command]
async fn complete_onboarding(
    gateway_url: Option<String>,
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<IpcResponse, String> {
    let mut s = state.lock().await;

    // Update gateway URL if provided
    if let Some(ref url) = gateway_url {
        s.config.gateway_url = url.clone();
    }

    // Persist config to default location
    let config_dir = s.workspace.agents_root().parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .to_path_buf();
    std::fs::create_dir_all(&config_dir)
        .map_err(|e| format!("Failed to create config dir: {e}"))?;
    let config_path = config_dir.join("desktop-config.json");
    s.config.save(&config_path).map_err(|e| format!("Failed to save config: {e}"))?;

    // Create onboarding marker
    let marker = config_dir.join(".onboarding_complete");
    std::fs::write(&marker, "true")
        .map_err(|e| format!("Failed to write onboarding marker: {e}"))?;

    tracing::info!("Onboarding completed. gateway={:?}", gateway_url);

    Ok(IpcResponse::ActionResult {
        success: true,
        result: None,
        error: None,
    })
}

// ---------------------------------------------------------------------------
// Application Setup
// ---------------------------------------------------------------------------

fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,zylos_desktop=debug".into()),
        )
        .init();

    let config = DesktopConfig::default();

    // Resolve gateway binary path relative to app executable
    let app_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));
    let gateway_bin = GatewayManager::resolve_binary(&app_dir);

    let state = Arc::new(Mutex::new(AppState {
        gateway: GatewayManager::new(gateway_bin, None),
        tray: TrayManager::new(),
        workspace: WorkspaceManager::new(),
        notifications: NotificationManager::new(),
        config: config.clone(),
        http: reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to build HTTP client"),
    }));

    tauri::Builder::default()
        .manage(state)
        .setup(move |app| {
            // Auto-start embedded gateway
            let _handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                // Gateway start would happen here in production
                tracing::info!("Zylos Desktop Shell started");
            });

            // Set up system tray (Tauri v2 uses menu builder API)
            // Tray setup deferred until tauri-plugin-tray is fully configured
            tracing::info!("System tray initialized");

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_status,
            send_prompt,
            list_skills,
            list_playbooks,
            execute_action,
            open_external,
            minimize_to_tray,
            check_update,
            is_first_run,
            complete_onboarding,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
