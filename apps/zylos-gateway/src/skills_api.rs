//! Registro de contratos de skills: guarda e lista o que o Editor de Skills salva, numa pasta do
//! gateway (`ZYLOS_SKILLS_DIR`, padrão `data/skills`), um arquivo JSON por skill.
//!
//! ## Endpoints (sob `/api/v1`)
//! - `GET    /skills/contracts`         lista `{ contracts: [{ name, description, category, updated_at }] }`
//! - `GET    /skills/contracts/:name`   a skill completa, com o `yaml`
//! - `POST   /skills/contracts`         cria ou atualiza `{ name, description, category, yaml }`
//! - `DELETE /skills/contracts/:name`   apaga
//!
//! Ao atualizar uma skill, a versão anterior fica em `<nome>.json.<data-hora>.bak` (as `MAX_BACKUPS`
//! mais recentes): o editor ainda não recarrega os passos de uma skill salva, então salvar de novo
//! pode empobrecer o YAML, e estes arquivos são a rede de segurança (um só `.bak` era sobrescrito
//! pelo segundo Save e o original se perdia). Se a cópia falhar, o Save falha: não se sobrescreve
//! sem reserva. Eles não aparecem na listagem.
//!
//! O gateway só guarda: o YAML é do editor (que o valida na tela); aqui não é executado nem
//! interpretado. Nomes só aceitam letras, dígitos, `.`, `_`, `-` e espaço, sem ponto inicial (o nome
//! vira nome de arquivo); a escrita é atômica (arquivo temporário + renomear); há limite de tamanho
//! e de quantidade.

use std::path::{Path, PathBuf};

use axum::{
    extract::{Path as UrlPath, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};

const MAX_SKILLS: usize = 500;
const MAX_YAML: usize = 64 * 1024;
const MAX_DESC: usize = 2000;
const MAX_CATEGORIA: usize = 60;
const MAX_NOME: usize = 100;
/// Versões anteriores guardadas por skill.
const MAX_BACKUPS: usize = 10;

#[derive(Clone)]
pub struct SkillsState {
    dir: PathBuf,
}

impl SkillsState {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// Pasta de `ZYLOS_SKILLS_DIR` (padrão `data/skills`).
    pub fn do_ambiente() -> Self {
        Self::new(std::env::var("ZYLOS_SKILLS_DIR").unwrap_or_else(|_| "data/skills".to_string()))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Skill {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub yaml: String,
    #[serde(default)]
    pub updated_at: String,
}

/// Nome seguro (vira nome de arquivo): sem barra, sem `..`, sem ponto inicial.
pub fn nome_valido(n: &str) -> bool {
    !n.is_empty()
        && n.chars().count() <= MAX_NOME
        && !n.starts_with('.')
        && !n.ends_with(' ')
        && n.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | ' '))
}

fn caminho(dir: &Path, nome: &str) -> Option<PathBuf> {
    nome_valido(nome).then(|| dir.join(format!("{nome}.json")))
}

fn problema(status: StatusCode, titulo: &str) -> Response {
    (status, Json(serde_json::json!({ "error": titulo }))).into_response()
}

/// Confere o que o cliente mandou; devolve o motivo em palavras se não serve.
pub fn validar(s: &Skill) -> Result<(), &'static str> {
    if !nome_valido(&s.name) {
        return Err("nome inválido: use letras sem acento, dígitos, ponto, hífen, sublinhado e espaço (até 100)");
    }
    if s.description.chars().count() > MAX_DESC {
        return Err("descrição longa demais (até 2000 caracteres)");
    }
    if s.category.chars().count() > MAX_CATEGORIA || s.category.chars().any(char::is_control) {
        return Err("categoria inválida (até 60 caracteres, sem controle)");
    }
    if s.yaml.len() > MAX_YAML {
        return Err("yaml grande demais (até 64 KB)");
    }
    Ok(())
}

