//! Debt Tracking Estruturado — Task 5.2.2
//!
//! Parser de comentários `shortcut:` e geração de relatório `/debt`.
//! Inspirado em ponytail shortcut: comments.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;
use tracing::{debug, info};

/// Erros do sistema de debt tracking
#[derive(Error, Debug)]
/// Enumeração DebtError.
pub enum DebtError {
    #[error("falha ao ler arquivo: {0}")]
    // Variante IoError.
    IoError(#[from] std::io::Error),

    #[error("JSON inválido: {0}")]
    // Variante ParseError.
    ParseError(#[from] serde_json::Error),

    #[error("diretório não encontrado: {0}")]
    // Variante DirectoryNotFound.
    DirectoryNotFound(String),

    #[error("nenhuma dívida encontrada")]
    /// Variante NoDebtFound.
    NoDebtFound,
}

/// Result type alias para operações de debt
pub type DebtResult<T> = Result<T, DebtError>;

/// Severidade da dívida técnica
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Default)]
#[serde(rename_all = "lowercase")]
/// Enumeração DebtSeverity.
pub enum DebtSeverity {
    /// Baixa prioridade, pode esperar
    Low,
    /// Média prioridade, resolver no próximo sprint
    #[default]
    /// Variante Medium.
    Medium,
    /// Alta prioridade, resolver urgentemente
    High,
    /// Crítica, bloqueia entregas
    Critical,
}

impl std::fmt::Display for DebtSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DebtSeverity::Low => write!(f, "low"),
            DebtSeverity::Medium => write!(f, "medium"),
            DebtSeverity::High => write!(f, "high"),
            DebtSeverity::Critical => write!(f, "critical"),
        }
    }
}



/// Categoria da dívida técnica
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
/// Enumeração DebtCategory.
pub enum DebtCategory {
    /// Código duplicado ou redundante
    Duplication,
    /// Complexidade excessiva
    Complexity,
    /// Falta de testes
    TestCoverage,
    /// Documentação ausente ou desatualizada
    Documentation,
    /// Performance abaixo do esperado
    Performance,
    /// Segurança comprometida
    Security,
    /// Dependência desatualizada ou vulnerável
    Dependency,
    /// Design/API inconsistente
    Design,
    // Outro tipo de dívida
    Other(String),
}

impl std::fmt::Display for DebtCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DebtCategory::Duplication => write!(f, "duplication"),
            DebtCategory::Complexity => write!(f, "complexity"),
            DebtCategory::TestCoverage => write!(f, "test_coverage"),
            DebtCategory::Documentation => write!(f, "documentation"),
            DebtCategory::Performance => write!(f, "performance"),
            DebtCategory::Security => write!(f, "security"),
            DebtCategory::Dependency => write!(f, "dependency"),
            DebtCategory::Design => write!(f, "design"),
            DebtCategory::Other(s) => write!(f, "{}", s),
        }
    }
}

impl Default for DebtCategory {
    fn default() -> Self {
        DebtCategory::Other("unknown".to_string())
    }
}

/// Item individual de dívida técnica extraído de um comentário `shortcut:`
#[derive(Debug, Clone, Serialize, Deserialize)]
/// Estrutura DebtItem.
pub struct DebtItem {
    /// Descrição da dívida
    pub description: String,

    /// Severidade
    pub severity: DebtSeverity,

    /// Categoria
    pub category: DebtCategory,

    /// Arquivo onde foi encontrada
    pub file: String,

    /// Linha no arquivo
    pub line: usize,

    /// Contexto (linha de código original)
    pub context: String,

    /// Tags associadas
    #[serde(default)]
    /// Campo tags.
    pub tags: Vec<String>,

    /// Ticket ou issue relacionada (opcional)
    #[serde(default)]
    /// Campo ticket.
    pub ticket: Option<String>,
}

/// Relatório consolidado de dívida técnica
#[derive(Debug, Clone, Serialize, Deserialize)]
/// Estrutura DebtReport.
pub struct DebtReport {
    /// Total de itens encontrados
    pub total_items: usize,

    /// Contagem por severidade
    pub by_severity: HashMap<String, usize>,

