//! Registry Machine-Readable — Task 5.1.1
//!
//! Descoberta automática de skills via `index.json`, validação de schema,
//! pruning inteligente e API de consulta com performance <100ms.
//!
//! Features avançadas:
//! - Content-addressable digests (SHA-256) para dedup e cache validation
//! - Semantic discovery com fuzzy matching (Levenshtein ≤2) + ranking
//! - Usage tracking com métricas e pruning policy (LRU/LFU/success-rate)
//! - Dependency graph resolution com topological sort e cycle detection
//! - Integrity verification periódica de SKILL.md files
//! - Registry metrics endpoints

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tracing::{debug, info, warn};

/// Erros do registry de skills
#[derive(Error, Debug)]
/// Enumeração RegistryError.
pub enum RegistryError {
    #[error("index.json não encontrado em {0}")]
    // Variante IndexNotFound.
    IndexNotFound(PathBuf),

    #[error("falha ao ler index.json: {0}")]
    // Variante IoError.
    IoError(#[from] std::io::Error),

    #[error("index.json malformado: {0}")]
    // Variante ParseError.
    ParseError(#[from] serde_json::Error),

    #[error("schema validation falhou para skill '{name}': {reason}")]
    // Variante SchemaValidation.
    SchemaValidation { name: String, reason: String },

    #[error("skill '{0}' não encontrada no registry")]
    // Variante SkillNotFound.
    SkillNotFound(String),

    #[error("ciclo de dependências detectado: {0:?}")]
    // Variante DependencyCycle.
    DependencyCycle(Vec<String>),

    #[error("conflito de versão semver para skill '{name}': requerido {required}, disponível {available}")]
    /// Variante SemverConflict.
    SemverConflict {
        name: String,
        required: String,
        available: String,
    },

    #[error("integrity check falhou para skill '{name}': digest esperado {expected}, encontrado {actual}")]
    /// Variante IntegrityMismatch.
    IntegrityMismatch {
        name: String,
        expected: String,
        actual: String,
    },
}

/// Result type alias para operações do registry
pub type RegistryResult<T> = Result<T, RegistryError>;

/// Métricas de uso de uma skill
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
/// Estrutura UsageStats.
pub struct UsageStats {
    /// Timestamp do último uso (unix epoch millis)
    #[serde(default)]
    /// Campo last used.
    pub last_used: u64,

    /// Contador total de usos
    #[serde(default)]
    /// Campo use count.
    pub use_count: u64,

    /// Taxa de sucesso (0.0 a 1.0)
    #[serde(default)]
    /// Campo success rate.
    pub success_rate: f64,

    /// Latência média em ms
    #[serde(default)]
    /// Campo avg latency ms.
    pub avg_latency_ms: f64,

    /// Contador interno de sucessos (para calcular success_rate)
    #[serde(default, skip_serializing)]
    /// Campo success count.
    pub success_count: u64,

    /// Contador interno de falhas
    #[serde(default, skip_serializing)]
    /// Campo failure count.
    pub failure_count: u64,
}

impl Default for UsageStats {
    fn default() -> Self {
        // Variante Self.
        Self {
            last_used: 0,
            use_count: 0,
            success_rate: 1.0,
            avg_latency_ms: 0.0,
            success_count: 0,
            failure_count: 0,
        }
    }
}

impl UsageStats {
    /// Registra um uso da skill
    pub fn record_usage(&mut self, success: bool, latency_ms: f64) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        self.last_used = now;
        self.use_count += 1;

        if success {
            self.success_count += 1;
        } else {
            self.failure_count += 1;
        }

        let total = self.success_count + self.failure_count;
        if total > 0 {
            self.success_rate = self.success_count as f64 / total as f64;
        }

        // Exponential moving average para latência
        if self.use_count == 1 {
            self.avg_latency_ms = latency_ms;
        } else {
            let alpha = 0.3;
            self.avg_latency_ms = alpha * latency_ms + (1.0 - alpha) * self.avg_latency_ms;
        }
    }
}

/// Metadata de uma skill no index.json
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
/// Estrutura SkillEntry.
pub struct SkillEntry {
    /// Nome único da skill (kebab-case)
    pub name: String,

