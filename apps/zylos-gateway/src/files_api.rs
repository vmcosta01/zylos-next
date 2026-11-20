//! Arquivos do cockpit: guarda e lista arquivos numa pasta do gateway (`ZYLOS_FILES_DIR`, padrão
//! `data/files`). Pastas são as subpastas diretas; arquivos soltos ficam na pasta `root`.
//!
//! ## Endpoints (sob `/api/v1`)
//! - `GET    /files`                      lista (até 1000), mais recentes primeiro
//! - `POST   /files?folder=F&name=N`      corpo = bytes do arquivo (até 10 MB); 409 se já existe
//! - `GET    /files/content?folder=F&name=N`  baixa (sempre como anexo, nunca renderizado)
//! - `DELETE /files?folder=F&name=N`      apaga
//!
//! Segurança: nomes de pasta/arquivo só aceitam letras, dígitos, `.`, `_`, `-` e espaço, sem
//! começar por ponto (nada de `..`, barras ou caminho); links simbólicos não são listados nem
//! lidos; a criação é atômica (`create_new`), então nunca sobrescreve; o download vai com
//! `Content-Disposition: attachment`, `nosniff` e tipo de conteúdo de uma lista curta (HTML e
//! desconhecidos viram `application/octet-stream`).

use std::path::{Path, PathBuf};

use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};

/// Maior arquivo aceito.
pub const MAX_FILE_BYTES: usize = 10 * 1024 * 1024;
/// Quantos arquivos a listagem devolve.
const MAX_LIST: usize = 1000;
const MAX_NAME: usize = 100;
/// Nome da pasta que representa a raiz do armazenamento.
pub const ROOT: &str = "root";

#[derive(Clone)]
pub struct FilesState {
    dir: PathBuf,
}

impl FilesState {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// Pasta de `ZYLOS_FILES_DIR` (padrão `data/files`).
    pub fn do_ambiente() -> Self {
        Self::new(std::env::var("ZYLOS_FILES_DIR").unwrap_or_else(|_| "data/files".to_string()))
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FileInfo {
    pub id: String,
    pub name: String,
    pub size_bytes: u64,
    pub mime_type: String,
    pub folder: String,
    pub uploaded_at: String,
    pub bot_id: Option<String>,
}

/// Nome seguro de pasta ou arquivo (ver doc do módulo).
pub fn nome_valido(n: &str) -> bool {
    !n.is_empty()
        && n.chars().count() <= MAX_NAME
        && !n.starts_with('.')
        && !n.ends_with(' ')
        && n.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | ' '))
}

/// Tipo de conteúdo a partir da extensão. Só tipos que o navegador mostra sem executar nada;
/// o resto (inclusive HTML e SVG) é `application/octet-stream`.
pub fn mime_por_extensao(nome: &str) -> &'static str {
    let ext = nome.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase());
    match ext.as_deref() {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("txt" | "log") => "text/plain",
        Some("md") => "text/markdown",
        Some("csv") => "text/csv",
        Some("json") => "application/json",
        Some("pdf") => "application/pdf",
        Some("zip") => "application/zip",
        _ => "application/octet-stream",
    }
}

fn caminho(dir: &Path, folder: &str, name: &str) -> Option<PathBuf> {
    if !nome_valido(name) {
        return None;
    }
    if folder == ROOT {
        return Some(dir.join(name));
    }
    nome_valido(folder).then(|| dir.join(folder).join(name))
}

fn problema(status: StatusCode, titulo: &str) -> Response {
    (status, Json(serde_json::json!({ "error": titulo }))).into_response()
}

#[derive(Debug, Deserialize)]
struct Alvo {
    folder: Option<String>,
    name: String,
}

impl Alvo {
    fn pasta(&self) -> &str {
        self.folder
            .as_deref()
            .filter(|f| !f.is_empty())
            .unwrap_or(ROOT)
    }
}