    /// Contagem por categoria
    pub by_category: HashMap<String, usize>,

    /// Itens agrupados por arquivo
    pub by_file: HashMap<String, Vec<DebtItem>>,

    /// Score de saúde do códigobase (0.0 = muita dívida, 1.0 = limpo)
    pub health_score: f64,

    /// Timestamp da geração (ISO 8601)
    pub generated_at: String,

    /// Diretório escaneado
    pub scanned_directory: String,
}

/// Parser de comentários `shortcut:`
#[derive(Debug)]
/// Estrutura DebtParser.
pub struct DebtParser {
    marker: String,
}

impl DebtParser {
    /// Cria um parser com o marcador padrão `shortcut:`
    pub fn new() -> Self {
        // Variante Self.
        Self {
            marker: "shortcut:".to_string(),
        }
    }

    /// Cria um parser com marcador customizado
    pub fn with_marker(marker: &str) -> Self {
        // Variante Self.
        Self {
            marker: marker.to_string(),
        }
    }

    /// Extrai itens de dívida de uma string de código-fonte
    pub fn parse_source(&self, source: &str, file_path: &str) -> Vec<DebtItem> {
        let mut items = Vec::new();

        for (line_idx, line) in source.lines().enumerate() {
            if let Some(pos) = line.find(&self.marker) {
                let after_marker = &line[pos + self.marker.len()..];
                let trimmed = after_marker.trim();

                if trimmed.is_empty() {
                    continue;
                }

                let item = self.parse_debt_comment(trimmed, file_path, line_idx + 1, line);

                debug!(
                    "Dívida encontrada em {}:{}: {}",
                    file_path,
                    line_idx + 1,
                    item.description
                );

                items.push(item);
            }
        }

        items
    }

    /// Faz parse de um comentário de dívida individual
    fn parse_debt_comment(&self, comment: &str, file: &str, line: usize, context: &str) -> DebtItem {
        // Formato esperado: [severity] [category] descrição #tag1 #tag2 (ticket: ABC-123)
        let mut severity = DebtSeverity::default();
        let mut category = DebtCategory::default();
        let mut tags = Vec::new();
        let mut ticket = None;
        let mut description = comment.to_string();

        // Extrai severidade entre colchetes
        if let Some(sev_start) = comment.find('[') {
            if let Some(sev_end) = comment.find(']') {
                let sev_str = &comment[sev_start + 1..sev_end].trim().to_lowercase();
                severity = match sev_str.as_str() {
                    "low" | "baixa" => DebtSeverity::Low,
                    "medium" | "média" | "media" => DebtSeverity::Medium,
                    "high" | "alta" => DebtSeverity::High,
                    "critical" | "crítica" | "critica" => DebtSeverity::Critical,
                    _ => DebtSeverity::default(),
                };
                // Remove o trecho de severidade da descrição
                description = format!(
                    "{}{}",
                    &comment[..sev_start],
                    &comment[sev_end + 1..]
                )
                .trim()
                .to_string();
            }
        }

        // Extrai categoria entre parênteses após severidade
        if let Some(cat_start) = description.find('(') {
            if let Some(cat_end) = description.find(')') {
                let cat_str = &description[cat_start + 1..cat_end].trim().to_lowercase();
                category = match cat_str.as_str() {
                    "duplication" | "duplicação" => DebtCategory::Duplication,
                    "complexity" | "complexidade" => DebtCategory::Complexity,
                    "test_coverage" | "testes" | "tests" => DebtCategory::TestCoverage,
                    "documentation" | "documentação" | "docs" => DebtCategory::Documentation,
                    "performance" | "perf" => DebtCategory::Performance,
                    "security" | "segurança" => DebtCategory::Security,
                    "dependency" | "dependência" | "deps" => DebtCategory::Dependency,
                    "design" | "api" => DebtCategory::Design,
                    other => DebtCategory::Other(other.to_string()),
                };
                // Remove o trecho de categoria da descrição
                description = format!(
                    "{}{}",
                    &description[..cat_start],
                    &description[cat_end + 1..]
                )
                .trim()
                .to_string();
            }
        }

        // Extrai tags (#tag)
        let words: Vec<&str> = description.split_whitespace().collect();
        let mut desc_parts = Vec::new();
        for word in &words {
            if word.starts_with('#') && word.len() > 1 {
                tags.push(word[1..].to_string());
            } else if word.starts_with("ticket:") || word.starts_with("ref:") {
                ticket = Some(word.split(':').nth(1).unwrap_or("").to_string());
            } else {
                desc_parts.push(*word);
            }
        }
        description = desc_parts.join(" ");

        // Variante DebtItem.
        DebtItem {
            description,
            severity,
            category,
            file: file.to_string(),
            line,
            context: context.trim().to_string(),
            tags,
            ticket,
        }
    }