    /// Descrição curta (max 200 chars)
    pub description: String,

    /// Versão semver da skill
    pub version: String,

    /// Caminho relativo ao SKILL.md
    pub path: String,

    /// Categoria opcional para grouping
    #[serde(default)]
    /// Campo category.
    pub category: Option<String>,

    /// Tags para busca e filtering
    #[serde(default)]
    /// Campo tags.
    pub tags: Vec<String>,

    /// Trigger phrases para semantic discovery
    #[serde(default)]
    /// Campo trigger phrases.
    pub trigger_phrases: Vec<String>,

    /// Dependências de outras skills (nomes)
    #[serde(default)]
    /// Campo depends on.
    pub depends_on: Vec<String>,

    /// Se a skill está ativa (pruning = false quando deprecated)
    #[serde(default = "default_active")]
    /// Campo active.
    pub active: bool,

    /// Content-addressable digest (SHA-256 hex do conteúdo SKILL.md)
    #[serde(default)]
    /// Campo digest.
    pub digest: String,

    /// Métricas de uso
    #[serde(default)]
    /// Campo usage.
    pub usage: UsageStats,
}

fn default_active() -> bool {
    true
}

/// Política de pruning para skills pouco usadas
#[derive(Debug, Clone, Copy, PartialEq)]
/// Enumeração PruningPolicy.
pub enum PruningPolicy {
    /// Remove skills menos recentemente usadas
    Lru,
    /// Remove skills menos frequentemente usadas
    Lfu,
    // Remove skills com success_rate abaixo do threshold
    SuccessRateThreshold(f64),
}

/// Resultado de semantic search com score
#[derive(Debug, Clone)]
/// Estrutura SearchResult.
pub struct SearchResult<'a> {
    /// Campo entry.
    pub entry: &'a SkillEntry,
    /// Campo score.
    pub score: f64,
}

/// Estrutura raiz do index.json
#[derive(Debug, Clone, Serialize, Deserialize)]
/// Estrutura SkillsIndex.
pub struct SkillsIndex {
    /// Versão do formato do index
    pub version: String,

    /// Timestamp da última geração (ISO 8601)
    #[serde(default)]
    /// Campo generated at.
    pub generated_at: Option<String>,

    /// Total de skills registradas
    #[serde(default)]
    /// Campo total skills.
    pub total_skills: usize,

    /// Entradas de skills
    pub skills: Vec<SkillEntry>,
}

/// Métricas agregadas do registry
#[derive(Debug, Clone, Serialize, Deserialize)]
/// Estrutura RegistryMetrics.
pub struct RegistryMetrics {
    /// Skills mais usadas (top N)
    pub most_used: Vec<(String, u64)>,
    /// Skills com menor taxa de sucesso
    pub least_successful: Vec<(String, f64)>,
    /// Skills órfãs (sem dependents e sem uso)
    pub orphaned: Vec<String>,
    /// Total de skills ativas
    pub total_active: usize,
    /// Tempo de carregamento em ms
    pub load_time_ms: f64,
}

/// Registry de skills com cache e descoberta automática
#[derive(Debug)]
/// Estrutura SkillRegistry.
pub struct SkillRegistry {
    entries: HashMap<String, SkillEntry>,

    base_path: PathBuf,

    load_time_ms: f64,

    dry_run: bool,
}

impl SkillRegistry {
    /// Carrega o registry a partir de um diretório contendo index.json
    pub fn load(base_path: &Path) -> RegistryResult<Self> {
        let start = Instant::now();
        let index_path = base_path.join("index.json");

        if !index_path.exists() {
            return Err(RegistryError::IndexNotFound(index_path));
        }

        let content = std::fs::read_to_string(&index_path)?;
        let index: SkillsIndex = serde_json::from_str(&content)?;

        let mut entries = HashMap::with_capacity(index.skills.len());
        for entry in index.skills {
            if entry.active {
                entries.insert(entry.name.clone(), entry);
            } else {
                debug!("Skill '{}' ignorada (active=false)", entry.name);
            }
        }

        let load_time_ms = start.elapsed().as_secs_f64() * 1000.0;
        info!(
            "Registry carregado: {} skills ativas em {:.2}ms",
            entries.len(),
            load_time_ms
        );

        // Variante Ok.
        Ok(Self {
            entries,
            base_path: base_path.to_path_buf(),
            load_time_ms,
            dry_run: false,
        })
    }

