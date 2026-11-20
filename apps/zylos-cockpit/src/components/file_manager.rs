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
    // Sobe a cada upload/exclusão para a lista ser relida do gateway.
    let reload = RwSignal::new(0u32);
    // Último erro de leitura/upload/exclusão, em palavras que dizem o que fazer.
    let erro = RwSignal::new(None::<String>);
    let input_ref = NodeRef::<leptos::html::Input>::new();

    Effect::new(move |_| {
        let _ = reload.get();
        spawn_local(async move {
            loading.set(true);
            match fetch_files().await {
                Ok(data) => {
                    files.set(data);
                    erro.set(None);
                }
                Err(e) => erro.set(Some(crate::ui_error::report("arquivos", &e))),
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
    <h3 class="file-manager-title"><i class="ph ph-folder-simple" aria-hidden="true"></i>" Gerenciador de Arquivos"</h3>
    <input
    type="file"
    node_ref=input_ref
    class="file-input-hidden"
    hidden
    on:change=move |_| {
    let Some(input) = input_ref.get() else { return };
    let Some(file) = input.files().and_then(|l| l.get(0)) else { return };
    input.set_value("");
    let pasta = current_folder.get_untracked();
    spawn_local(async move {
    match upload_file(&pasta, file).await {
    Ok(()) => {
    erro.set(None);
    reload.update(|n| *n += 1);
    }
    Err(e) => erro.set(Some(e)),
    }
    });
    }
    />
    <button
    class="file-upload-btn"
    on:click=move |_| {
    if let Some(input) = input_ref.get() {
    input.click();
    }
    }
    title="Upload arquivo"
    >
    <i class="ph ph-upload-simple" aria-hidden="true"></i>" Upload"
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

    {move || erro.get().map(|e| view! { <div class="file-error" role="alert">{e}</div> })}

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
    "Nenhum arquivo nesta pasta. Use Upload para guardar um."
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
    // Só a data (AAAA-MM-DD): o gateway manda o instante completo em RFC 3339.
    let date: String = file.uploaded_at.chars().take(10).collect();
    let previewable = file.is_previewable();
    let (pasta_dl, nome_dl) = (file.folder.clone(), file.name.clone());
    let (pasta_rm, nome_rm) = (file.folder.clone(), file.name.clone());

    view! {
    <div class="file-card" class:file-previewable=previewable>
    <div class="file-icon">{icon}</div>
    <div class="file-info">
    <span class="file-name">{name}</span>
    <span class="file-meta">{size}{" • "}{date}</span>
    </div>
    <div class="file-actions">
    <button
    class="file-action-btn"
    title="Baixar"
    aria-label=format!("Baixar {}", nome_dl)
    on:click=move |_| {
    let (p, n) = (pasta_dl.clone(), nome_dl.clone());
    spawn_local(async move {
    if let Err(e) = download_file(&p, &n).await {
    erro.set(Some(e));
    }
    });
    }
    >
    <i class="ph ph-download-simple" aria-hidden="true"></i>
    </button>
    <button
    class="file-action-btn"
    title="Apagar"
    aria-label=format!("Apagar {}", nome_rm)
    on:click=move |_| {
    let (p, n) = (pasta_rm.clone(), nome_rm.clone());
    let ok = web_sys::window()
    .and_then(|w| w.confirm_with_message(&format!("Apagar {n}? Não dá para desfazer.")).ok())
    .unwrap_or(false);
    if !ok {
    return;
    }
    spawn_local(async move {
    match delete_file(&p, &n).await {
    Ok(()) => reload.update(|c| *c += 1),
    Err(e) => erro.set(Some(e)),
    }
    });
    }
    >
    <i class="ph ph-trash" aria-hidden="true"></i>
    </button>
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

#[derive(Deserialize)]
struct FilesResp {
    files: Vec<FileInfo>,
}

async fn fetch_files() -> Result<Vec<FileInfo>, String> {
    let r: FilesResp = crate::gateway_http::get_json("/api/v1/files").await?;
    Ok(r.files)
}

/// `?folder=..&name=..` com os dois valores codificados.
fn alvo(folder: &str, name: &str) -> String {
    let enc = |t: &str| String::from(js_sys::encode_uri_component(t));
    format!("folder={}&name={}", enc(folder), enc(name))
}

/// Mensagem em palavras para o status que o gateway devolveu.
fn erro_de_status(status: u16, acao: &str) -> String {
    match status {
        409 => "Já existe um arquivo com esse nome nesta pasta.".to_string(),
        413 => "O arquivo passa de 10 MB.".to_string(),
        422 => "Nome inválido: use letras sem acento, dígitos, ponto, hífen, sublinhado e espaço."
            .to_string(),
        404 => "Arquivo não encontrado (talvez já tenha sido apagado).".to_string(),
        401 | 403 => "Sem permissão: entre de novo no cockpit.".to_string(),
        _ => format!("Não foi possível {acao} (erro {status})."),
    }
}

async fn upload_file(folder: &str, file: web_sys::File) -> Result<(), String> {
    let url =
        crate::gateway_http::api_url(&format!("/api/v1/files?{}", alvo(folder, &file.name())))?;
    let resp = crate::auth::authed(gloo_net::http::Request::post(&url))
        .body(file)
        .map_err(|e| format!("Não foi possível enviar: {e}"))?
        .send()
        .await
        .map_err(|e| format!("Sem conexão com o gateway: {e}"))?;
    if resp.status() == 201 {
        Ok(())
    } else {
        Err(erro_de_status(resp.status(), "enviar o arquivo"))
    }
}

async fn delete_file(folder: &str, name: &str) -> Result<(), String> {
    let url = crate::gateway_http::api_url(&format!("/api/v1/files?{}", alvo(folder, name)))?;
    let resp = crate::auth::authed(gloo_net::http::Request::delete(&url))
        .send()
        .await
        .map_err(|e| format!("Sem conexão com o gateway: {e}"))?;
    if resp.status() == 204 {
        Ok(())
    } else {
        Err(erro_de_status(resp.status(), "apagar o arquivo"))
    }
}

/// Baixa pelo fetch autenticado (um link simples não leva o token) e aciona o download.
async fn download_file(folder: &str, name: &str) -> Result<(), String> {
    use wasm_bindgen::JsCast;
    let url =
        crate::gateway_http::api_url(&format!("/api/v1/files/content?{}", alvo(folder, name)))?;
    let resp = crate::auth::authed(gloo_net::http::Request::get(&url))
        .send()
        .await
        .map_err(|e| format!("Sem conexão com o gateway: {e}"))?;
    if !resp.ok() {
        return Err(erro_de_status(resp.status(), "baixar o arquivo"));
    }
    let bytes = resp
        .binary()
        .await
        .map_err(|e| format!("Falha ao ler o arquivo: {e}"))?;
    let array = js_sys::Uint8Array::from(bytes.as_slice());
    let parts = js_sys::Array::of1(&array);
    let blob = web_sys::Blob::new_with_u8_array_sequence(&parts)
        .map_err(|_| "Não foi possível montar o arquivo.".to_string())?;
    let obj = web_sys::Url::create_object_url_with_blob(&blob)
        .map_err(|_| "Não foi possível montar o arquivo.".to_string())?;
    let doc = web_sys::window()
        .and_then(|w| w.document())
        .ok_or("Sem documento.")?;
    let a: web_sys::HtmlAnchorElement = doc
        .create_element("a")
        .map_err(|_| "Sem documento.".to_string())?
        .dyn_into()
        .map_err(|_| "Sem documento.".to_string())?;
    a.set_href(&obj);
    a.set_download(name);
    a.click();
    let _ = web_sys::Url::revoke_object_url(&obj);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mensagens_de_erro_dizem_o_que_fazer() {
        assert!(erro_de_status(409, "x").contains("Já existe"));
        assert!(erro_de_status(413, "x").contains("10 MB"));
        assert!(erro_de_status(422, "x").contains("Nome inválido"));
        assert_eq!(
            erro_de_status(500, "enviar"),
            "Não foi possível enviar (erro 500)."
        );
    }
}