    /// Escaneia um diretório recursivamente buscando dívidas
    pub fn scan_directory(&self, dir: &Path) -> DebtResult<Vec<DebtItem>> {
        if !dir.exists() {
            return Err(DebtError::DirectoryNotFound(dir.display().to_string()));
        }

        let mut all_items = Vec::new();
        self.scan_recursive(dir, &mut all_items)?;

        info!(
            "Scan completo: {} itens de dívida encontrados em '{}'",
            all_items.len(),
            dir.display()
        );

        // Variante Ok.
        Ok(all_items)
    }

    fn scan_recursive(&self, dir: &Path, items: &mut Vec<DebtItem>) -> DebtResult<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                // Pula diretórios comuns que não devem ser escaneados
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if matches!(name, "target" | "node_modules" | ".git" | ".svn" | "__pycache__") {
                    continue;
                }
                self.scan_recursive(&path, items)?;
            } else if path.is_file() {
                // Escaneia apenas arquivos de código
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                if matches!(
                    ext,
                    "rs" | "py" | "ts" | "tsx" | "js" | "jsx" | "go" | "java" | "c" | "cpp" | "h" | "hpp" | "md" | "yaml" | "yml" | "toml"
                ) {
                    let content = std::fs::read_to_string(&path)?;
                    let file_str = path.display().to_string();
                    let found = self.parse_source(&content, &file_str);
                    items.extend(found);
                }
            }
        }

        // Variante Ok.
        Ok(())
    }
}

impl Default for DebtParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Gera um relatório consolidado de dívida
pub fn generate_report(items: Vec<DebtItem>, scanned_dir: &str) -> DebtReport {
    let total = items.len();

    let mut by_severity: HashMap<String, usize> = HashMap::new();
    let mut by_category: HashMap<String, usize> = HashMap::new();
    let mut by_file: HashMap<String, Vec<DebtItem>> = HashMap::new();

    for item in &items {
        *by_severity.entry(item.severity.to_string()).or_insert(0) += 1;
        *by_category.entry(item.category.to_string()).or_insert(0) += 1;
        by_file
            .entry(item.file.clone())
            .or_default()
            .push(item.clone());
    }

    // Health score: penaliza mais severidades altas
    let health_score = if total == 0 {
        1.0
    } else {
        let weighted: f64 = items
            .iter()
            .map(|i| match i.severity {
                DebtSeverity::Low => 0.1,
                DebtSeverity::Medium => 0.3,
                DebtSeverity::High => 0.7,
                DebtSeverity::Critical => 1.0,
            })
            .sum();
        // Normaliza: assume máximo de 1 ponto de dívida por item escaneado
        let max_possible = total as f64;
        (1.0 - (weighted / max_possible)).max(0.0)
    };

    // Variante DebtReport.
    DebtReport {
        total_items: total,
        by_severity,
        by_category,
        by_file,
        health_score,
        generated_at: chrono::Utc::now().to_rfc3339(),
        scanned_directory: scanned_dir.to_string(),
    }
}

