//! EgressChecker — engine de detecção de PII, secrets e dados sensíveis
//!
//! Patterns built-in compilados estaticamente via LazyLock para performance.
//! Suporta regras customizáveis via API REST.

use crate::schema::*;
use regex::Regex;
use std::sync::LazyLock;

// === Patterns built-in (compilados uma vez) ===

static EMAIL_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}").unwrap()
});

static PHONE_BR_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:\+55\s?)?(?:\(?\d{2}\)?\s?)?\d{4,5}[-\s]?\d{4}").unwrap()
});

static CPF_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\d{3}\.\d{3}\.\d{3}-\d{2}").unwrap()
});

static CNPJ_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\d{2}\.\d{3}\.\d{3}/\d{4}-\d{2}").unwrap()
});

static CREDIT_CARD_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(?:\d[ -]*?){13,16}\b").unwrap()
});

static SECRET_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        Regex::new(r#"(?i)(api[_-]?key|secret|token|password|passwd)\s*[:=]\s*['"]?([a-zA-Z0-9_\-]{16,})['"]?"#).unwrap(),
        Regex::new(r"sk-[a-zA-Z0-9]{32,}").unwrap(),
        Regex::new(r"ghp_[a-zA-Z0-9]{36}").unwrap(),
        Regex::new(r"xox[baprs]-[a-zA-Z0-9\-]+").unwrap(),
        Regex::new(r"AIza[a-zA-Z0-9_\-]{35}").unwrap(),
    ]
});

static HEALTH_KEYWORDS: LazyLock<Vec<&str>> = LazyLock::new(|| {
    vec![
        "diagnóstico", "prescrição", "medicamento",
        "doença", "tratamento", "hospital", "cirurgia",
        "exame de sangue", "pressão arterial", "glicose", "colesterol",
    ]
});

/// Engine de verificação de egress
pub struct EgressChecker {
    custom_rules: Vec<EgressRule>,
    blocked_categories: Vec<SensitiveCategory>,
}

impl EgressChecker {
    pub fn new() -> Self {
        Self {
            custom_rules: vec![],
            blocked_categories: vec![
                SensitiveCategory::Secret,
                SensitiveCategory::CreditCard,
            ],
        }
    }

    pub fn with_custom_rules(mut self, rules: Vec<EgressRule>) -> Self {
        self.custom_rules = rules.into_iter().filter(|r| r.enabled).collect();
        self
    }

    pub fn with_blocked_categories(mut self, cats: Vec<SensitiveCategory>) -> Self {
        self.blocked_categories = cats;
        self
    }

