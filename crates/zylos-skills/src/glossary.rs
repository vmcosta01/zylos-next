//! GLOSSARY.md 1st-Class — Task 5.1.3
//!
//! Glossário canônico como artefato de contexto denso, com injeção automática,
//! API de consulta e validação de termos em skills. Inspirado em mattpocock/skills.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;
use tracing::info;

/// Erros do glossário
#[derive(Error, Debug)]
/// Enumeração GlossaryError.
pub enum GlossaryError {
    #[error("glossário não encontrado: {0}")]
    // Variante NotFound.
    NotFound(String),

    #[error("falha ao ler glossário: {0}")]
    // Variante IoError.
    IoError(#[from] std::io::Error),

    #[error("glossário JSON inválido: {0}")]
    // Variante ParseError.
    ParseError(#[from] serde_json::Error),

    #[error("termo '{0}' não encontrado no glossário")]
    // Variante TermNotFound.
    TermNotFound(String),
}

/// Result type alias para operações do glossário
pub type GlossaryResult<T> = Result<T, GlossaryError>;

/// Definição de um termo no glossário
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
/// Estrutura GlossaryTerm.
pub struct GlossaryTerm {
    /// Termo canônico (lowercase, kebab-case)
    pub term: String,

    /// Definição concisa (max 300 chars)
    pub definition: String,

    /// Sinônimos ou aliases para busca
    #[serde(default)]
    /// Campo aliases.
    pub aliases: Vec<String>,

    /// Tags de domínio para grouping
    #[serde(default)]
    /// Campo tags.
    pub tags: Vec<String>,

    /// Referências cruzadas a outros termos
    #[serde(default)]
    /// Campo see also.
    pub see_also: Vec<String>,

    /// Exemplo de uso em contexto
    #[serde(default)]
    /// Campo example.
    pub example: Option<String>,
}

/// Estrutura raiz do GLOSSARY.json
#[derive(Debug, Clone, Serialize, Deserialize)]
/// Estrutura Glossary.
pub struct Glossary {
    /// Versão do glossário
    pub version: String,

    /// Timestamp da última atualização (ISO 8601)
    #[serde(default)]
    /// Campo updated at.
    pub updated_at: Option<String>,

    /// Total de termos
    #[serde(default)]
    /// Campo total terms.
    pub total_terms: usize,

    /// Termos indexados
    pub terms: Vec<GlossaryTerm>,
}

/// Cache de glossário com busca otimizada
#[derive(Debug)]
/// Estrutura GlossaryIndex.
pub struct GlossaryIndex {
    terms: HashMap<String, GlossaryTerm>,

    alias_map: HashMap<String, String>,

    load_time_ms: f64,
}

impl GlossaryIndex {
    /// Carrega o glossário a partir de um arquivo JSON
    pub fn load(path: &Path) -> GlossaryResult<Self> {
        let start = std::time::Instant::now();

        if !path.exists() {
            return Err(GlossaryError::NotFound(path.display().to_string()));
        }

        let content = std::fs::read_to_string(path)?;
        let glossary: Glossary = serde_json::from_str(&content)?;

        let mut terms = HashMap::with_capacity(glossary.terms.len());
        let mut alias_map = HashMap::new();

        for term in glossary.terms {
            let canonical = term.term.to_lowercase();

            // Indexa aliases
            for alias in &term.aliases {
                alias_map.insert(alias.to_lowercase(), canonical.clone());
            }

            terms.insert(canonical, term);
        }

        let load_time_ms = start.elapsed().as_secs_f64() * 1000.0;
        info!(
            "Glossário carregado: {} termos em {:.2}ms",
            terms.len(),
            load_time_ms
        );

        // Variante Ok.
        Ok(Self {
            terms,
            alias_map,
            load_time_ms,
        })
    }

    /// Cria um glossário vazio
    pub fn empty() -> Self {
        // Variante Self.
        Self {
            terms: HashMap::new(),
            alias_map: HashMap::new(),
            load_time_ms: 0.0,
        }
    }

    /// Adiciona um termo ao glossário
    pub fn add(&mut self, term: GlossaryTerm) {
        let canonical = term.term.to_lowercase();
        for alias in &term.aliases {
            self.alias_map
                .insert(alias.to_lowercase(), canonical.clone());
        }
        self.terms.insert(canonical, term);
    }

    /// Busca um termo por nome exato ou alias
    pub fn get(&self, query: &str) -> GlossaryResult<&GlossaryTerm> {
        let lower = query.to_lowercase();

        // Tenta busca direta
        if let Some(term) = self.terms.get(&lower) {
            return Ok(term);
        }

        // Tenta via alias
        if let Some(canonical) = self.alias_map.get(&lower) {
            if let Some(term) = self.terms.get(canonical) {
                return Ok(term);
            }
        }

        // Variante Err.
        Err(GlossaryError::TermNotFound(query.to_string()))
    }

    /// Busca fuzzy por substring em termo/definição/aliases
    pub fn search(&self, query: &str) -> Vec<&GlossaryTerm> {
        let lower = query.to_lowercase();
        self.terms
            .values()
            .filter(|t| {
                t.term.to_lowercase().contains(&lower)
                    || t.definition.to_lowercase().contains(&lower)
                    || t.aliases.iter().any(|a| a.to_lowercase().contains(&lower))
                    || t.tags.iter().any(|tag| tag.to_lowercase().contains(&lower))
            })
            .collect()
    }

    /// Lista todos os termos ordenados alfabeticamente
    pub fn list_all(&self) -> Vec<&GlossaryTerm> {
        let mut terms: Vec<&GlossaryTerm> = self.terms.values().collect();
        terms.sort_by(|a, b| a.term.cmp(&b.term));
        terms
    }

    /// Filtra termos por tag
    pub fn by_tag(&self, tag: &str) -> Vec<&GlossaryTerm> {
        let lower = tag.to_lowercase();
        self.terms
            .values()
            .filter(|t| t.tags.iter().any(|tg| tg.to_lowercase() == lower))
            .collect()
    }

    /// Gera contexto denso para injeção em prompt
    /// Retorna definições compactas dos N termos mais relevantes
    pub fn inject_context(&self, queries: &[&str], max_terms: usize) -> String {
        let mut seen = std::collections::HashSet::new();
        let mut lines = Vec::new();

        for query in queries {
            if let Ok(term) = self.get(query) {
                if seen.insert(&term.term) {
                    lines.push(format!("- **{}**: {}", term.term, term.definition));
                }
            }
            // Também inclui resultados fuzzy
            for term in self.search(query) {
                if seen.insert(&term.term) && lines.len() < max_terms {
                    lines.push(format!("- **{}**: {}", term.term, term.definition));
                }
            }
        }

        if lines.is_empty() {
            return String::new();
        }

        format!("## Glossário\n{}", lines.join("\n"))
    }

    /// Valida se todos os termos técnicos em uma skill estão definidos
    pub fn validate_skill_terms(&self, skill_text: &str) -> Vec<String> {
        let mut undefined = Vec::new();
        let lower_text = skill_text.to_lowercase();

        // Extrai palavras potencialmente técnicas (camelCase, kebab-case, ALL_CAPS)
        let words: Vec<String> = lower_text
            .split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
            .filter(|w| {
                w.len() > 3
                    && (w.contains('-')
                        || w.contains('_')
                        || w.chars().all(|c| c.is_ascii_uppercase()))
            })
            .map(|s| s.to_string())
            .collect();

        for word in words {
            if self.get(&word).is_err() {
                // Não reporta se for palavra comum
                if !COMMON_TERMS.contains(&word.as_str()) {
                    undefined.push(word);
                }
            }
        }

        undefined.sort();
        undefined.dedup();
        undefined
    }

    /// Retorna o tempo de carregamento
    pub fn load_time_ms(&self) -> f64 {
        self.load_time_ms
    }

    /// Número de termos
    pub fn len(&self) -> usize {
        self.terms.len()
    }

    /// Se está vazio
    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }

    /// Serializa para JSON
    pub fn to_json(&self) -> GlossaryResult<String> {
        let mut terms: Vec<GlossaryTerm> = self.terms.values().cloned().collect();
        terms.sort_by(|a, b| a.term.cmp(&b.term));

        let glossary = Glossary {
            version: "1.0.0".to_string(),
            updated_at: None,
            total_terms: terms.len(),
            terms,
        };

        serde_json::to_string_pretty(&glossary).map_err(GlossaryError::ParseError)
    }
}

/// Termos comuns que não precisam estar no glossário
const COMMON_TERMS: &[&str] = &[
    "true", "false", "null", "none", "some", "this", "that", "with", "from", "into", "over",
    "under", "self", "super", "crate", "mod", "pub", "use", "let", "mut", "ref", "fn", "struct",
    "enum", "impl", "trait", "type",
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn sample_term(name: &str) -> GlossaryTerm {
        // Variante GlossaryTerm.
        GlossaryTerm {
            term: name.to_string(),
            definition: format!("Definição do termo {}", name),
            aliases: vec![format!("{}-alias", name)],
            tags: vec!["core".to_string()],
            see_also: vec![],
            example: Some(format!("Exemplo de uso de {}", name)),
        }
    }

    #[test]
    fn test_empty_glossary() {
        let idx = GlossaryIndex::empty();
        assert!(idx.is_empty());
        assert_eq!(idx.len(), 0);
    }

    #[test]
    fn test_add_and_get() {
        let mut idx = GlossaryIndex::empty();
        idx.add(sample_term("vector-store"));

        let term = idx.get("vector-store").unwrap();
        assert_eq!(term.term, "vector-store");
    }

    #[test]
    fn test_get_by_alias() {
        let mut idx = GlossaryIndex::empty();
        idx.add(sample_term("dag-runtime"));

        let term = idx.get("dag-runtime-alias").unwrap();
        assert_eq!(term.term, "dag-runtime");
    }

    #[test]
    fn test_get_not_found() {
        let idx = GlossaryIndex::empty();
        assert!(idx.get("nonexistent").is_err());
    }

    #[test]
    fn test_search() {
        let mut idx = GlossaryIndex::empty();
        idx.add(sample_term("memory-system"));
        idx.add(sample_term("sandbox-isolation"));

        let results = idx.search("memory");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].term, "memory-system");
    }

    #[test]
    fn test_list_all_sorted() {
        let mut idx = GlossaryIndex::empty();
        idx.add(sample_term("zebra"));
        idx.add(sample_term("alpha"));
        idx.add(sample_term("beta"));

        let all = idx.list_all();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].term, "alpha");
        assert_eq!(all[1].term, "beta");
        assert_eq!(all[2].term, "zebra");
    }

    #[test]
    fn test_by_tag() {
        let mut idx = GlossaryIndex::empty();
        let mut t1 = sample_term("term-a");
        t1.tags = vec!["rust".to_string()];
        let mut t2 = sample_term("term-b");
        t2.tags = vec!["python".to_string()];
        idx.add(t1);
        idx.add(t2);

        let rust_terms = idx.by_tag("rust");
        assert_eq!(rust_terms.len(), 1);
        assert_eq!(rust_terms[0].term, "term-a");
    }

    #[test]
    fn test_inject_context() {
        let mut idx = GlossaryIndex::empty();
        idx.add(sample_term("wasm-sandbox"));
        idx.add(sample_term("context-graph"));

        let ctx = idx.inject_context(&["wasm-sandbox", "unknown"], 5);
        assert!(ctx.contains("## Glossário"));
        assert!(ctx.contains("**wasm-sandbox**"));
        assert!(!ctx.contains("unknown"));
    }

    #[test]
    fn test_inject_context_empty() {
        let idx = GlossaryIndex::empty();
        let ctx = idx.inject_context(&["nothing"], 5);
        assert!(ctx.is_empty());
    }

    #[test]
    fn test_validate_skill_terms() {
        let mut idx = GlossaryIndex::empty();
        idx.add(sample_term("vector-store"));

        let text = "This skill uses vector-store and some unknown-term-here";
        let undefined = idx.validate_skill_terms(text);
        assert!(undefined.contains(&"unknown-term-here".to_string()));
        assert!(!undefined.contains(&"vector-store".to_string()));
    }

    #[test]
    fn test_to_json_roundtrip() {
        let mut idx = GlossaryIndex::empty();
        idx.add(sample_term("test-term"));

        let json = idx.to_json().unwrap();
        let parsed: Glossary = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.terms.len(), 1);
        assert_eq!(parsed.terms[0].term, "test-term");
    }

    #[test]
    fn test_load_from_file() {
        let dir = std::env::temp_dir().join("zylos-glossary-test");
        fs::create_dir_all(&dir).unwrap();

        let glossary = Glossary {
            version: "1.0.0".to_string(),
            updated_at: Some("2026-10-31T00:00:00Z".to_string()),
            total_terms: 2,
            terms: vec![sample_term("term-a"), sample_term("term-b")],
        };

        let json = serde_json::to_string_pretty(&glossary).unwrap();
        fs::write(dir.join("GLOSSARY.json"), &json).unwrap();

        let idx = GlossaryIndex::load(&dir.join("GLOSSARY.json")).unwrap();
        assert_eq!(idx.len(), 2);
        assert!(idx.get("term-a").is_ok());
        assert!(idx.load_time_ms() < 100.0);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_load_missing_file() {
        let result = GlossaryIndex::load(Path::new("/nonexistent/GLOSSARY.json"));
        assert!(result.is_err());
    }
}