/// Gera relatório em formato Markdown
pub fn report_to_markdown(report: &DebtReport) -> String {
    let mut lines = vec![
        "# Relatório de Dívida Técnica".to_string(),
        format!("Gerado em: {}", report.generated_at),
        format!("Diretório: `{}`", report.scanned_directory),
        String::new(),
        "## Resumo".to_string(),
        format!("- **Total de itens:** {}", report.total_items),
        format!("- **Score de saúde:** {:.1}%", report.health_score * 100.0),
        String::new(),
    ];

    // Por severidade
    lines.push("## Por Severidade".to_string());
    let mut sev_entries: Vec<_> = report.by_severity.iter().collect();
    sev_entries.sort_by_key(|(k, _)| match k.as_str() {
        "critical" => 0,
        "high" => 1,
        "medium" => 2,
        "low" => 3,
        _ => 4,
    });
    for (sev, count) in &sev_entries {
        lines.push(format!("- **{}**: {} itens", sev, count));
    }
    lines.push(String::new());

    // Por categoria
    lines.push("## Por Categoria".to_string());
    let mut cat_entries: Vec<_> = report.by_category.iter().collect();
    cat_entries.sort_by(|a, b| b.1.cmp(a.1));
    for (cat, count) in &cat_entries {
        lines.push(format!("- **{}**: {} itens", cat, count));
    }
    lines.push(String::new());

    // Por arquivo
    lines.push("## Detalhes por Arquivo".to_string());
    let mut file_entries: Vec<_> = report.by_file.iter().collect();
    file_entries.sort_by_key(|a| std::cmp::Reverse(a.1.len()));
    for (file, items) in &file_entries {
        lines.push(format!("\n### `{}` ({} itens)", file, items.len()));
        for item in *items {
            let ticket_info = item
                .ticket
                .as_ref()
                .map(|t| format!(" [{}]", t))
                .unwrap_or_default();
            let tags_info = if item.tags.is_empty() {
                String::new()
            } else {
                format!(" ({})", item.tags.iter().map(|t| format!("#{}", t)).collect::<Vec<_>>().join(" "))
            };
            lines.push(format!(
                "- **L{}** [{}] [{}] {}{}{}",
                item.line, item.severity, item.category, item.description, tags_info, ticket_info
            ));
            if !item.context.is_empty() {
                lines.push(format!("  ```\n  {}\n  ```", item.context));
            }
        }
    }

    lines.join("\n")
}

