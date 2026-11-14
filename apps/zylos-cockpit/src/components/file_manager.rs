//! FileManager — gerenciador visual de arquivos com upload, preview e organização
//!
//! Inspirado em: OpenMuse (files panel), OpenMausBot (file browser).
//! Permite visualizar, organizar e gerenciar arquivos associados a bots/spaces.
//!
//! ## Features
//! - Listagem de arquivos com ícones por tipo
//! - Upload via drag-and-drop ou botão
//! - Preview inline para imagens/texto/markdown
//! - Organização por pastas virtuais
//! - Metadados (tamanho, data, tipo MIME)
//!
//! ## Related Documents
//! - Implementation Plan: [IMPLEMENTATION_PLAN_DETAILED.md](../../docs/IMPLEMENTATION_PLAN_DETAILED.md) §4.3

use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::{Deserialize, Serialize};

/// Um arquivo no gerenciador
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileInfo {
    pub id: String,
    pub name: String,
    pub size_bytes: u64,
    pub mime_type: String,
    pub folder: String,
    pub uploaded_at: String,
    pub bot_id: Option<String>,
}

impl FileInfo {
    pub fn icon(&self) -> &'static str {
        if self.mime_type.starts_with("image/") {
            "🖼️"
        } else if self.mime_type.starts_with("text/") || self.mime_type == "application/json" {
            "📄"
        } else if self.mime_type == "application/pdf" {
            "📕"
        } else if self.mime_type.contains("zip")
            || self.mime_type.contains("tar")
            || self.mime_type.contains("gz")
        {
            "📦"
        } else if self.mime_type.starts_with("audio/") {
            "🎵"
        } else if self.mime_type.starts_with("video/") {
            "🎬"
        } else {
            "📎"
        }
    }

    pub fn formatted_size(&self) -> String {
        if self.size_bytes < 1024 {
            format!("{} B", self.size_bytes)
        } else if self.size_bytes < 1024 * 1024 {
            format!("{:.1} KB", self.size_bytes as f64 / 1024.0)
        } else if self.size_bytes < 1024 * 1024 * 1024 {
            format!("{:.1} MB", self.size_bytes as f64 / (1024.0 * 1024.0))
        } else {
            format!(
                "{:.1} GB",
                self.size_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
            )
        }
    }

    pub fn is_previewable(&self) -> bool {
        self.mime_type.starts_with("image/")
            || self.mime_type.starts_with("text/")
            || self.mime_type == "application/json"
            || self.mime_type == "text/markdown"
    }
}

#[component]
pub fn FileManager() -> impl IntoView {
    let files = RwSignal::new(Vec::<FileInfo>::new());
    let current_folder = RwSignal::new("root".to_string());
    let loading = RwSignal::new(true);

    // Fetch arquivos ao montar
    Effect::new(move |_| {
        spawn_local(async move {
            loading.set(true);
            if let Ok(data) = fetch_files().await {
                files.set(data);
            }
            loading.set(false);
        });
    });

    let folders = Memo::new(move |_| {
        let all_files = files.get();
        let mut folders: Vec<String> = all_files.iter().map(|f| f.folder.clone()).collect();
        folders.sort();
        folders.dedup();
        if !folders.contains(&"root".to_string()) {
            folders.insert(0, "root".to_string());
        }
        folders
    });

    let filtered_files = Memo::new(move |_| {
        let folder = current_folder.get();
        files
            .get()
            .into_iter()
            .filter(|f| f.folder == folder)
            .collect::<Vec<_>>()
    });

    view! {
    <div class="file-manager">
    <div class="file-manager-header">
    <h3 class="file-manager-title">"📁 Gerenciador de Arquivos"</h3>
    <button
    class="file-upload-btn"
    on:click=move |_| {
    // Placeholder: abrir file picker dialog
    web_sys::console::log_1(&"Upload button clicked — file picker not yet implemented in WASM".into());
    }
    title="Upload arquivo"
    >
    "📤 Upload"
    </button>
    </div>

    // Folder tabs
    <div class="file-folder-tabs">
    <For
    each=move || folders.get()
    key=|f| f.clone()
    children=move |folder| {
    let folder_clone_for_click = folder.clone();
    let folder_name = folder.clone();
    let current = current_folder;
    let is_active = move || current.get() == folder_name;
    let css = if is_active() {
    "folder-tab folder-tab-active".to_string()
    } else {
    "folder-tab".to_string()
    };

    view! {
    <button
    class=css
    on:click=move |_| current.set(folder_clone_for_click.clone())
    >
    {folder.clone()}
    </button>
    }
    }
    />
    </div>

    // File list
    {move || {
    if loading.get() {
    view! {
    <div class="file-loading">
    <span class="spinner"></span>
    <span>"Carregando arquivos..."</span>
    </div>
    }.into_any()
    } else {
    let items = filtered_files.get();
    if items.is_empty() {
    view! {
    <div class="file-empty">
    "Nenhum arquivo nesta pasta. Clique em Upload para adicionar."
    </div>
    }.into_any()
    } else {
    view! {
    <div class="file-grid">
    <For
    each=move || filtered_files.get()
    key=|f| f.id.clone()
    children=move |file| {
    let icon = file.icon().to_string();
    let name = file.name.clone();
    let size = file.formatted_size();
    let date = file.uploaded_at.clone();
    let previewable = file.is_previewable();

    view! {
    <div class="file-card" class:file-previewable=previewable>
    <div class="file-icon">{icon}</div>
    <div class="file-info">
    <span class="file-name">{name}</span>
    <span class="file-meta">{size}{" • "}{date}</span>
    </div>
    </div>
    }
    }
    />
    </div>
    }.into_any()
    }
    }
    }}
    </div>
    }
}

// === API Function ===

async fn fetch_files() -> Result<Vec<FileInfo>, String> {
    // Placeholder: arquivos virão do backend quando file storage estiver implementado
    Ok(vec![
        FileInfo {
            id: "file-1".into(),
            name: "architecture-diagram.png".into(),
            size_bytes: 245_760,
            mime_type: "image/png".into(),
            folder: "root".into(),
            uploaded_at: "2026-11-11".into(),
            bot_id: Some("bot-1".into()),
        },
        FileInfo {
            id: "file-2".into(),
            name: "api-spec.json".into(),
            size_bytes: 12_340,
            mime_type: "application/json".into(),
            folder: "root".into(),
            uploaded_at: "2026-11-10".into(),
            bot_id: None,
        },
        FileInfo {
            id: "file-3".into(),
            name: "meeting-notes.md".into(),
            size_bytes: 3_456,
            mime_type: "text/markdown".into(),
            folder: "docs".into(),
            uploaded_at: "2026-11-09".into(),
            bot_id: Some("bot-2".into()),
        },
        FileInfo {
            id: "file-4".into(),
            name: "backup-2026-11.zip".into(),
            size_bytes: 52_428_800,
            mime_type: "application/zip".into(),
            folder: "backups".into(),
            uploaded_at: "2026-11-08".into(),
            bot_id: None,
        },
        FileInfo {
            id: "file-5".into(),
            name: "demo-recording.mp4".into(),
            size_bytes: 157_286_400,
            mime_type: "video/mp4".into(),
            folder: "media".into(),
            uploaded_at: "2026-11-07".into(),
            bot_id: Some("bot-1".into()),
        },
    ])
}