    /// Verifica texto e retorna findings + versão redigida
    pub fn check(&self, text: &str) -> EgressCheckResult {
        let mut findings = Vec::new();

        // Emails
        for m in EMAIL_PATTERN.find_iter(text) {
            findings.push(SensitiveFinding {
                category: SensitiveCategory::Email.as_str().to_string(),
                matched_text: m.as_str().to_string(),
                redacted_text: redact_email(m.as_str()),
                start: m.start(),
                end: m.end(),
                confidence: 0.95,
                rule_name: "email_builtin".to_string(),
            });
        }

        // Telefones BR (mínimo 10 dígitos para evitar falsos positivos)
        for m in PHONE_BR_PATTERN.find_iter(text) {
            let digits: String = m.as_str().chars().filter(|c| c.is_ascii_digit()).collect();
            if digits.len() >= 10 {
                findings.push(SensitiveFinding {
                    category: SensitiveCategory::Phone.as_str().to_string(),
                    matched_text: m.as_str().to_string(),
                    redacted_text: redact_phone(m.as_str()),
                    start: m.start(),
                    end: m.end(),
                    confidence: 0.85,
                    rule_name: "phone_br_builtin".to_string(),
                });
            }
        }

        // CPF com validação de dígitos verificadores
        for m in CPF_PATTERN.find_iter(text) {
            if validate_cpf(m.as_str()) {
                findings.push(SensitiveFinding {
                    category: SensitiveCategory::BrazilianTaxId.as_str().to_string(),
                    matched_text: m.as_str().to_string(),
                    redacted_text: "***.***.***-**".to_string(),
                    start: m.start(),
                    end: m.end(),
                    confidence: 0.99,
                    rule_name: "cpf_builtin".to_string(),
                });
            }
        }

        // CNPJ com validação básica
        for m in CNPJ_PATTERN.find_iter(text) {
            if validate_cnpj(m.as_str()) {
                findings.push(SensitiveFinding {
                    category: SensitiveCategory::BrazilianTaxId.as_str().to_string(),
                    matched_text: m.as_str().to_string(),
                    redacted_text: "**.***.***/****-**".to_string(),
                    start: m.start(),
                    end: m.end(),
                    confidence: 0.99,
                    rule_name: "cnpj_builtin".to_string(),
                });
            }
        }

        // Credit cards com validação Luhn
        for m in CREDIT_CARD_PATTERN.find_iter(text) {
            let digits: String = m.as_str().chars().filter(|c| c.is_ascii_digit()).collect();
            if luhn_check(&digits) {
                findings.push(SensitiveFinding {
                    category: SensitiveCategory::CreditCard.as_str().to_string(),
                    matched_text: m.as_str().to_string(),
                    redacted_text: redact_credit_card(m.as_str()),
                    start: m.start(),
                    end: m.end(),
                    confidence: 0.98,
                    rule_name: "credit_card_builtin".to_string(),
                });
            }
        }

        // Secrets (API keys, tokens, passwords)
        for pattern in SECRET_PATTERNS.iter() {
            for m in pattern.find_iter(text) {
                findings.push(SensitiveFinding {
                    category: SensitiveCategory::Secret.as_str().to_string(),
                    matched_text: m.as_str().to_string(),
                    redacted_text: "[SECRET REDACTED]".to_string(),
                    start: m.start(),
                    end: m.end(),
                    confidence: 0.95,
                    rule_name: "secret_builtin".to_string(),
                });
            }
        }

        // Health keywords (baixa confiança, só aviso)
        let text_lower = text.to_lowercase();
        for keyword in HEALTH_KEYWORDS.iter() {
            if let Some(pos) = text_lower.find(keyword) {
                findings.push(SensitiveFinding {
                    category: SensitiveCategory::HealthData.as_str().to_string(),
                    matched_text: keyword.to_string(),
                    redacted_text: "[HEALTH DATA]".to_string(),
                    start: pos,
                    end: pos + keyword.len(),
                    confidence: 0.6,
                    rule_name: "health_keyword_builtin".to_string(),
                });
            }
        }

        // Custom rules
        for rule in &self.custom_rules {
            if let Ok(pattern) = Regex::new(&rule.pattern) {
                for m in pattern.find_iter(text) {
                    findings.push(SensitiveFinding {
                        category: rule.category.clone(),
                        matched_text: m.as_str().to_string(),
                        redacted_text: format!("[{} REDACTED]", rule.name),
                        start: m.start(),
                        end: m.end(),
                        confidence: 0.9,
                        rule_name: rule.name.clone(),
                    });
                }
            }
        }

        // Ordenar por posição e deduplicar overlaps
        findings.sort_by_key(|f| f.start);
        findings.dedup_by(|a, b| {
            a.start == b.start && a.end == b.end && a.category == b.category
        });

        // Gerar texto redigido
        let redacted_text = if !findings.is_empty() {
            let mut result = text.to_string();
            let mut offset: isize = 0;
            for finding in &findings {
                let start = (finding.start as isize + offset) as usize;
                let end = (finding.end as isize + offset) as usize;
                if start <= result.len() && end <= result.len() {
                    let original_len = end - start;
                    result.replace_range(start..end, &finding.redacted_text);
                    offset += finding.redacted_text.len() as isize - original_len as isize;
                }
            }
            Some(result)
        } else {
            None
        };

        // Determinar categorias bloqueantes
        let blocked_categories: Vec<String> = findings
            .iter()
            .filter_map(|f| SensitiveCategory::parse(&f.category))
            .filter(|cat| self.blocked_categories.contains(cat))
            .map(|cat| cat.as_str().to_string())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        EgressCheckResult {
            passed: blocked_categories.is_empty(),
            findings,
            redacted_text,
            blocked_categories,
        }
    }
}