/// Serializa relatório para JSON
pub fn report_to_json(report: &DebtReport) -> DebtResult<String> {
    serde_json::to_string_pretty(report).map_err(DebtError::ParseError)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_shortcut() {
        let parser = DebtParser::new();
        let source = r#"
fn main() {
    // shortcut: Refatorar esta função muito longa
    println!("hello");
}
"#;
        let items = parser.parse_source(source, "main.rs");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].description, "Refatorar esta função muito longa");
        assert_eq!(items[0].file, "main.rs");
        assert_eq!(items[0].line, 3);
    }

    #[test]
    fn test_parse_with_severity_and_category() {
        let parser = DebtParser::new();
        let source = "// shortcut: [high] (performance) Otimizar query N+1 #database #perf";
        let items = parser.parse_source(source, "repo.rs");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].severity, DebtSeverity::High);
        assert_eq!(items[0].category, DebtCategory::Performance);
        assert!(items[0].tags.contains(&"database".to_string()));
        assert!(items[0].tags.contains(&"perf".to_string()));
        assert_eq!(items[0].description, "Otimizar query N+1");
    }

    #[test]
    fn test_parse_with_ticket() {
        let parser = DebtParser::new();
        let source = "// shortcut: [critical] (security) Sanitizar input do usuário ticket:SEC-42";
        let items = parser.parse_source(source, "auth.rs");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].severity, DebtSeverity::Critical);
        assert_eq!(items[0].category, DebtCategory::Security);
        assert_eq!(items[0].ticket, Some("SEC-42".to_string()));
    }

    #[test]
    fn test_parse_multiple_items() {
        let parser = DebtParser::new();
        let source = r#"
// shortcut: [low] (docs) Adicionar exemplos na API
fn api_handler() {}

// shortcut: [medium] (test_coverage) Cobrir edge cases
fn process_data() {}

// shortcut: [high] (duplication) Unificar lógica repetida #refactor
fn validate_input() {}
"#;
        let items = parser.parse_source(source, "lib.rs");
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].severity, DebtSeverity::Low);
        assert_eq!(items[1].severity, DebtSeverity::Medium);
        assert_eq!(items[2].severity, DebtSeverity::High);
    }

    #[test]
    fn test_parse_no_shortcuts() {
        let parser = DebtParser::new();
        let source = r#"
fn clean_code() {
    // Este é um comentário normal
    println!("sem dívida aqui");
}
"#;
        let items = parser.parse_source(source, "clean.rs");
        assert!(items.is_empty());
    }

    #[test]
    fn test_custom_marker() {
        let parser = DebtParser::with_marker("TODO-DEBT:");
        let source = "// TODO-DEBT: Implementar cache #perf";
        let items = parser.parse_source(source, "cache.rs");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].description, "Implementar cache");
    }

    #[test]
    fn test_generate_report_empty() {
        let report = generate_report(vec![], "/src");
        assert_eq!(report.total_items, 0);
        assert_eq!(report.health_score, 1.0);
    }

    #[test]
    fn test_generate_report_with_items() {
        let items = vec![
            // Variante DebtItem.
                        DebtItem {
                            description: "Fix A".to_string(),
                            severity: DebtSeverity::High,
                            category: DebtCategory::Performance,
                file: "a.rs".to_string(),
                line: 10,
                context: "".to_string(),
                tags: vec![],
                ticket: None,
            },
            // Variante DebtItem.
                        DebtItem {
                            description: "Fix B".to_string(),
                            severity: DebtSeverity::Low,
                            category: DebtCategory::Documentation,
                file: "b.rs".to_string(),
                line: 20,
                context: "".to_string(),
                tags: vec![],
                ticket: None,
            },
        ];
        let report = generate_report(items, "/src");
        assert_eq!(report.total_items, 2);
        assert!(report.health_score > 0.0 && report.health_score < 1.0);
        assert_eq!(report.by_severity.get("high"), Some(&1));
        assert_eq!(report.by_severity.get("low"), Some(&1));
    }

    #[test]
    fn test_report_to_markdown() {
        let items = vec![DebtItem {
            description: "Test debt".to_string(),
            severity: DebtSeverity::Medium,
            category: DebtCategory::Complexity,
            file: "test.rs".to_string(),
            line: 5,
            context: "// shortcut: [medium] (complexity) Test debt".to_string(),
            tags: vec!["refactor".to_string()],
            ticket: Some("JIRA-100".to_string()),
        }];
        let report = generate_report(items, "/project");
        let md = report_to_markdown(&report);
        assert!(md.contains("# Relatório de Dívida Técnica"));
        assert!(md.contains("test.rs"));
        assert!(md.contains("Test debt"));
        assert!(md.contains("JIRA-100"));
        assert!(md.contains("#refactor"));
    }

    #[test]
    fn test_report_to_json() {
        let report = generate_report(vec![], "/empty");
        let json = report_to_json(&report).unwrap();
        let parsed: DebtReport = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.total_items, 0);
    }

    #[test]
    fn test_health_score_all_critical() {
        let items: Vec<DebtItem> = (0..5)
            .map(|i| DebtItem {
                description: format!("Issue {}", i),
                severity: DebtSeverity::Critical,
                category: DebtCategory::Security,
                file: format!("file{}.rs", i),
                line: 1,
                context: String::new(),
                tags: vec![],
                ticket: None,
            })
            .collect();
        let report = generate_report(items, "/bad");
        assert_eq!(report.health_score, 0.0);
    }

    #[test]
    fn test_health_score_all_low() {
        let items: Vec<DebtItem> = (0..5)
            .map(|i| DebtItem {
                description: format!("Minor {}", i),
                severity: DebtSeverity::Low,
                category: DebtCategory::Documentation,
                file: format!("file{}.rs", i),
                line: 1,
                context: String::new(),
                tags: vec![],
                ticket: None,
            })
            .collect();
        let report = generate_report(items, "/ok");
        assert!(report.health_score > 0.8);
    }

    #[test]
    fn test_severity_display() {
        assert_eq!(DebtSeverity::Low.to_string(), "low");
        assert_eq!(DebtSeverity::Critical.to_string(), "critical");
    }

    #[test]
    fn test_category_display() {
        assert_eq!(DebtCategory::Performance.to_string(), "performance");
        assert_eq!(
            DebtCategory::Other("custom".to_string()).to_string(),
            "custom"
        );
    }
}