//! # OutputSanitizer — Pipeline de sanitização de output de agentes (T8.3.4)
//!
//! Remove secrets, tokens, passwords e outros padrões sensíveis do output
//! de agentes ANTES de ser retornado ao usuário ou transmitido externamente.
//!
//! ## Arquitetura
//! - Pipeline de regras regex aplicadas sequencialmente
//! - Cada regra tem nome, padrão e estratégia de substituição
//! - Suporte a regras customizadas além dos padrões built-in
//! - Métricas de sanitização para monitoring
//!
//! ## Integração
//! Usar como etapa final no pipeline de resposta do gateway:
//! ```ignore
//! let sanitized = sanitizer.sanitize(agent_output);
//! return sanitized.text;
//! ```

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Estratégia de substituição para uma regra de sanitização.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SanitizeStrategy {
    /// Substitui por `[REDACTED:<label>]`.
    Redact { label: String },
    /// Substitui por string fixa.
    Replace { with: String },
    /// Remove completamente.
    Remove,
}

impl Default for SanitizeStrategy {
    fn default() -> Self {
        Self::Redact {
            label: "SECRET".into(),
        }
    }
}

/// Uma regra de sanitização individual.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SanitizeRule {
    /// Nome da regra para logging/métricas.
    pub name: String,
    /// Padrão regex a detectar.
    pub pattern: String,
    /// Estratégia de substituição.
    pub strategy: SanitizeStrategy,
    /// Se true, a regra é aplicada; se false, é ignorada.
    pub enabled: bool,
}

/// Resultado da sanitização.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SanitizeResult {
    /// Texto após sanitização.
    pub text: String,
    /// Número total de substituições realizadas.
    pub redactions_count: usize,
    /// Nomes das regras que foram acionadas.
    pub rules_triggered: Vec<String>,
    /// Se o texto foi modificado.
    pub was_modified: bool,
}

/// Configuração do pipeline de sanitização.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SanitizerConfig {
    /// Regras de sanitização.
    pub rules: Vec<SanitizeRule>,
    /// Se true, usa regras built-in além das customizadas.
    pub use_builtin_rules: bool,
}

impl Default for SanitizerConfig {
    fn default() -> Self {
        Self {
            rules: vec![],
            use_builtin_rules: true,
        }
    }
}

/// Pipeline de sanitização de output.
#[derive(Clone)]
pub struct OutputSanitizer {
    compiled_rules: Arc<Vec<CompiledRule>>,
}

struct CompiledRule {
    name: String,
    regex: Regex,
    strategy: SanitizeStrategy,
}

impl std::fmt::Debug for CompiledRule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompiledRule")
            .field("name", &self.name)
            .field("pattern", &self.regex.as_str())
            .field("strategy", &self.strategy)
            .finish()
    }
}