impl Default for EgressChecker {
    fn default() -> Self {
        Self::new()
    }
}

// === Funções auxiliares de redação ===

fn redact_email(email: &str) -> String {
    let parts: Vec<&str> = email.split('@').collect();
    if parts.len() == 2 {
        format!("***@{}", parts[1])
    } else {
        "***@***.com".to_string()
    }
}

fn redact_phone(phone: &str) -> String {
    let digits: String = phone.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() >= 4 {
        format!("(**) *****-{}", &digits[digits.len() - 4..])
    } else {
        "(**) ****-****".to_string()
    }
}

fn redact_credit_card(cc: &str) -> String {
    let digits: String = cc.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() >= 4 {
        format!("****-****-****-{}", &digits[digits.len() - 4..])
    } else {
        "****-****-****-****".to_string()
    }
}

// === Funções de validação ===

/// Validação Luhn para cartões de crédito
fn luhn_check(number: &str) -> bool {
    let digits: Vec<u32> = number.chars().filter_map(|c| c.to_digit(10)).collect();
    if digits.len() < 13 || digits.len() > 19 {
        return false;
    }

    let sum: u32 = digits
        .iter()
        .rev()
        .enumerate()
        .map(|(i, &d)| {
            if i % 2 == 1 {
                let doubled = d * 2;
                if doubled > 9 { doubled - 9 } else { doubled }
            } else {
                d
            }
        })
        .sum();

    sum.is_multiple_of(10)
}

/// Validação de dígitos verificadores do CPF
fn validate_cpf(cpf: &str) -> bool {
    let digits: Vec<u32> = cpf.chars().filter_map(|c| c.to_digit(10)).collect();
    if digits.len() != 11 {
        return false;
    }
    // Rejeitar CPFs com todos os dígitos iguais
    if digits.iter().all(|&d| d == digits[0]) {
        return false;
    }

    let calc_digit = |pos: usize| -> u32 {
        let sum: u32 = digits[..pos]
            .iter()
            .enumerate()
            .map(|(i, &d)| d * (pos + 1 - i) as u32)
            .sum();
        let remainder = sum % 11;
        if remainder < 2 { 0 } else { 11 - remainder }
    };

    digits[9] == calc_digit(9) && digits[10] == calc_digit(10)
}