fn ler(dir: &Path, nome: &str) -> Option<Skill> {
    let p = caminho(dir, nome)?;
    // Não segue link simbólico.
    if !std::fs::symlink_metadata(&p).ok()?.is_file() {
        return None;
    }
    serde_json::from_slice(&std::fs::read(&p).ok()?).ok()
}

fn listar(dir: &Path) -> Vec<Skill> {
    let Ok(entradas) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut skills: Vec<Skill> = entradas
        .filter_map(Result::ok)
        .filter_map(|e| {
            let nome = e.file_name().into_string().ok()?;
            let base = nome.strip_suffix(".json")?;
            ler(dir, base)
        })
        .collect();
    skills.sort_by_key(|s| s.name.to_lowercase());
    skills.truncate(MAX_SKILLS);
    skills
}

/// Existe uma skill cujo nome é igual a `nome` ignorando maiúsculas, mas não idêntico?
fn colide_por_maiusculas(dir: &Path, nome: &str) -> bool {
    let Ok(entradas) = std::fs::read_dir(dir) else {
        return false;
    };
    entradas
        .filter_map(Result::ok)
        .filter_map(|e| e.file_name().into_string().ok())
        .filter_map(|f| f.strip_suffix(".json").map(str::to_string))
        .any(|existente| existente != nome && existente.to_lowercase() == nome.to_lowercase())
}

/// Copia o arquivo atual para `<nome>.json.<data-hora>.bak` e apaga as reservas mais antigas além
/// de `MAX_BACKUPS`. Erro aqui impede o Save: sem reserva não se sobrescreve.
fn guardar_reserva(dir: &Path, nome: &str, atual: &Path) -> Result<(), String> {
    let prefixo = format!("{nome}.json.");
    let marca = chrono::Utc::now().format("%Y%m%dT%H%M%S%9f");
    std::fs::copy(atual, dir.join(format!("{prefixo}{marca}.bak")))
        .map_err(|e| format!("reserva falhou: {e}"))?;
    let mut reservas: Vec<String> = std::fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|f| f.starts_with(&prefixo) && f.ends_with(".bak"))
        .collect();
    reservas.sort();
    let sobra = reservas.len().saturating_sub(MAX_BACKUPS);
    for velha in reservas.into_iter().take(sobra) {
        let _ = std::fs::remove_file(dir.join(velha));
    }
    Ok(())
}

async fn get_contracts(State(st): State<SkillsState>) -> Response {
    let dir = st.dir.clone();
    match tokio::task::spawn_blocking(move || listar(&dir)).await {
        Ok(skills) => {
            let resumo: Vec<_> = skills
                .into_iter()
                .map(|s| {
                    serde_json::json!({
                        "name": s.name,
                        "description": s.description,
                        "category": s.category,
                        "updated_at": s.updated_at,
                    })
                })
                .collect();
            Json(serde_json::json!({ "contracts": resumo })).into_response()
        }
        Err(_) => problema(
            StatusCode::INTERNAL_SERVER_ERROR,
            "falha ao listar as skills",
        ),
    }
}

async fn get_contract(State(st): State<SkillsState>, UrlPath(nome): UrlPath<String>) -> Response {
    let dir = st.dir.clone();
    match tokio::task::spawn_blocking(move || ler(&dir, &nome)).await {
        Ok(Some(s)) => Json(s).into_response(),
        _ => problema(StatusCode::NOT_FOUND, "skill não encontrada"),
    }
}