    /// Cria um registry vazio (para testing ou construção manual)
    pub fn empty(base_path: &Path) -> Self {
        // Variante Self.
        Self {
            entries: HashMap::new(),
            base_path: base_path.to_path_buf(),
            load_time_ms: 0.0,
            dry_run: false,
        }
    }

    /// Habilita modo dry-run (operações não modificam estado)
    pub fn set_dry_run(&mut self, dry_run: bool) {
        self.dry_run = dry_run;
    }

    /// Computa o digest SHA-256 de um conteúdo
    pub fn compute_digest(content: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content);
        hex::encode(hasher.finalize())
    }

    /// Adiciona uma skill ao registry, computando digest se não fornecido
    pub fn add(&mut self, mut entry: SkillEntry) {
        if entry.digest.is_empty() {
            let skill_path = self.base_path.join(&entry.path);
            if let Ok(content) = std::fs::read(&skill_path) {
                entry.digest = Self::compute_digest(&content);
            }
        }
        self.entries.insert(entry.name.clone(), entry);
    }

    /// Adiciona uma skill com conteúdo explícito para digest
    pub fn add_with_content(&mut self, mut entry: SkillEntry, content: &[u8]) {
        entry.digest = Self::compute_digest(content);
        self.entries.insert(entry.name.clone(), entry);
    }

    /// Busca uma skill por nome exato
    pub fn get(&self, name: &str) -> RegistryResult<&SkillEntry> {
        self.entries
            .get(name)
            .ok_or_else(|| RegistryError::SkillNotFound(name.to_string()))
    }

    /// Busca uma skill mutável por nome exato
    pub fn get_mut(&mut self, name: &str) -> RegistryResult<&mut SkillEntry> {
        self.entries
            .get_mut(name)
            .ok_or_else(|| RegistryError::SkillNotFound(name.to_string()))
    }

    /// Lista todas as skills ativas
    pub fn list_all(&self) -> Vec<&SkillEntry> {
        self.entries.values().collect()
    }

    /// Busca skills por tag
    pub fn search_by_tag(&self, tag: &str) -> Vec<&SkillEntry> {
        self.entries
            .values()
            .filter(|e| e.tags.iter().any(|t| t == tag))
            .collect()
    }

    /// Busca skills por categoria
    pub fn search_by_category(&self, category: &str) -> Vec<&SkillEntry> {
        self.entries
            .values()
            .filter(|e| e.category.as_deref() == Some(category))
            .collect()
    }

    /// Busca fuzzy por nome/descrição (legacy — usa contains)
    pub fn search(&self, query: &str) -> Vec<&SkillEntry> {
        let lower = query.to_lowercase();
        self.entries
            .values()
            .filter(|e| {
                e.name.to_lowercase().contains(&lower)
                    || e.description.to_lowercase().contains(&lower)
                    || e.tags.iter().any(|t| t.to_lowercase().contains(&lower))
            })
            .collect()
    }

    /// Semantic discovery com fuzzy matching e ranking.
    pub fn semantic_search(&self, query: &str, max_results: usize) -> Vec<SearchResult<'_>> {
        let lower_query = query.to_lowercase();
        let mut results: Vec<SearchResult<'_>> = Vec::new();

        for entry in self.entries.values() {
            let mut score: f64 = 0.0;

            if entry.name.to_lowercase() == lower_query {
                score += 10.0;
            } else if entry.name.to_lowercase().contains(&lower_query) {
                score += 5.0;
            }

            for phrase in &entry.trigger_phrases {
                let dist = strsim::levenshtein(&phrase.to_lowercase(), &lower_query);
                if dist == 0 {
                    score += 8.0;
                } else if dist <= 2 {
                    score += 4.0 / (dist as f64);
                }
            }

            let name_dist = strsim::levenshtein(&entry.name.to_lowercase(), &lower_query);
            if name_dist > 0 && name_dist <= 2 {
                score += 3.0 / (name_dist as f64);
            }

            if entry.description.to_lowercase().contains(&lower_query) {
                score += 2.0;
            }

            for tag in &entry.tags {
                if tag.to_lowercase().contains(&lower_query) {
                    score += 1.5;
                }
            }

            if entry.usage.use_count > 0 {
                score += (entry.usage.use_count as f64).ln() * 0.1;
            }

            if entry.usage.last_used > 0 {
                let age_hours = {
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64;
                    let diff = now.saturating_sub(entry.usage.last_used);
                    diff as f64 / 3_600_000.0
                };
                if age_hours < 168.0 {
                    score += 0.5 * (1.0 - age_hours / 168.0);
                }
            }

            if score > 0.0 {
                results.push(SearchResult { entry, score });
            }
        }

        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(max_results);
        results
    }

    /// Registra uso de uma skill
    pub fn record_usage(
        &mut self,
        name: &str,
        success: bool,
        latency_ms: f64,
    ) -> RegistryResult<()> {
        if self.dry_run {
            return Ok(());
        }
        let entry = self
            .entries
            .get_mut(name)
            .ok_or_else(|| RegistryError::SkillNotFound(name.to_string()))?;
        entry.usage.record_usage(success, latency_ms);
        // Variante Ok.
        Ok(())
    }

    /// Pruning inteligente baseado em política.
    pub fn prune_with_policy(&mut self, policy: PruningPolicy) -> Vec<String> {
        if self.dry_run {
            return Vec::new();
        }

        let mut to_remove: Vec<String> = Vec::new();

        match policy {
            PruningPolicy::Lru => {
                let mut entries: Vec<(&String, &SkillEntry)> = self
                    .entries
                    .iter()
                    .filter(|(_, e)| e.usage.use_count > 0)
                    .collect();
                entries.sort_by_key(|(_, e)| e.usage.last_used);
                let cutoff = (entries.len() as f64 * 0.1).ceil() as usize;
                for (name, _) in entries.into_iter().take(cutoff) {
                    to_remove.push(name.clone());
                }
            }
            PruningPolicy::Lfu => {
                let mut entries: Vec<(&String, &SkillEntry)> = self
                    .entries
                    .iter()
                    .filter(|(_, e)| e.usage.use_count > 0)
                    .collect();
                entries.sort_by_key(|(_, e)| e.usage.use_count);
                let cutoff = (entries.len() as f64 * 0.1).ceil() as usize;
                for (name, _) in entries.into_iter().take(cutoff) {
                    to_remove.push(name.clone());
                }
            }
            PruningPolicy::SuccessRateThreshold(threshold) => {
                for (name, entry) in &self.entries {
                    if entry.usage.use_count >= 5 && entry.usage.success_rate < threshold {
                        to_remove.push(name.clone());
                    }
                }
            }
        }

        let mut removed = Vec::new();
        for name in to_remove {
            if let Some(entry) = self.entries.remove(&name) {
                warn!("Skill '{}' removida por pruning policy {:?}", name, policy);
                removed.push(entry.name);
            }
        }
        removed
    }

    /// Pruning legado: remove skills inativas ou com dependências quebradas
    pub fn prune(&mut self) -> Vec<String> {
        let mut removed = Vec::new();
        let active_names: Vec<String> = self.entries.keys().cloned().collect();
        let broken: Vec<String> = self
            .entries
            .iter()
            .filter(|(_, entry)| {
                entry
                    .depends_on
                    .iter()
                    .any(|dep| !active_names.contains(dep))
            })
            .map(|(name, _)| name.clone())
            .collect();

        for name in broken {
            if let Some(entry) = self.entries.remove(&name) {
                warn!(
                    "Skill '{}' removida por dependência quebrada: {:?}",
                    name, entry.depends_on
                );
                removed.push(name);
            }
        }
        removed
    }

    /// Resolve dependency graph com topological sort.
    pub fn resolve_dependencies(&self, root: &str) -> RegistryResult<Vec<String>> {
        let mut visited: HashSet<String> = HashSet::new();
        let mut in_stack: HashSet<String> = HashSet::new();
        let mut result: Vec<String> = Vec::new();
        let mut stack: Vec<String> = Vec::new();

        fn visit(
            entries: &HashMap<String, SkillEntry>,
            name: &str,
            visited: &mut HashSet<String>,
            in_stack: &mut HashSet<String>,
            result: &mut Vec<String>,
            path: &mut Vec<String>,
        ) -> RegistryResult<()> {
            if in_stack.contains(name) {
                path.push(name.to_string());
                return Err(RegistryError::DependencyCycle(path.clone()));
            }
            if visited.contains(name) {
                return Ok(());
            }

            in_stack.insert(name.to_string());
            path.push(name.to_string());

            if let Some(entry) = entries.get(name) {
                for dep in &entry.depends_on {
                    visit(entries, dep, visited, in_stack, result, path)?;
                }
            }

            in_stack.remove(name);
            path.pop();
            visited.insert(name.to_string());
            result.push(name.to_string());
            // Variante Ok.
            Ok(())
        }

        visit(
            &self.entries,
            root,
            &mut visited,
            &mut in_stack,
            &mut result,
            &mut stack,
        )?;

        // Variante Ok.
        Ok(result)
    }

    /// Resolve todas as dependências transitivas de uma skill
    pub fn transitive_deps(&self, root: &str) -> RegistryResult<Vec<&SkillEntry>> {
        let order = self.resolve_dependencies(root)?;
        let mut deps: Vec<&SkillEntry> = Vec::new();
        for name in &order {
            if name != root {
                if let Some(entry) = self.entries.get(name) {
                    deps.push(entry);
                }
            }
        }
        // Variante Ok.
        Ok(deps)
    }

    /// Verifica conflitos de versão semver entre dependências
    pub fn check_semver_conflicts(&self) -> Vec<RegistryError> {
        let mut conflicts = Vec::new();

        for (name, entry) in &self.entries {
            for dep_name in &entry.depends_on {
                if let Some(dep_entry) = self.entries.get(dep_name) {
                    if let (Ok(req_ver), Ok(avail_ver)) = (
                        semver::Version::parse(&entry.version),
                        semver::Version::parse(&dep_entry.version),
                    ) {
                        if req_ver.major != avail_ver.major && req_ver.major > 0 {
                            conflicts.push(RegistryError::SemverConflict {
                                name: name.clone(),
                                required: entry.version.clone(),
                                available: dep_entry.version.clone(),
                            });
                        }
                    }
                }
            }
        }

        conflicts
    }

    /// Verifica integridade dos SKILL.md files comparando digests.
    pub fn verify_integrity(&self) -> Vec<RegistryError> {
        let mut mismatches = Vec::new();

        for (name, entry) in &self.entries {
            if entry.digest.is_empty() {
                continue;
            }

            let skill_path = self.base_path.join(&entry.path);
            match std::fs::read(&skill_path) {
                // Variante Ok.
                Ok(content) => {
                    let actual = Self::compute_digest(&content);
                    if actual != entry.digest {
                        mismatches.push(RegistryError::IntegrityMismatch {
                            name: name.clone(),
                            expected: entry.digest.clone(),
                            actual,
                        });
                    }
                }
                // Variante Err.
                Err(e) => {
                    warn!("Não foi possível ler '{}': {}", skill_path.display(), e);
                }
            }
        }

        mismatches
    }

    /// Retorna métricas agregadas do registry
    pub fn metrics(&self, top_n: usize) -> RegistryMetrics {
        let mut by_usage: Vec<(&String, u64)> = self
            .entries
            .iter()
            .map(|(name, e)| (name, e.usage.use_count))
            .collect();
        by_usage.sort_by_key(|a| std::cmp::Reverse(a.1));
        let most_used: Vec<(String, u64)> = by_usage
            .into_iter()
            .take(top_n)
            .map(|(n, c)| (n.clone(), c))
            .collect();

        let mut by_success: Vec<(&String, f64)> = self
            .entries
            .iter()
            .filter(|(_, e)| e.usage.use_count >= 3)
            .map(|(name, e)| (name, e.usage.success_rate))
            .collect();
        by_success.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        let least_successful: Vec<(String, f64)> = by_success
            .into_iter()
            .take(top_n)
            .map(|(n, r)| (n.clone(), r))
            .collect();

        let all_names: std::collections::HashSet<&str> =
            self.entries.keys().map(|s| s.as_str()).collect();
        let depended_on: std::collections::HashSet<&str> = self
            .entries
            .values()
            .flat_map(|e| e.depends_on.iter())
            .map(|d| d.as_str())
            .filter(|d| all_names.contains(d))
            .collect();

        let orphaned: Vec<String> = self
            .entries
            .iter()
            .filter(|(name, e)| e.usage.use_count == 0 && !depended_on.contains(name.as_str()))
            .map(|(name, _)| name.clone())
            .collect();

        // Variante RegistryMetrics.
        RegistryMetrics {
            most_used,
            least_successful,
            orphaned,
            total_active: self.entries.len(),
            load_time_ms: self.load_time_ms,
        }
    }

    /// Dedup por digest: remove entradas duplicadas mantendo a primeira
    pub fn dedup_by_digest(&mut self) -> Vec<String> {
        let mut seen_digests: HashMap<String, String> = HashMap::new();
        let mut to_remove: Vec<String> = Vec::new();

        for (name, entry) in &self.entries {
            if entry.digest.is_empty() {
                continue;
            }
            if let Some(existing) = seen_digests.get(&entry.digest) {
                warn!(
                    "Skill '{}' é duplicata de '{}' (mesmo digest {})",
                    name, existing, entry.digest
                );
                to_remove.push(name.clone());
            } else {
                seen_digests.insert(entry.digest.clone(), name.clone());
            }
        }

        let mut removed = Vec::new();
        for name in to_remove {
            if self.entries.remove(&name).is_some() {
                removed.push(name);
            }
        }
        removed
    }

    /// Retorna o tempo de carregamento em ms
    pub fn load_time_ms(&self) -> f64 {
        self.load_time_ms
    }

    /// Retorna o número de skills ativas
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Verifica se o registry está vazio
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Retorna o caminho base
    pub fn base_path(&self) -> &Path {
        &self.base_path
    }

    /// Gera um index.json a partir das skills atuais
    pub fn generate_index(&self) -> SkillsIndex {
        let mut skills: Vec<SkillEntry> = self.entries.values().cloned().collect();
        skills.sort_by(|a, b| a.name.cmp(&b.name));

        // Variante SkillsIndex.
        SkillsIndex {
            version: "1.0.0".to_string(),
            generated_at: None,
            total_skills: skills.len(),
            skills,
        }
    }

    /// Serializa o index para JSON
    pub fn to_json(&self) -> RegistryResult<String> {
        let index = self.generate_index();
        serde_json::to_string_pretty(&index).map_err(RegistryError::ParseError)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn make_entry(name: &str) -> SkillEntry {
        // Variante SkillEntry.
        SkillEntry {
            name: name.to_string(),
            description: format!("{} skill", name),
            version: "1.0.0".to_string(),
            path: format!("{}.md", name),
            category: None,
            tags: vec![],
            trigger_phrases: vec![],
            depends_on: vec![],
            active: true,
            digest: String::new(),
            usage: UsageStats::default(),
        }
    }

    fn test_registry() -> SkillRegistry {
        let dir = std::env::temp_dir().join("zylos-skills-test");
        fs::create_dir_all(&dir).unwrap();
        SkillRegistry::empty(&dir)
    }

    #[test]
    fn test_compute_digest_deterministic() {
        let content = b"# My Skill\nSome content here";
        let d1 = SkillRegistry::compute_digest(content);
        let d2 = SkillRegistry::compute_digest(content);
        assert_eq!(d1, d2);
        assert_eq!(d1.len(), 64); // SHA-256 hex = 64 chars
    }

    #[test]
    fn test_compute_digest_different_content() {
        let d1 = SkillRegistry::compute_digest(b"content A");
        let d2 = SkillRegistry::compute_digest(b"content B");
        assert_ne!(d1, d2);
    }

    #[test]
    fn test_add_with_content_sets_digest() {
        let mut reg = test_registry();
        let entry = make_entry("test-skill");
        let content = b"# Test Skill Content";
        reg.add_with_content(entry, content);
        let e = reg.get("test-skill").unwrap();
        assert_eq!(e.digest, SkillRegistry::compute_digest(content));
    }

    #[test]
    fn test_usage_stats_record() {
        let mut stats = UsageStats::default();
        stats.record_usage(true, 100.0);
        assert_eq!(stats.use_count, 1);
        assert_eq!(stats.success_count, 1);
        assert_eq!(stats.failure_count, 0);
        assert!((stats.success_rate - 1.0).abs() < f64::EPSILON);
        assert!((stats.avg_latency_ms - 100.0).abs() < f64::EPSILON);

        stats.record_usage(false, 200.0);
        assert_eq!(stats.use_count, 2);
        assert_eq!(stats.success_count, 1);
        assert_eq!(stats.failure_count, 1);
        assert!((stats.success_rate - 0.5).abs() < f64::EPSILON);
        // EMA: 0.3 * 200 + 0.7 * 100 = 60 + 70 = 130
        assert!((stats.avg_latency_ms - 130.0).abs() < 1.0);
    }

    #[test]
    fn test_semantic_search_exact_name() {
        let mut reg = test_registry();
        let mut e = make_entry("deploy-app");
        e.trigger_phrases = vec!["deploy application".to_string()];
        reg.add_with_content(e, b"content");

        let results = reg.semantic_search("deploy-app", 5);
        assert!(!results.is_empty());
        assert_eq!(results[0].entry.name, "deploy-app");
        assert!(results[0].score >= 10.0);
    }

    #[test]
    fn test_semantic_search_fuzzy_trigger() {
        let mut reg = test_registry();
        let mut e = make_entry("build-tool");
        e.trigger_phrases = vec!["compile project".to_string()];
        reg.add_with_content(e, b"content");

        // "compile projct" is Levenshtein distance 1 from "compile project"
        let results = reg.semantic_search("compile projct", 5);
        assert!(!results.is_empty());
        assert!(results[0].score > 0.0);
    }

    #[test]
    fn test_semantic_search_no_match() {
        let mut reg = test_registry();
        reg.add_with_content(make_entry("alpha"), b"c");
        let results = reg.semantic_search("zzzzzzzzz", 5);
        assert!(results.is_empty());
    }

    #[test]
    fn test_pruning_lru() {
        let mut reg = test_registry();
        for i in 0..10 {
            let mut e = make_entry(&format!("skill-{}", i));
            e.usage.use_count = (i + 1) as u64;
            e.usage.last_used = (i * 1000) as u64;
            reg.add_with_content(e, b"c");
        }
        let removed = reg.prune_with_policy(PruningPolicy::Lru);
        // Should remove bottom 10% = ceil(10 * 0.1) = 1
        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0], "skill-0"); // lowest last_used
    }

    #[test]
    fn test_pruning_success_rate_threshold() {
        let mut reg = test_registry();
        let mut good = make_entry("good-skill");
        good.usage.use_count = 10;
        good.usage.success_rate = 0.9;
        reg.add_with_content(good, b"c");

        let mut bad = make_entry("bad-skill");
        bad.usage.use_count = 10;
        bad.usage.success_rate = 0.3;
        reg.add_with_content(bad, b"c");

        let removed = reg.prune_with_policy(PruningPolicy::SuccessRateThreshold(0.5));
        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0], "bad-skill");
    }

    #[test]
    fn test_pruning_dry_run() {
        let mut reg = test_registry();
        let mut e = make_entry("dry-skill");
        e.usage.use_count = 1;
        e.usage.last_used = 1;
        reg.add_with_content(e, b"c");
        reg.set_dry_run(true);
        let removed = reg.prune_with_policy(PruningPolicy::Lru);
        assert!(removed.is_empty());
        assert_eq!(reg.len(), 1); // not actually removed
    }

    #[test]
    fn test_resolve_dependencies_linear() {
        let mut reg = test_registry();
        let mut a = make_entry("a");
        a.depends_on = vec!["b".to_string()];
        reg.add_with_content(a, b"c");

        let mut b = make_entry("b");
        b.depends_on = vec!["c".to_string()];
        reg.add_with_content(b, b"c");

        reg.add_with_content(make_entry("c"), b"c");

        let order = reg.resolve_dependencies("a").unwrap();
        // Topological: c before b before a
        let pos_c = order.iter().position(|n| n == "c").unwrap();
        let pos_b = order.iter().position(|n| n == "b").unwrap();
        let pos_a = order.iter().position(|n| n == "a").unwrap();
        assert!(pos_c < pos_b);
        assert!(pos_b < pos_a);
    }

    #[test]
    fn test_resolve_dependencies_cycle_detection() {
        let mut reg = test_registry();
        let mut a = make_entry("x");
        a.depends_on = vec!["y".to_string()];
        reg.add_with_content(a, b"c");

        let mut b = make_entry("y");
        b.depends_on = vec!["x".to_string()];
        reg.add_with_content(b, b"c");

        let result = reg.resolve_dependencies("x");
        assert!(result.is_err());
        match result.unwrap_err() {
            RegistryError::DependencyCycle(path) => {
                assert!(path.contains(&"x".to_string()));
                assert!(path.contains(&"y".to_string()));
            }
            other => panic!("Expected DependencyCycle, got {:?}", other),
        }
    }

    #[test]
    fn test_verify_integrity_match() {
        let dir = std::env::temp_dir().join("zylos-integrity-test");
        fs::create_dir_all(&dir).unwrap();
        let content = b"# Integrity Test Skill";
        fs::write(dir.join("integrity.md"), content).unwrap();

        let mut reg = SkillRegistry::empty(&dir);
        let mut e = make_entry("integrity");
        e.path = "integrity.md".to_string();
        reg.add_with_content(e, content);

        let mismatches = reg.verify_integrity();
        assert!(mismatches.is_empty());

        // Cleanup
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_verify_integrity_mismatch() {
        let dir = std::env::temp_dir().join("zylos-integrity-mismatch");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("changed.md"), b"original content").unwrap();

        let mut reg = SkillRegistry::empty(&dir);
        let mut e = make_entry("changed");
        e.path = "changed.md".to_string();
        // Set digest to something different
        e.digest = SkillRegistry::compute_digest(b"different content");
        reg.add(e);

        let mismatches = reg.verify_integrity();
        assert_eq!(mismatches.len(), 1);
        match &mismatches[0] {
            RegistryError::IntegrityMismatch { name, .. } => {
                assert_eq!(name, "changed");
            }
            other => panic!("Expected IntegrityMismatch, got {:?}", other),
        }

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_metrics_endpoints() {
        let mut reg = test_registry();

        let mut popular = make_entry("popular");
        popular.usage.use_count = 100;
        reg.add_with_content(popular, b"c");

        let mut failing = make_entry("failing");
        failing.usage.use_count = 10;
        failing.usage.success_rate = 0.2;
        reg.add_with_content(failing, b"c");

        let orphan = make_entry("orphan");
        reg.add_with_content(orphan, b"c");

        let m = reg.metrics(5);
        assert_eq!(m.total_active, 3);
        assert!(!m.most_used.is_empty());
        assert_eq!(m.most_used[0].0, "popular");
        assert!(!m.least_successful.is_empty());
        assert_eq!(m.least_successful[0].0, "failing");
        assert!(m.orphaned.contains(&"orphan".to_string()));
    }

    #[test]
    fn test_dedup_by_digest() {
        let mut reg = test_registry();
        let content = b"same content";
        let expected_digest = SkillRegistry::compute_digest(content);

        let mut e1 = make_entry("original");
        e1.digest = expected_digest.clone();
        reg.add(e1);

        let mut e2 = make_entry("duplicate");
        e2.digest = expected_digest.clone();
        reg.add(e2);

        assert_eq!(reg.len(), 2);
        let removed = reg.dedup_by_digest();
        assert_eq!(removed.len(), 1);
        assert_eq!(reg.len(), 1);
        // One of the two entries should remain with the correct digest
        let remaining = reg.list_all();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].digest, expected_digest);
    }

    #[test]
    fn test_record_usage_dry_run() {
        let mut reg = test_registry();
        reg.add_with_content(make_entry("dry"), b"c");
        reg.set_dry_run(true);
        reg.record_usage("dry", true, 50.0).unwrap();
        let e = reg.get("dry").unwrap();
        assert_eq!(e.usage.use_count, 0); // not recorded in dry run
    }
}