fn info(folder: &str, name: &str, meta: &std::fs::Metadata) -> FileInfo {
    let quando = meta
        .modified()
        .ok()
        .map(chrono::DateTime::<chrono::Utc>::from)
        .unwrap_or_default();
    FileInfo {
        id: format!("{folder}/{name}"),
        name: name.to_string(),
        size_bytes: meta.len(),
        mime_type: mime_por_extensao(name).to_string(),
        folder: folder.to_string(),
        uploaded_at: quando.to_rfc3339(),
        bot_id: None,
    }
}

/// Lê um diretório e devolve só arquivos comuns com nome válido (sem links simbólicos).
fn arquivos_de(dir: &Path, folder: &str, saida: &mut Vec<(std::time::SystemTime, FileInfo)>) {
    let Ok(entradas) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entradas.filter_map(Result::ok) {
        let Ok(name) = e.file_name().into_string() else {
            continue;
        };
        if !nome_valido(&name) {
            continue;
        }
        // `symlink_metadata` não segue o link: link simbólico não é arquivo comum.
        let Ok(meta) = std::fs::symlink_metadata(e.path()) else {
            continue;
        };
        if meta.is_file() {
            let t = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
            saida.push((t, info(folder, &name, &meta)));
        }
    }
}

fn listar(dir: &Path) -> Vec<FileInfo> {
    let mut todos = Vec::new();
    arquivos_de(dir, ROOT, &mut todos);
    if let Ok(entradas) = std::fs::read_dir(dir) {
        for e in entradas.filter_map(Result::ok) {
            let Ok(nome) = e.file_name().into_string() else {
                continue;
            };
            let eh_pasta = std::fs::symlink_metadata(e.path())
                .map(|m| m.is_dir())
                .unwrap_or(false);
            if eh_pasta && nome_valido(&nome) && nome != ROOT {
                arquivos_de(&e.path(), &nome, &mut todos);
            }
        }
    }
    todos.sort_by_key(|(t, _)| std::cmp::Reverse(*t));
    todos.truncate(MAX_LIST);
    todos.into_iter().map(|(_, f)| f).collect()
}

async fn get_files(State(st): State<FilesState>) -> Response {
    let dir = st.dir.clone();
    match tokio::task::spawn_blocking(move || listar(&dir)).await {
        Ok(files) => Json(serde_json::json!({ "files": files })).into_response(),
        Err(_) => problema(
            StatusCode::INTERNAL_SERVER_ERROR,
            "falha ao listar arquivos",
        ),
    }
}

async fn post_file(
    State(st): State<FilesState>,
    Query(alvo): Query<Alvo>,
    body: Bytes,
) -> Response {
    let Some(destino) = caminho(&st.dir, alvo.pasta(), &alvo.name) else {
        return problema(
            StatusCode::UNPROCESSABLE_ENTITY,
            "nome de pasta ou arquivo inválido",
        );
    };
    let r = tokio::task::spawn_blocking(move || -> std::io::Result<()> {
        use std::io::Write;
        if let Some(pai) = destino.parent() {
            std::fs::create_dir_all(pai)?;
        }
        // `create_new`: falha se existir (inclusive link simbólico): nunca sobrescreve.
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&destino)?;
        f.write_all(&body)
    })
    .await;
    match r {
        Ok(Ok(())) => (
            StatusCode::CREATED,
            Json(serde_json::json!({ "folder": alvo.pasta(), "name": alvo.name })),
        )
            .into_response(),
        Ok(Err(e)) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            problema(StatusCode::CONFLICT, "já existe um arquivo com esse nome")
        }
        _ => problema(
            StatusCode::INTERNAL_SERVER_ERROR,
            "falha ao guardar o arquivo",
        ),
    }
}

