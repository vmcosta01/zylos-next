//! Loader: filesystem loaders for runbooks and playbooks from YAML/Markdown specs.

use crate::error::{Result, SoarError};
use crate::playbook::Playbook;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::fs;
use std::path::{Path, PathBuf};

type HmacSha256 = Hmac<Sha256>;

/// A runbook specification parsed from YAML.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunbookSpec {
    pub metadata: RunbookMetadata,
    #[serde(default)]
    pub preconditions: Vec<Precondition>,
    #[serde(default)]
    pub steps: Vec<RunbookStep>,
    #[serde(default)]
    pub validation: Vec<ValidationCheck>,
    #[serde(default)]
    pub rollback: Vec<RollbackStep>,
    /// F18: assinatura HMAC-SHA256 do conteúdo canônico do runbook.
    /// Obrigatória quando ZYLOS_RUNBOOK_KEY está definida.
    #[serde(default)]
    pub signature: Option<String>,
}

/// Verifica a assinatura HMAC-SHA256 de um runbook usando uma chave explícita.
///
/// O payload assinado é o YAML completo **sem** o campo `signature`,
/// normalizado com finais de linha LF.
/// - `key`: `Some(bytes)` exige assinatura válida; `None` pula a verificação (modo dev).
/// - `provided`: assinatura presente no YAML.
fn verify_runbook_signature_with_key(
    content: &str,
    provided: &Option<String>,
    key: Option<&[u8]>,
) -> Result<()> {
    let Some(key) = key else {
        return Ok(()); // sem chave = sem exigência (dev/local)
    };
    let sig = provided
        .as_deref()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| SoarError::UnsignedRunbook("assinatura ausente".into()))?;

    // Remove o campo `signature:` do YAML para calcular o HMAC sobre o resto.
    let canonical: String = content
        .lines()
        .filter(|line| !line.trim_start().starts_with("signature:"))
        .collect::<Vec<_>>()
        .join("\n");

    let mut mac = HmacSha256::new_from_slice(key)
        .map_err(|e| SoarError::UnsignedRunbook(format!("chave inválida: {e}")))?;
    mac.update(canonical.as_bytes());
    let expected = hex::encode(mac.finalize().into_bytes());

    if expected != sig {
        return Err(SoarError::UnsignedRunbook(
            "assinatura HMAC-SHA256 não confere".into(),
        ));
    }
    Ok(())
}

/// Wrapper de produção: lê `ZYLOS_RUNBOOK_KEY` da env e delega para
/// [`verify_runbook_signature_with_key`]. Sem a env, pula a verificação.
fn verify_runbook_signature(content: &str, provided: &Option<String>) -> Result<()> {
    let key = std::env::var("ZYLOS_RUNBOOK_KEY")
        .ok()
        .filter(|k| !k.is_empty());
    verify_runbook_signature_with_key(content, provided, key.as_deref().map(|s| s.as_bytes()))
}

/// Runbook metadata section.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunbookMetadata {
    pub name: String,
    pub version: String,
    pub trigger: String,
    #[serde(default)]
    pub audience: Vec<String>,
    #[serde(default)]
    pub estimated_duration_minutes: u32,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub related_playbooks: Vec<String>,
    #[serde(default)]
    pub playbook_ref: Option<String>,
    #[serde(default)]
    pub runbook_refs: Vec<String>,
}

/// Precondition check before runbook execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Precondition {
    pub check: String,
    pub command: String,
    #[serde(default)]
    pub expected: Option<String>,
    #[serde(default)]
    pub on_failure: Option<String>,
}

/// A single step in a runbook execution pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunbookStep {
    pub id: String,
    pub action: String,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub timeout_seconds: Option<u32>,
    #[serde(default)]
    pub retry: Option<RetryConfig>,
    #[serde(default)]
    pub on_success: Option<String>,
    #[serde(default)]
    pub on_failure: Option<String>,
    #[serde(default)]
    pub human_approval_required: bool,
}

/// Retry configuration for a runbook step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub delay_seconds: u32,
}

/// Validation check after runbook execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationCheck {
    pub check: String,
    pub command: String,
    #[serde(default)]
    pub expected: Option<String>,
    #[serde(default)]
    pub on_failure: Option<String>,
}