/// Regras built-in cobrindo os padrões mais comuns de secrets.
fn builtin_rules() -> Vec<SanitizeRule> {
    vec![
        // AWS Access Key ID
        SanitizeRule {
            name: "aws_access_key".into(),
            pattern: r"(?:AKIA|ABIA|ACCA|ASIA)[0-9A-Z]{16}".into(),
            strategy: SanitizeStrategy::Redact { label: "AWS_KEY".into() },
            enabled: true,
        },
        // AWS Secret Access Key
        SanitizeRule {
            name: "aws_secret_key".into(),
            pattern: r#"(?i)(?:aws[_\-]?secret[_\-]?(?:access)?[_\-]?key)\s*[=:]\s*['"]?[A-Za-z0-9/+=]{40}['"]?"#.into(),
            strategy: SanitizeStrategy::Redact { label: "AWS_SECRET".into() },
            enabled: true,
        },
        // Generic API Key / Secret
        SanitizeRule {
            name: "generic_api_key".into(),
            pattern: r#"(?i)(?:api[_\-]?key|secret[_\-]?key|access[_\-]?token|auth[_\-]?token)\s*[=:]\s*['"]?[A-Za-z0-9_\-\.]{16,}['"]?"#.into(),
            strategy: SanitizeStrategy::Redact { label: "API_KEY".into() },
            enabled: true,
        },
        // Bearer Token
        SanitizeRule {
            name: "bearer_token".into(),
            pattern: r"(?i)bearer\s+[A-Za-z0-9_\-\.]{20,}".into(),
            strategy: SanitizeStrategy::Redact { label: "BEARER_TOKEN".into() },
            enabled: true,
        },
        // Password in config/code
        SanitizeRule {
            name: "password_field".into(),
            pattern: r#"(?i)(?:password|passwd|pwd|pass|secret)\s*[=:]\s*['"][^'"]{4,}['"]"#.into(),
            strategy: SanitizeStrategy::Redact { label: "PASSWORD".into() },
            enabled: true,
        },
        // Private Key header
        SanitizeRule {
            name: "private_key".into(),
            pattern: r"-----BEGIN (?:RSA |EC |DSA |OPENSSH )?PRIVATE KEY-----[\s\S]*?-----END (?:RSA |EC |DSA |OPENSSH )?PRIVATE KEY-----".into(),
            strategy: SanitizeStrategy::Redact { label: "PRIVATE_KEY".into() },
            enabled: true,
        },
        // GitHub Personal Access Token
        SanitizeRule {
            name: "github_pat".into(),
            pattern: r"ghp_[A-Za-z0-9]{36}".into(),
            strategy: SanitizeStrategy::Redact { label: "GITHUB_PAT".into() },
            enabled: true,
        },
        // npm token
        SanitizeRule {
            name: "npm_token".into(),
            pattern: r"npm_[A-Za-z0-9]{36}".into(),
            strategy: SanitizeStrategy::Redact { label: "NPM_TOKEN".into() },
            enabled: true,
        },
        // Generic high-entropy hex strings (potential keys/hashes leaked)
        SanitizeRule {
            name: "hex_secret".into(),
            pattern: r#"(?i)(?:key|secret|token|hash|credential)\s*[=:]\s*['"]?[0-9a-f]{32,}['"]?"#.into(),
            strategy: SanitizeStrategy::Redact { label: "HEX_SECRET".into() },
            enabled: true,
        },
        // Connection strings with credentials
        SanitizeRule {
            name: "connection_string".into(),
            pattern: r#"(?i)(?:mongodb|postgres|mysql|redis|amqp)://[^:\s]+:[^@\s]+@"#.into(),
            strategy: SanitizeStrategy::Redact { label: "CONN_STRING".into() },
            enabled: true,
        },
    ]
}

impl OutputSanitizer {
    /// Cria um novo sanitizador com a configuração fornecida.
    pub fn new(config: &SanitizerConfig) -> Result<Self, regex::Error> {
        let mut rules: Vec<SanitizeRule> = Vec::new();

        if config.use_builtin_rules {
            rules.extend(builtin_rules());
        }

        // Custom rules override/extend builtins
        rules.extend(config.rules.iter().cloned());

        let compiled: Result<Vec<CompiledRule>, _> = rules
            .into_iter()
            .filter(|r| r.enabled)
            .map(|r| {
                Ok(CompiledRule {
                    name: r.name.clone(),
                    regex: Regex::new(&r.pattern)?,
                    strategy: r.strategy.clone(),
                })
            })
            .collect();

        Ok(Self {
            compiled_rules: Arc::new(compiled?),
        })
    }

    /// Cria um sanitizador com apenas regras built-in.
    pub fn builtin() -> Result<Self, regex::Error> {
        Self::new(&SanitizerConfig::default())
    }

    /// Sanitiza um texto, aplicando todas as regras habilitadas.
    pub fn sanitize(&self, input: &str) -> SanitizeResult {
        let mut text = input.to_string();
        let mut redactions_count = 0;
        let mut rules_triggered: Vec<String> = Vec::new();

        for rule in self.compiled_rules.iter() {
            let replacement = match &rule.strategy {
                SanitizeStrategy::Redact { label } => format!("[REDACTED:{}]", label),
                SanitizeStrategy::Replace { with } => with.clone(),
                SanitizeStrategy::Remove => String::new(),
            };

            let count = rule.regex.find_iter(&text).count();
            if count > 0 {
                text = rule.regex.replace_all(&text, replacement.as_str()).to_string();
                redactions_count += count;
                rules_triggered.push(rule.name.clone());
            }
        }

        SanitizeResult {
            was_modified: text != input,
            text,
            redactions_count,
            rules_triggered,
        }
    }

