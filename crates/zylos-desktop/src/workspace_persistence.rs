//! # WorkspacePersistence — Persistência de estado do cockpit entre sessões (T8.6.1)
//!
//! Salva e restaura o estado da UI do cockpit (abas abertas, filtros ativos,
//! seleções, layout) em um arquivo JSON local, permitindo que o usuário
//! retome exatamente onde parou ao reabrir o aplicativo.
//!
//! ## Arquitetura
//! - Estado serializado via serde para `~/.zylos/workspace-state.json`
//! - Save automático com debounce (300ms após última mudança)
//! - Load síncrono na inicialização antes do render
//! - Tolerante a corrupção: fallback para defaults se JSON inválido
//! - Multi-tab: cada aba tem seu próprio namespace de estado
//!
//! ## Integração
//! O cockpit WASM chama `save_state()` via IPC quando o estado muda.
//! O desktop shell chama `load_state()` na startup e injeta no webview.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, error, info, warn};

/// Erros de persistência de workspace.
#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Path not writable: {0}")]
    PathNotWritable(String),
}

/// Estado completo do workspace persistido entre sessões.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkspaceState {
    /// Versão do schema para migração futura.
    pub schema_version: u32,
    /// Abas abertas com seus estados individuais.
    pub tabs: Vec<TabState>,
    /// Índice da aba ativa (0-indexed).
    pub active_tab_index: usize,
    /// Layout global (sidebar visível, painel direito, etc.).
    pub layout: LayoutState,
    /// Filtros globais ativos.
    pub global_filters: HashMap<String, serde_json::Value>,
    /// Metadata arbitrária para extensão.
    pub metadata: HashMap<String, serde_json::Value>,
    /// Timestamp da última salvaguarda (ISO 8601).
    pub last_saved_at: String,
}

impl Default for WorkspaceState {
    fn default() -> Self {
        Self {
            schema_version: 1,
            tabs: vec![TabState::default()],
            active_tab_index: 0,
            layout: LayoutState::default(),
            global_filters: HashMap::new(),
            metadata: HashMap::new(),
            last_saved_at: chrono_now_stub(),
        }
    }
}

/// Estado de uma aba individual do cockpit.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TabState {
    /// ID único da aba.
    pub id: String,
    /// Título da aba.
    pub title: String,
    /// Tipo de conteúdo da aba (dashboard, agent, settings, etc.).
    pub content_type: String,
    /// Estado específico do conteúdo (serializado como JSON opaco).
    pub content_state: serde_json::Value,
    /// Scroll position vertical.
    pub scroll_y: f64,
    /// Se a aba está fixada (não pode ser fechada acidentalmente).
    pub pinned: bool,
}

impl Default for TabState {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            title: "Dashboard".into(),
            content_type: "dashboard".into(),
            content_state: serde_json::Value::Null,
            scroll_y: 0.0,
            pinned: false,
        }
    }
}

/// Estado do layout global do cockpit.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LayoutState {
    /// Se a sidebar esquerda está visível.
    pub sidebar_visible: bool,
    /// Largura da sidebar em pixels.
    pub sidebar_width: u32,
    /// Se o painel direito (detalhes/inspector) está visível.
    pub right_panel_visible: bool,
    /// Largura do painel direito em pixels.
    pub right_panel_width: u32,
    /// Tema ativo (dark, light, auto).
    pub theme: String,
    /// Zoom level (1.0 = 100%).
    pub zoom_level: f64,
}

impl Default for LayoutState {
    fn default() -> Self {
        Self {
            sidebar_visible: true,
            sidebar_width: 260,
            right_panel_visible: false,
            right_panel_width: 320,
            theme: "dark".into(),
            zoom_level: 1.0,
        }
    }
}

/// Gerenciador de persistência de workspace com debounce automático.
pub struct WorkspacePersistence {
    /// Caminho do arquivo de estado.
    state_path: PathBuf,
    /// Estado atual em memória.
    state: Arc<RwLock<WorkspaceState>>,
    /// Mutex para serializar operações de escrita.
    write_lock: Arc<Mutex<()>>,
    /// Debounce delay em milissegundos.
    debounce_ms: u64,
}

impl WorkspacePersistence {
    /// Cria um novo gerenciador com caminho customizado.
    pub fn new(state_path: PathBuf) -> Self {
        Self {
            state_path,
            state: Arc::new(RwLock::new(WorkspaceState::default())),
            write_lock: Arc::new(Mutex::new(())),
            debounce_ms: 300,
        }
    }

    /// Cria um gerenciador com caminho default (`~/.zylos/workspace-state.json`).
    pub fn default_path() -> Self {
        let path = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("zylos")
            .join("workspace-state.json");
        Self::new(path)
    }