/// Rollback step for undoing a runbook action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackStep {
    pub id: String,
    pub action: String,
    pub command: String,
    #[serde(default)]
    pub timeout_seconds: Option<u32>,
    #[serde(default)]
    pub on_failure: Option<String>,
}

/// Load all runbook specs from a directory (YAML files matching *.runbook.yaml).
pub fn load_runbook_specs(dir: &Path) -> Result<Vec<RunbookSpec>> {
    if !dir.exists() {
        return Err(SoarError::Config(format!(
            "Runbook directory does not exist: {}",
            dir.display()
        )));
    }

    let mut specs = Vec::new();
    let entries = fs::read_dir(dir)
        .map_err(|e| SoarError::Config(format!("Failed to read runbook directory: {}", e)))?;

    for entry in entries.flatten() {
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("yaml") {
            if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                if filename.ends_with(".runbook.yaml") || filename.ends_with(".yaml") {
                    match load_single_runbook(&path) {
                        Ok(spec) => specs.push(spec),
                        Err(e) => {
                            eprintln!("Warning: failed to load runbook {}: {}", path.display(), e);
                        }
                    }
                }
            }
        }
    }

    Ok(specs)
}

/// Load a single runbook spec from a YAML file.
pub fn load_single_runbook(path: &Path) -> Result<RunbookSpec> {
    let content = fs::read_to_string(path).map_err(|e| {
        SoarError::Config(format!(
            "Failed to read runbook file {}: {}",
            path.display(),
            e
        ))
    })?;

    let spec: RunbookSpec = serde_yaml::from_str(&content).map_err(|e| {
        SoarError::Config(format!(
            "Failed to parse runbook YAML {}: {}",
            path.display(),
            e
        ))
    })?;

    // F18: verifica assinatura HMAC-SHA256 quando ZYLOS_RUNBOOK_KEY está definida.
    verify_runbook_signature(&content, &spec.signature)?;

    Ok(spec)
}

/// Gera a assinatura HMAC-SHA256 de um conteúdo YAML de runbook.
/// Útil para ferramentas que assinam runbooks offline.
pub fn sign_runbook_content(content: &str, key: &[u8]) -> Result<String> {
    let canonical: String = content
        .lines()
        .filter(|line| !line.trim_start().starts_with("signature:"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut mac = HmacSha256::new_from_slice(key)
        .map_err(|e| SoarError::UnsignedRunbook(format!("chave inválida: {e}")))?;
    mac.update(canonical.as_bytes());
    Ok(hex::encode(mac.finalize().into_bytes()))
}

/// Load all playbooks from a directory (YAML files).
pub fn load_playbooks(dir: &Path) -> Result<Vec<Playbook>> {
    if !dir.exists() {
        return Err(SoarError::Config(format!(
            "Playbook directory does not exist: {}",
            dir.display()
        )));
    }

    let mut playbooks = Vec::new();
    let entries = fs::read_dir(dir)
        .map_err(|e| SoarError::Config(format!("Failed to read playbook directory: {}", e)))?;

    for entry in entries.flatten() {
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("yaml") {
            match load_single_playbook(&path) {
                Ok(playbook) => playbooks.push(playbook),
                Err(e) => {
                    eprintln!("Warning: failed to load playbook {}: {}", path.display(), e);
                }
            }
        }
    }

    Ok(playbooks)
}

/// Load a single playbook from a YAML file.
pub fn load_single_playbook(path: &Path) -> Result<Playbook> {
    let content = fs::read_to_string(path).map_err(|e| {
        SoarError::Config(format!(
            "Failed to read playbook file {}: {}",
            path.display(),
            e
        ))
    })?;

    Playbook::from_yaml(&content)
}

/// Discover runbook specs in common project locations.
pub fn discover_runbook_dirs() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    let candidates = ["docs/soar/runbooks", "runbooks", "docs/runbooks"];

    for candidate in &candidates {
        let p = PathBuf::from(candidate);
        if p.exists() && p.is_dir() {
            paths.push(p);
        }
    }

    paths
}

/// Discover playbook specs in common project locations.
pub fn discover_playbook_dirs() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    let candidates = ["playbooks", "docs/soar/playbooks"];

    for candidate in &candidates {
        let p = PathBuf::from(candidate);
        if p.exists() && p.is_dir() {
            paths.push(p);
        }
    }

    paths
}