    /// Verifica se o texto contém algum padrão sensível sem modificar.
    pub fn contains_secrets(&self, input: &str) -> bool {
        self.compiled_rules
            .iter()
            .any(|rule| rule.regex.is_match(input))
    }

    /// Retorna os nomes de todas as regras ativas.
    pub fn active_rule_names(&self) -> Vec<&str> {
        self.compiled_rules.iter().map(|r| r.name.as_str()).collect()
    }

    /// Retorna o número de regras ativas.
    pub fn rule_count(&self) -> usize {
        self.compiled_rules.len()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_sanitizer() -> OutputSanitizer {
        OutputSanitizer::builtin().unwrap()
    }

    #[test]
    fn test_sanitize_aws_key() {
        let s = test_sanitizer();
        let result = s.sanitize("My key is AKIAIOSFODNN7EXAMPLE and it works");
        assert!(result.was_modified);
        assert!(!result.text.contains("AKIAIOSFODNN7EXAMPLE"));
        assert!(result.text.contains("[REDACTED:AWS_KEY]"));
        assert!(result.rules_triggered.contains(&"aws_access_key".to_string()));
    }

    #[test]
    fn test_sanitize_bearer_token() {
        let s = test_sanitizer();
        let result = s.sanitize("Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U");
        assert!(result.was_modified);
        assert!(!result.text.contains("eyJhbGci"));
        assert!(result.text.contains("[REDACTED:BEARER_TOKEN]"));
    }

    #[test]
    fn test_sanitize_password() {
        let s = test_sanitizer();
        let result = s.sanitize(r#"config.password = "super_secret_123""#);
        assert!(result.was_modified);
        assert!(!result.text.contains("super_secret_123"));
        assert!(result.text.contains("[REDACTED:PASSWORD]"));
    }

    #[test]
    fn test_sanitize_private_key() {
        let s = test_sanitizer();
        let input = "Here is the key:\n-----BEGIN RSA PRIVATE KEY-----\nMIIEowIBAAKCAQEA...\n-----END RSA PRIVATE KEY-----\nDone.";
        let result = s.sanitize(input);
        assert!(result.was_modified);
        assert!(!result.text.contains("BEGIN RSA PRIVATE KEY"));
        assert!(result.text.contains("[REDACTED:PRIVATE_KEY]"));
    }

    #[test]
    fn test_sanitize_github_pat() {
        let s = test_sanitizer();
        let result = s.sanitize("Token: ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghij");
        assert!(result.was_modified);
        assert!(!result.text.contains("ghp_"));
        assert!(result.text.contains("[REDACTED:GITHUB_PAT]"));
    }

    #[test]
    fn test_sanitize_connection_string() {
        let s = test_sanitizer();
        let result = s.sanitize("Connect to postgres://admin:s3cret@db.example.com:5432/mydb");
        assert!(result.was_modified);
        assert!(!result.text.contains("s3cret"));
        assert!(result.text.contains("[REDACTED:CONN_STRING]"));
    }

    #[test]
    fn test_clean_text_unchanged() {
        let s = test_sanitizer();
        let input = "This is a perfectly normal response with no secrets.";
        let result = s.sanitize(input);
        assert!(!result.was_modified);
        assert_eq!(result.text, input);
        assert_eq!(result.redactions_count, 0);
        assert!(result.rules_triggered.is_empty());
    }

    #[test]
    fn test_contains_secrets() {
        let s = test_sanitizer();
        assert!(s.contains_secrets("key=AKIAIOSFODNN7EXAMPLE"));
        assert!(!s.contains_secrets("just normal text here"));
    }

    #[test]
    fn test_multiple_secrets_in_one_text() {
        let s = test_sanitizer();
        let input = "AWS: AKIAIOSFODNN7EXAMPLE and also password=\"hunter2\" and Bearer eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.longtokenvaluehere1234";
        let result = s.sanitize(input);
        assert!(result.was_modified);
        assert!(result.redactions_count >= 3);
        assert!(result.rules_triggered.len() >= 2);
    }

    #[test]
    fn test_custom_rule_replace_strategy() {
        let config = SanitizerConfig {
            rules: vec![SanitizeRule {
                name: "internal_id".into(),
                pattern: r"INTERNAL-\d{6}".into(),
                strategy: SanitizeStrategy::Replace {
                    with: "[ID-HIDDEN]".into(),
                },
                enabled: true,
            }],
            use_builtin_rules: false,
        };
        let s = OutputSanitizer::new(&config).unwrap();
        let result = s.sanitize("Processing INTERNAL-123456 now");
        assert!(result.was_modified);
        assert!(result.text.contains("[ID-HIDDEN]"));
        assert!(!result.text.contains("INTERNAL-123456"));
    }

    #[test]
    fn test_custom_rule_remove_strategy() {
        let config = SanitizerConfig {
            rules: vec![SanitizeRule {
                name: "debug_marker".into(),
                pattern: r"\[DEBUG\]".into(),
                strategy: SanitizeStrategy::Remove,
                enabled: true,
            }],
            use_builtin_rules: false,
        };
        let s = OutputSanitizer::new(&config).unwrap();
        let result = s.sanitize("Output [DEBUG] with marker [DEBUG] removed");
        assert!(result.was_modified);
        assert!(!result.text.contains("[DEBUG]"));
        assert_eq!(result.redactions_count, 2);
    }

    #[test]
    fn test_disabled_rule_ignored() {
        let config = SanitizerConfig {
            rules: vec![SanitizeRule {
                name: "disabled_rule".into(),
                pattern: r"SECRET_WORD".into(),
                strategy: SanitizeStrategy::Redact {
                    label: "X".into(),
                },
                enabled: false,
            }],
            use_builtin_rules: false,
        };
        let s = OutputSanitizer::new(&config).unwrap();
        let result = s.sanitize("Contains SECRET_WORD here");
        assert!(!result.was_modified);
        assert!(result.text.contains("SECRET_WORD"));
    }

    #[test]
    fn test_active_rule_names() {
        let s = test_sanitizer();
        let names = s.active_rule_names();
        assert!(names.contains(&"aws_access_key"));
        assert!(names.contains(&"bearer_token"));
        assert!(names.contains(&"password_field"));
        assert!(names.len() >= 10); // All builtins
    }

    #[test]
    fn test_rule_count() {
        let s = test_sanitizer();
        assert!(s.rule_count() >= 10);

        let empty = OutputSanitizer::new(&SanitizerConfig {
            rules: vec![],
            use_builtin_rules: false,
        })
        .unwrap();
        assert_eq!(empty.rule_count(), 0);
    }

    #[test]
    fn test_sanitize_result_serialization() {
        let result = SanitizeResult {
            text: "cleaned".into(),
            redactions_count: 3,
            rules_triggered: vec!["a".into(), "b".into()],
            was_modified: true,
        };
        let json = serde_json::to_string(&result).unwrap();
        let restored: SanitizeResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, restored);
    }

    #[test]
    fn test_npm_token_detection() {
        let s = test_sanitizer();
        let result = s.sanitize("npm_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA");
        assert!(result.was_modified);
        assert!(result.text.contains("[REDACTED:NPM_TOKEN]"));
    }

    #[test]
    fn test_hex_secret_detection() {
        let s = test_sanitizer();
        let result = s.sanitize("secret=abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789");
        assert!(result.was_modified);
        assert!(result.text.contains("[REDACTED:HEX_SECRET]"));
    }

    #[test]
    fn test_builtin_plus_custom_rules() {
        let config = SanitizerConfig {
            rules: vec![SanitizeRule {
                name: "custom_ticket".into(),
                pattern: r"TICKET-\d+".into(),
                strategy: SanitizeStrategy::Redact {
                    label: "TICKET".into(),
                },
                enabled: true,
            }],
            use_builtin_rules: true,
        };
        let s = OutputSanitizer::new(&config).unwrap();

        // Both builtin and custom should work
        let result = s.sanitize("Key: AKIAIOSFODNN7EXAMPLE Ref: TICKET-9999");
        assert!(result.was_modified);
        assert!(result.rules_triggered.contains(&"aws_access_key".to_string()));
        assert!(result.rules_triggered.contains(&"custom_ticket".to_string()));
    }
}