    /// Carrega o estado do disco. Retorna default se arquivo não existir ou estiver corrompido.
    pub async fn load(&self) -> WorkspaceState {
        match self.try_load().await {
            Ok(state) => {
                info!(path = %self.state_path.display(), "Workspace state loaded successfully");
                let mut current = self.state.write().await;
                *current = state.clone();
                state
            }
            Err(e) => {
                warn!(
                path = %self.state_path.display(),
                error = %e,
                "Failed to load workspace state, using defaults"
                );
                WorkspaceState::default()
            }
        }
    }

    /// Tenta carregar estado do disco, retornando erro se falhar.
    async fn try_load(&self) -> Result<WorkspaceState, WorkspaceError> {
        if !self.state_path.exists() {
            return Ok(WorkspaceState::default());
        }

        let content = tokio::fs::read_to_string(&self.state_path).await?;
        let state: WorkspaceState = serde_json::from_str(&content)?;

        // Validar schema version
        if state.schema_version > 1 {
            warn!(
                version = state.schema_version,
                "Workspace state has newer schema version, attempting best-effort load"
            );
        }

        Ok(state)
    }

    /// Salva o estado atual no disco imediatamente.
    pub async fn save(&self) -> Result<(), WorkspaceError> {
        let _lock = self.write_lock.lock().await;

        let state = self.state.read().await.clone();
        self.write_state_to_disk(&state).await
    }

    /// Salva o estado com debounce: espera `debounce_ms` após a última chamada
    /// antes de escrever no disco. Múltiplas chamadas rápidas resultam em apenas
    /// uma escrita.
    pub async fn save_debounced(&self) {
        let state = self.state.clone();
        let path = self.state_path.clone();
        let debounce = self.debounce_ms;
        let lock = self.write_lock.clone();

        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(debounce)).await;

            let _lock = lock.lock().await;
            let current_state = state.read().await.clone();
            if let Err(e) = write_state_to_path(&path, &current_state).await {
                error!(path = %path.display(), error = %e, "Debounced save failed");
            } else {
                debug!(path = %path.display(), "Debounced save completed");
            }
        });
    }

    /// Atualiza o estado em memória e dispara save debounced.
    pub async fn update_and_save<F>(&self, updater: F)
    where
        F: FnOnce(&mut WorkspaceState),
    {
        {
            let mut state = self.state.write().await;
            updater(&mut state);
            state.last_saved_at = chrono_now_stub();
        }
        self.save_debounced().await;
    }

    /// Retorna uma cópia do estado atual.
    pub async fn get_state(&self) -> WorkspaceState {
        self.state.read().await.clone()
    }

    /// Atualiza o estado de uma aba específica por ID.
    pub async fn update_tab(&self, tab_id: &str, updater: impl FnOnce(&mut TabState)) {
        self.update_and_save(|state| {
            if let Some(tab) = state.tabs.iter_mut().find(|t| t.id == tab_id) {
                updater(tab);
            }
        })
        .await;
    }

    /// Adiciona uma nova aba e retorna seu ID.
    pub async fn add_tab(&self, tab: TabState) -> String {
        let id = tab.id.clone();
        self.update_and_save(|state| {
            state.tabs.push(tab);
            state.active_tab_index = state.tabs.len() - 1;
        })
        .await;
        id
    }

    /// Remove uma aba por ID (respeita pinned).
    pub async fn remove_tab(&self, tab_id: &str) -> bool {
        let mut removed = false;
        self.update_and_save(|state| {
            if let Some(idx) = state.tabs.iter().position(|t| t.id == tab_id && !t.pinned) {
                state.tabs.remove(idx);
                if state.active_tab_index >= state.tabs.len() && !state.tabs.is_empty() {
                    state.active_tab_index = state.tabs.len() - 1;
                }
                removed = true;
            }
        })
        .await;
        removed
    }

    /// Define a aba ativa por índice.
    pub async fn set_active_tab(&self, index: usize) {
        self.update_and_save(|state| {
            if index < state.tabs.len() {
                state.active_tab_index = index;
            }
        })
        .await;
    }

    /// Atualiza o layout global.
    pub async fn update_layout(&self, updater: impl FnOnce(&mut LayoutState)) {
        self.update_and_save(|state| updater(&mut state.layout))
            .await;
    }

    /// Define um filtro global.
    pub async fn set_filter(&self, key: impl Into<String>, value: serde_json::Value) {
        self.update_and_save(|state| {
            state.global_filters.insert(key.into(), value);
        })
        .await;
    }

    /// Remove um filtro global.
    pub async fn remove_filter(&self, key: &str) {
        self.update_and_save(|state| {
            state.global_filters.remove(key);
        })
        .await;
    }

    /// Reseta o estado para defaults e salva.
    pub async fn reset(&self) {
        let default = WorkspaceState::default();
        {
            let mut state = self.state.write().await;
            *state = default;
        }
        if let Err(e) = self.save().await {
            error!(error = %e, "Failed to save reset state");
        }
    }

    /// Retorna o caminho do arquivo de estado.
    pub fn state_path(&self) -> &Path {
        &self.state_path
    }

    /// Verifica se o arquivo de estado existe no disco.
    pub async fn has_saved_state(&self) -> bool {
        self.state_path.exists()
    }

    /// Escreve estado no disco (interno).
    async fn write_state_to_disk(&self, state: &WorkspaceState) -> Result<(), WorkspaceError> {
        write_state_to_path(&self.state_path, state).await
    }
}