async fn get_content(State(st): State<FilesState>, Query(alvo): Query<Alvo>) -> Response {
    let Some(origem) = caminho(&st.dir, alvo.pasta(), &alvo.name) else {
        return problema(
            StatusCode::UNPROCESSABLE_ENTITY,
            "nome de pasta ou arquivo inválido",
        );
    };
    let lido = tokio::task::spawn_blocking(move || -> std::io::Result<Vec<u8>> {
        let meta = std::fs::symlink_metadata(&origem)?;
        if !meta.is_file() {
            return Err(std::io::ErrorKind::NotFound.into());
        }
        std::fs::read(&origem)
    })
    .await;
    match lido {
        Ok(Ok(bytes)) => (
            StatusCode::OK,
            [
                (
                    header::CONTENT_TYPE,
                    mime_por_extensao(&alvo.name).to_string(),
                ),
                (
                    header::CONTENT_DISPOSITION,
                    format!("attachment; filename=\"{}\"", alvo.name),
                ),
                (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_string()),
            ],
            bytes,
        )
            .into_response(),
        Ok(Err(e)) if e.kind() == std::io::ErrorKind::NotFound => {
            problema(StatusCode::NOT_FOUND, "arquivo não encontrado")
        }
        _ => problema(StatusCode::INTERNAL_SERVER_ERROR, "falha ao ler o arquivo"),
    }
}

async fn delete_file(State(st): State<FilesState>, Query(alvo): Query<Alvo>) -> Response {
    let Some(origem) = caminho(&st.dir, alvo.pasta(), &alvo.name) else {
        return problema(
            StatusCode::UNPROCESSABLE_ENTITY,
            "nome de pasta ou arquivo inválido",
        );
    };
    let r = tokio::task::spawn_blocking(move || -> std::io::Result<()> {
        let meta = std::fs::symlink_metadata(&origem)?;
        if !meta.is_file() {
            return Err(std::io::ErrorKind::NotFound.into());
        }
        std::fs::remove_file(&origem)
    })
    .await;
    match r {
        Ok(Ok(())) => StatusCode::NO_CONTENT.into_response(),
        Ok(Err(e)) if e.kind() == std::io::ErrorKind::NotFound => {
            problema(StatusCode::NOT_FOUND, "arquivo não encontrado")
        }
        _ => problema(
            StatusCode::INTERNAL_SERVER_ERROR,
            "falha ao apagar o arquivo",
        ),
    }
}

/// Roteador com estado próprio (vira `Router<()>` para o `.merge`).
pub fn create_files_router(state: FilesState) -> Router {
    Router::new()
        .route(
            "/files",
            get(get_files)
                .post(post_file)
                .delete(delete_file)
                .layer(DefaultBodyLimit::max(MAX_FILE_BYTES)),
        )
        .route("/files/content", get(get_content))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nomes_validos_e_invalidos() {
        for ok in ["a.txt", "relatorio final.pdf", "x_1-2", "docs"] {
            assert!(nome_valido(ok), "{ok}");
        }
        for ruim in [
            "", ".env", "..", "a/b", "a\\b", "C:", "a\u{0}b", "fim ", "é.txt",
        ] {
            assert!(!nome_valido(ruim), "{ruim:?}");
        }
        assert!(!nome_valido(&"x".repeat(MAX_NAME + 1)));
    }

    #[test]
    fn html_e_svg_nao_sao_servidos_como_tal() {
        assert_eq!(mime_por_extensao("a.png"), "image/png");
        assert_eq!(mime_por_extensao("a.PNG"), "image/png");
        assert_eq!(mime_por_extensao("pagina.html"), "application/octet-stream");
        assert_eq!(mime_por_extensao("img.svg"), "application/octet-stream");
        assert_eq!(mime_por_extensao("semextensao"), "application/octet-stream");
    }

    #[test]
    fn caminho_nao_sai_da_pasta() {
        let base = Path::new("/base");
        assert_eq!(
            caminho(base, ROOT, "a.txt"),
            Some(PathBuf::from("/base/a.txt"))
        );
        assert_eq!(
            caminho(base, "docs", "a.txt"),
            Some(PathBuf::from("/base/docs/a.txt"))
        );
        assert_eq!(caminho(base, "..", "a.txt"), None);
        assert_eq!(caminho(base, "docs", "../a.txt"), None);
        assert_eq!(caminho(base, "a/b", "x"), None);
    }
}