/// Validação básica de CNPJ (formato + dígitos não repetidos)
fn validate_cnpj(cnpj: &str) -> bool {
    let digits: Vec<u32> = cnpj.chars().filter_map(|c| c.to_digit(10)).collect();
    if digits.len() != 14 {
        return false;
    }
    // Rejeitar CNPJs com todos os dígitos iguais
    if digits.iter().all(|&d| d == digits[0]) {
        return false;
    }
    // Validação completa dos dígitos verificadores do CNPJ
    let weights_1: [u32; 12] = [5, 4, 3, 2, 9, 8, 7, 6, 5, 4, 3, 2];
    let sum1: u32 = digits[..12].iter().zip(weights_1.iter()).map(|(&d, &w)| d * w).sum();
    let rem1 = sum1 % 11;
    let d1 = if rem1 < 2 { 0 } else { 11 - rem1 };

    let weights_2: [u32; 13] = [6, 5, 4, 3, 2, 9, 8, 7, 6, 5, 4, 3, 2];
    let sum2: u32 = digits[..13].iter().zip(weights_2.iter()).map(|(&d, &w)| d * w).sum();
    let rem2 = sum2 % 11;
    let d2 = if rem2 < 2 { 0 } else { 11 - rem2 };

    digits[12] == d1 && digits[13] == d2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_email() {
        let checker = EgressChecker::new();
        let result = checker.check("Envie para teste@exemplo.com por favor");
        assert!(!result.findings.is_empty());
        assert_eq!(result.findings[0].category, "email");
        assert_eq!(result.findings[0].redacted_text, "***@exemplo.com");
        assert!(result.passed); // Email não é categoria bloqueante por padrão
    }

    #[test]
    fn test_detect_cpf_valid() {
        let checker = EgressChecker::new();
        // CPF válido: 529.982.247-25
        let result = checker.check("Meu CPF é 529.982.247-25");
        let cpf_findings: Vec<_> = result.findings.iter()
            .filter(|f| f.rule_name == "cpf_builtin")
            .collect();
        assert_eq!(cpf_findings.len(), 1);
        assert_eq!(cpf_findings[0].confidence, 0.99);
    }

    #[test]
    fn test_reject_invalid_cpf() {
        let checker = EgressChecker::new();
        // CPF inválido (dígitos verificadores errados): 123.456.789-00
        let result = checker.check("CPF: 123.456.789-00");
        let cpf_findings: Vec<_> = result.findings.iter()
            .filter(|f| f.rule_name == "cpf_builtin")
            .collect();
        assert_eq!(cpf_findings.len(), 0);
    }

    #[test]
    fn test_reject_all_same_digits_cpf() {
        let checker = EgressChecker::new();
        let result = checker.check("CPF: 111.111.111-11");
        let cpf_findings: Vec<_> = result.findings.iter()
            .filter(|f| f.rule_name == "cpf_builtin")
            .collect();
        assert_eq!(cpf_findings.len(), 0);
    }

    #[test]
    fn test_detect_cnpj_valid() {
        let checker = EgressChecker::new();
        // CNPJ válido: 11.222.333/0001-81
        let result = checker.check("CNPJ da empresa: 11.222.333/0001-81");
        let cnpj_findings: Vec<_> = result.findings.iter()
            .filter(|f| f.rule_name == "cnpj_builtin")
            .collect();
        assert_eq!(cnpj_findings.len(), 1);
    }

    #[test]
    fn test_detect_credit_card_luhn_valid() {
        let checker = EgressChecker::new();
        // Visa válido (Luhn passes): 4111 1111 1111 1111
        let result = checker.check("Cartão: 4111 1111 1111 1111");
        assert!(!result.passed); // Credit card é bloqueante por padrão
        let cc_findings: Vec<_> = result.findings.iter()
            .filter(|f| f.category == "credit_card")
            .collect();
        assert_eq!(cc_findings.len(), 1);
        assert!(cc_findings[0].redacted_text.ends_with("1111"));
    }

    #[test]
    fn test_reject_non_luhn_numbers() {
        let checker = EgressChecker::new();
        // Número longo que NÃO passa Luhn
        let result = checker.check("Número: 1234 5678 9012 3456");
        let cc_findings: Vec<_> = result.findings.iter()
            .filter(|f| f.category == "credit_card")
            .collect();
        assert_eq!(cc_findings.len(), 0);
    }

    #[test]
    fn test_detect_secret_api_key() {
        let checker = EgressChecker::new();
        let result = checker.check("Use api_key = 'sk-abc123def456ghi789jkl012mno345pqr'");
        assert!(!result.passed); // Secret é bloqueante por padrão
        let secret_findings: Vec<_> = result.findings.iter()
            .filter(|f| f.category == "secret")
            .collect();
        assert!(!secret_findings.is_empty());
    }

    #[test]
    fn test_detect_github_pat() {
        let checker = EgressChecker::new();
        let result = checker.check("Token: ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghij");
        let secret_findings: Vec<_> = result.findings.iter()
            .filter(|f| f.category == "secret")
            .collect();
        assert!(!secret_findings.is_empty());
    }

    #[test]
    fn test_health_keywords_low_confidence() {
        let checker = EgressChecker::new();
        let result = checker.check("O paciente precisa de tratamento hospitalar");
        let health_findings: Vec<_> = result.findings.iter()
            .filter(|f| f.category == "health_data")
            .collect();
        assert!(!health_findings.is_empty());
        assert!(health_findings.iter().all(|f| f.confidence < 0.7));
        assert!(result.passed); // Health data não é bloqueante por padrão
    }

    #[test]
    fn test_redacted_text_generation() {
        let checker = EgressChecker::new();
        let result = checker.check("Email: teste@exemplo.com e CPF: 529.982.247-25");
        assert!(result.redacted_text.is_some());
        let redacted = result.redacted_text.unwrap();
        assert!(redacted.contains("***@exemplo.com"));
        assert!(redacted.contains("***.***.***-**"));
        assert!(!redacted.contains("teste@"));
        assert!(!redacted.contains("529.982"));
    }

    #[test]
    fn test_no_findings_returns_none_redacted() {
        let checker = EgressChecker::new();
        let result = checker.check("Texto completamente seguro sem dados sensíveis");
        assert!(result.passed);
        assert!(result.findings.is_empty());
        assert!(result.redacted_text.is_none());
    }

    #[test]
    fn test_custom_rule() {
        let rule = EgressRule {
            id: "custom-1".into(),
            name: "internal_id".into(),
            pattern: r"INT-\d{6}".into(),
            category: "custom".into(),
            blocks: true,
            enabled: true,
        };
        let checker = EgressChecker::new().with_custom_rules(vec![rule]);
        let result = checker.check("Referência: INT-123456");
        let custom_findings: Vec<_> = result.findings.iter()
            .filter(|f| f.rule_name == "internal_id")
            .collect();
        assert_eq!(custom_findings.len(), 1);
        assert_eq!(custom_findings[0].redacted_text, "[internal_id REDACTED]");
    }

    #[test]
    fn test_disabled_custom_rule_ignored() {
        let rule = EgressRule {
            id: "custom-1".into(),
            name: "internal_id".into(),
            pattern: r"INT-\d{6}".into(),
            category: "custom".into(),
            blocks: true,
            enabled: false, // Desabilitada
        };
        let checker = EgressChecker::new().with_custom_rules(vec![rule]);
        let result = checker.check("Referência: INT-123456");
        let custom_findings: Vec<_> = result.findings.iter()
            .filter(|f| f.rule_name == "internal_id")
            .collect();
        assert_eq!(custom_findings.len(), 0);
    }

    #[test]
    fn test_blocked_categories_configurable() {
        let checker = EgressChecker::new()
            .with_blocked_categories(vec![SensitiveCategory::Email]);
        let result = checker.check("Envie para teste@exemplo.com");
        assert!(!result.passed); // Email agora é bloqueante
        assert!(result.blocked_categories.contains(&"email".to_string()));
    }

    #[test]
    fn test_multiple_findings_sorted_by_position() {
        let checker = EgressChecker::new();
        let result = checker.check("CPF: 529.982.247-25 e email: teste@exemplo.com");
        assert!(result.findings.len() >= 2);
        // Verificar ordenação por posição
        for window in result.findings.windows(2) {
            assert!(window[0].start <= window[1].start);
        }
    }

    #[test]
    fn test_luhn_check_valid() {
        assert!(luhn_check("4111111111111111")); // Visa
        assert!(luhn_check("5500000000000004")); // Mastercard
    }

    #[test]
    fn test_luhn_check_invalid() {
        assert!(!luhn_check("1234567890123456"));
        assert!(!luhn_check("4111111111111112")); // Visa modificado, falha Luhn
    }

    #[test]
    fn test_validate_cpf_valid() {
        assert!(validate_cpf("529.982.247-25"));
        assert!(validate_cpf("111.444.777-35")); // CPF válido com dígitos verificadores corretos
    }

    #[test]
    fn test_validate_cpf_invalid() {
        assert!(!validate_cpf("123.456.789-00"));
        assert!(!validate_cpf("111.111.111-11"));
        assert!(!validate_cpf("000.000.000-00"));
    }

    #[test]
    fn test_validate_cnpj_valid() {
        assert!(validate_cnpj("11.222.333/0001-81"));
    }

    #[test]
    fn test_validate_cnpj_invalid() {
        assert!(!validate_cnpj("11.111.111/1111-11"));
        assert!(!validate_cnpj("00.000.000/0000-00"));
    }
}