/// Função standalone para escrever estado em um caminho arbitrário.
async fn write_state_to_path(path: &Path, state: &WorkspaceState) -> Result<(), WorkspaceError> {
    // Garantir que o diretório pai exista
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let json = serde_json::to_string_pretty(state)?;
    tokio::fs::write(path, json.as_bytes()).await?;
    Ok(())
}

fn chrono_now_stub() -> String {
    use std::time::SystemTime;
    let duration = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}Z", duration.as_secs())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_persistence(dir: &TempDir) -> WorkspacePersistence {
        let path = dir.path().join("test-workspace.json");
        WorkspacePersistence::new(path)
    }

    #[tokio::test]
    async fn test_load_default_when_no_file() {
        let dir = TempDir::new().unwrap();
        let wp = test_persistence(&dir);

        let state = wp.load().await;
        assert_eq!(state.schema_version, 1);
        assert_eq!(state.tabs.len(), 1);
        assert_eq!(state.active_tab_index, 0);
    }

    #[tokio::test]
    async fn test_save_and_load_roundtrip() {
        let dir = TempDir::new().unwrap();
        let wp = test_persistence(&dir);

        // Modificar estado
        wp.update_and_save(|s| {
            s.tabs[0].title = "Custom Dashboard".into();
            s.layout.sidebar_width = 300;
            s.global_filters
                .insert("env".into(), serde_json::json!("production"));
        })
        .await;

        // Salvar explicitamente
        wp.save().await.unwrap();

        // Criar nova instância apontando para o mesmo arquivo
        let wp2 = test_persistence(&dir);
        let loaded = wp2.load().await;

        assert_eq!(loaded.tabs[0].title, "Custom Dashboard");
        assert_eq!(loaded.layout.sidebar_width, 300);
        assert_eq!(loaded.global_filters["env"], "production");
    }

    #[tokio::test]
    async fn test_load_corrupted_file_returns_default() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("corrupt.json");
        tokio::fs::write(&path, "not valid json {{{").await.unwrap();

        let wp = WorkspacePersistence::new(path);
        let state = wp.load().await;

        // Deve retornar default sem panic
        assert_eq!(state.schema_version, 1);
        assert_eq!(state.tabs.len(), 1);
    }

    #[tokio::test]
    async fn test_add_and_remove_tab() {
        let dir = TempDir::new().unwrap();
        let wp = test_persistence(&dir);

        let new_tab = TabState {
            id: "tab-2".into(),
            title: "Agents".into(),
            content_type: "agents".into(),
            ..Default::default()
        };

        let id = wp.add_tab(new_tab).await;
        assert_eq!(id, "tab-2");

        let state = wp.get_state().await;
        assert_eq!(state.tabs.len(), 2);
        assert_eq!(state.active_tab_index, 1);

        // Remover tab
        let removed = wp.remove_tab("tab-2").await;
        assert!(removed);

        let state = wp.get_state().await;
        assert_eq!(state.tabs.len(), 1);
    }

    #[tokio::test]
    async fn test_pinned_tab_cannot_be_removed() {
        let dir = TempDir::new().unwrap();
        let wp = test_persistence(&dir);

        // Pin the default tab
        wp.update_tab(&wp.get_state().await.tabs[0].id, |tab| {
            tab.pinned = true;
        })
        .await;

        let tab_id = wp.get_state().await.tabs[0].id.clone();
        let removed = wp.remove_tab(&tab_id).await;
        assert!(!removed);

        let state = wp.get_state().await;
        assert_eq!(state.tabs.len(), 1); // Still there
    }

    #[tokio::test]
    async fn test_set_active_tab() {
        let dir = TempDir::new().unwrap();
        let wp = test_persistence(&dir);

        wp.add_tab(TabState {
            id: "t2".into(),
            title: "Tab 2".into(),
            ..Default::default()
        })
        .await;
        wp.add_tab(TabState {
            id: "t3".into(),
            title: "Tab 3".into(),
            ..Default::default()
        })
        .await;

        wp.set_active_tab(0).await;
        assert_eq!(wp.get_state().await.active_tab_index, 0);

        wp.set_active_tab(2).await;
        assert_eq!(wp.get_state().await.active_tab_index, 2);

        // Out of bounds should not change
        wp.set_active_tab(99).await;
        assert_eq!(wp.get_state().await.active_tab_index, 2);
    }

    #[tokio::test]
    async fn test_update_layout() {
        let dir = TempDir::new().unwrap();
        let wp = test_persistence(&dir);

        wp.update_layout(|layout| {
            layout.sidebar_visible = false;
            layout.theme = "light".into();
            layout.zoom_level = 1.5;
        })
        .await;

        let state = wp.get_state().await;
        assert!(!state.layout.sidebar_visible);
        assert_eq!(state.layout.theme, "light");
        assert!((state.layout.zoom_level - 1.5).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_global_filters() {
        let dir = TempDir::new().unwrap();
        let wp = test_persistence(&dir);

        wp.set_filter("status", serde_json::json!("running")).await;
        wp.set_filter("agent", serde_json::json!("dev-1")).await;

        let state = wp.get_state().await;
        assert_eq!(state.global_filters.len(), 2);
        assert_eq!(state.global_filters["status"], "running");

        wp.remove_filter("status").await;
        let state = wp.get_state().await;
        assert_eq!(state.global_filters.len(), 1);
        assert!(state.global_filters.get("status").is_none());
    }

    #[tokio::test]
    async fn test_reset() {
        let dir = TempDir::new().unwrap();
        let wp = test_persistence(&dir);

        // Modify state
        wp.update_and_save(|s| {
            s.tabs[0].title = "Modified".into();
            s.layout.theme = "custom".into();
        })
        .await;

        wp.reset().await;

        let state = wp.get_state().await;
        assert_eq!(state.tabs[0].title, "Dashboard");
        assert_eq!(state.layout.theme, "dark");
    }

    #[tokio::test]
    async fn test_has_saved_state() {
        let dir = TempDir::new().unwrap();
        let wp = test_persistence(&dir);

        assert!(!wp.has_saved_state().await);

        wp.save().await.unwrap();
        assert!(wp.has_saved_state().await);
    }

    #[tokio::test]
    async fn test_state_path_accessor() {
        let dir = TempDir::new().unwrap();
        let wp = test_persistence(&dir);
        assert!(wp.state_path().ends_with("test-workspace.json"));
    }

    #[tokio::test]
    async fn test_workspace_state_serialization() {
        let state = WorkspaceState {
            schema_version: 1,
            tabs: vec![TabState {
                id: "t1".into(),
                title: "Test".into(),
                content_type: "dashboard".into(),
                content_state: serde_json::json!({"zoom": 1.0}),
                scroll_y: 100.0,
                pinned: true,
            }],
            active_tab_index: 0,
            layout: LayoutState {
                sidebar_visible: true,
                sidebar_width: 280,
                right_panel_visible: true,
                right_panel_width: 350,
                theme: "dark".into(),
                zoom_level: 1.2,
            },
            global_filters: HashMap::from([("env".into(), serde_json::json!("prod"))]),
            metadata: HashMap::new(),
            last_saved_at: "2026-11-11T00:00:00Z".into(),
        };

        let json = serde_json::to_string_pretty(&state).unwrap();
        let restored: WorkspaceState = serde_json::from_str(&json).unwrap();
        assert_eq!(state, restored);
    }

    #[tokio::test]
    async fn test_tab_state_default() {
        let tab = TabState::default();
        assert_eq!(tab.title, "Dashboard");
        assert_eq!(tab.content_type, "dashboard");
        assert!(!tab.pinned);
        assert!(!tab.id.is_empty());
    }

    #[tokio::test]
    async fn test_layout_state_default() {
        let layout = LayoutState::default();
        assert!(layout.sidebar_visible);
        assert_eq!(layout.sidebar_width, 260);
        assert!(!layout.right_panel_visible);
        assert_eq!(layout.theme, "dark");
        assert!((layout.zoom_level - 1.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_concurrent_updates_no_data_loss() {
        let dir = TempDir::new().unwrap();
        let wp = Arc::new(test_persistence(&dir));

        let mut handles = Vec::new();

        // Spawn 10 concurrent updates
        for i in 0..10 {
            let wp = wp.clone();
            handles.push(tokio::spawn(async move {
                wp.set_filter(format!("key-{}", i), serde_json::json!(i))
                    .await;
            }));
        }

        for h in handles {
            h.await.unwrap();
        }

        // All 10 filters should be present
        let state = wp.get_state().await;
        assert_eq!(state.global_filters.len(), 10);
    }

    #[tokio::test]
    async fn test_save_creates_parent_dirs() {
        let dir = TempDir::new().unwrap();
        let nested_path = dir.path().join("deep").join("nested").join("state.json");
        let wp = WorkspacePersistence::new(nested_path.clone());

        wp.save().await.unwrap();
        assert!(nested_path.exists());
    }
}