async fn post_contract(State(st): State<SkillsState>, Json(mut s): Json<Skill>) -> Response {
    if let Err(m) = validar(&s) {
        return problema(StatusCode::UNPROCESSABLE_ENTITY, m);
    }
    s.updated_at = chrono::Utc::now().to_rfc3339();
    let dir = st.dir.clone();
    let r = tokio::task::spawn_blocking(move || -> Result<bool, String> {
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let destino = caminho(&dir, &s.name).ok_or("nome inválido")?;
        // "Deploy" e "deploy" viram o MESMO arquivo no Windows/macOS: recusa o nome que só difere
        // por maiúsculas de uma skill que já existe, em qualquer sistema (o comportamento não
        // pode depender de onde o gateway roda).
        if colide_por_maiusculas(&dir, &s.name) {
            return Err(
                "colisao: já existe uma skill com esse nome (maiúsculas não diferenciam)".into(),
            );
        }
        let novo = !destino.exists();
        if novo && listar(&dir).len() >= MAX_SKILLS {
            return Err("limite de skills atingido".into());
        }
        // Atualização: guarda a versão anterior numa cópia nova (nunca sobrescreve outra reserva).
        if !novo {
            guardar_reserva(&dir, &s.name, &destino)?;
        }
        // Escrita atômica: arquivo temporário na mesma pasta, depois renomeia por cima.
        let tmp = dir.join(format!(".{}.tmp", uuid::Uuid::new_v4()));
        std::fs::write(
            &tmp,
            serde_json::to_vec_pretty(&s).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &destino).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            e.to_string()
        })?;
        Ok(novo)
    })
    .await;
    match r {
        Ok(Ok(novo)) => {
            let status = if novo {
                StatusCode::CREATED
            } else {
                StatusCode::OK
            };
            (status, Json(serde_json::json!({ "saved": true }))).into_response()
        }
        Ok(Err(e)) if e.starts_with("colisao") => problema(
            StatusCode::CONFLICT,
            "já existe uma skill com esse nome (maiúsculas não diferenciam)",
        ),
        Ok(Err(e)) if e.contains("limite") => {
            problema(StatusCode::CONFLICT, "limite de skills atingido")
        }
        _ => problema(
            StatusCode::INTERNAL_SERVER_ERROR,
            "falha ao guardar a skill",
        ),
    }
}

async fn delete_contract(
    State(st): State<SkillsState>,
    UrlPath(nome): UrlPath<String>,
) -> Response {
    let Some(p) = caminho(&st.dir, &nome) else {
        return problema(StatusCode::UNPROCESSABLE_ENTITY, "nome inválido");
    };
    let r = tokio::task::spawn_blocking(move || -> std::io::Result<()> {
        if !std::fs::symlink_metadata(&p)?.is_file() {
            return Err(std::io::ErrorKind::NotFound.into());
        }
        std::fs::remove_file(&p)
    })
    .await;
    match r {
        Ok(Ok(())) => StatusCode::NO_CONTENT.into_response(),
        Ok(Err(e)) if e.kind() == std::io::ErrorKind::NotFound => {
            problema(StatusCode::NOT_FOUND, "skill não encontrada")
        }
        _ => problema(StatusCode::INTERNAL_SERVER_ERROR, "falha ao apagar a skill"),
    }
}

/// Roteador com estado próprio (vira `Router<()>` para o `.merge`).
pub fn create_skills_router(state: SkillsState) -> Router {
    Router::new()
        .route("/skills/contracts", get(get_contracts).post(post_contract))
        .route(
            "/skills/contracts/:name",
            get(get_contract).delete(delete_contract),
        )
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skill(nome: &str) -> Skill {
        Skill {
            name: nome.into(),
            description: "d".into(),
            category: "c".into(),
            yaml: "name: x\n".into(),
            updated_at: String::new(),
        }
    }

    #[test]
    fn nomes_e_limites() {
        assert!(validar(&skill("Hello World")).is_ok());
        for ruim in ["", ".env", "..", "a/b", "a\\b", "é", "fim "] {
            assert!(validar(&skill(ruim)).is_err(), "{ruim:?}");
        }
        let mut s = skill("ok");
        s.yaml = "x".repeat(MAX_YAML + 1);
        assert!(validar(&s).is_err());
        s.yaml.clear();
        s.description = "x".repeat(MAX_DESC + 1);
        assert!(validar(&s).is_err());
        s.description.clear();
        s.category = "linha\nquebrada".into();
        assert!(validar(&s).is_err());
    }
}