/// Load all runbook specs from all discovered directories.
pub fn load_all_runbook_specs() -> Vec<RunbookSpec> {
    let mut all_specs = Vec::new();
    for dir in discover_runbook_dirs() {
        if let Ok(specs) = load_runbook_specs(&dir) {
            all_specs.extend(specs);
        }
    }
    all_specs
}

/// Load all playbooks from all discovered directories.
pub fn load_all_playbooks() -> Vec<Playbook> {
    let mut all_playbooks = Vec::new();
    for dir in discover_playbook_dirs() {
        if let Ok(playbooks) = load_playbooks(&dir) {
            all_playbooks.extend(playbooks);
        }
    }
    all_playbooks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_runbook_spec() {
        let yaml = r#"
metadata:
  name: test-runbook
  version: "1.0"
  trigger: "test_trigger"
  audience: ["dev_pleno"]
  estimated_duration_minutes: 10
  tags: ["test"]
preconditions:
  - check: "Test check"
    command: "echo test"
steps:
  - id: step1
    action: execute
    command: "echo step1"
    timeout_seconds: 30
validation: []
rollback: []
"#;

        let spec: RunbookSpec = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(spec.metadata.name, "test-runbook");
        assert_eq!(spec.metadata.version, "1.0");
        assert_eq!(spec.preconditions.len(), 1);
        assert_eq!(spec.steps.len(), 1);
    }

    #[test]
    fn f18_signature_roundtrip_valid() {
        let yaml = "metadata:\n  name: signed\n  version: \"1\"\n  trigger: t\nsteps: []\nvalidation: []\nrollback: []\npreconditions: []\n";
        let key = b"f18-test-key-32bytes-long-ok!!";
        let sig = sign_runbook_content(yaml, key).unwrap();
        let with_sig = format!("{}signature: {}\n", yaml, sig);
        let spec: RunbookSpec = serde_yaml::from_str(&with_sig).unwrap();
        assert!(verify_runbook_signature_with_key(
            &with_sig,
            &spec.signature,
            Some(key.as_slice())
        )
        .is_ok());
    }

    #[test]
    fn f18_missing_signature_rejected_when_key_set() {
        let yaml = "metadata:\n  name: unsigned\n  version: \"1\"\n  trigger: t\nsteps: []\nvalidation: []\nrollback: []\npreconditions: []\n";
        let key = b"f18-test-key-32bytes-long-ok!!";
        let spec: RunbookSpec = serde_yaml::from_str(yaml).unwrap();
        let err = verify_runbook_signature_with_key(yaml, &spec.signature, Some(key.as_slice()))
            .unwrap_err();
        assert!(matches!(err, SoarError::UnsignedRunbook(_)));
    }

    #[test]
    fn f18_wrong_signature_rejected() {
        let yaml = "metadata:\n  name: bad-sig\n  version: \"1\"\n  trigger: t\nsteps: []\nvalidation: []\nrollback: []\npreconditions: []\nsignature: deadbeef\n";
        let key = b"f18-test-key-32bytes-long-ok!!";
        let spec: RunbookSpec = serde_yaml::from_str(yaml).unwrap();
        let err = verify_runbook_signature_with_key(yaml, &spec.signature, Some(key.as_slice()))
            .unwrap_err();
        assert!(matches!(err, SoarError::UnsignedRunbook(_)));
    }

    #[test]
    fn f18_no_key_skips_verification() {
        let yaml = "metadata:\n  name: no-key\n  version: \"1\"\n  trigger: t\nsteps: []\nvalidation: []\nrollback: []\npreconditions: []\n";
        let spec: RunbookSpec = serde_yaml::from_str(yaml).unwrap();
        // key=None → modo dev, runbook sem assinatura passa
        assert!(verify_runbook_signature_with_key(yaml, &spec.signature, None).is_ok());
    }
}
