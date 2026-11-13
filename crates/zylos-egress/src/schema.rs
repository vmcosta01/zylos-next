//! Schema de dados para verificação de egress
//!
//! Tipos serializáveis para comunicação via API REST e integração no cockpit.

use serde::{Deserialize, Serialize};

/// Categoria de dado sensível detectado
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SensitiveCategory {
    /// Email addresses
    Email,
    /// Phone numbers (BR/US/international)
    Phone,
    /// Credit card numbers (Luhn validated)
    CreditCard,
    /// CPF/CNPJ (Brazilian tax IDs)
    BrazilianTaxId,
    /// API keys, tokens, passwords
    Secret,
    /// Health information keywords
    HealthData,
    /// Financial account numbers
    FinancialAccount,
    /// IP addresses
    IpAddress,
    /// Custom patterns defined by user
    Custom,
}

impl SensitiveCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Email => "email",
            Self::Phone => "phone",
            Self::CreditCard => "credit_card",
            Self::BrazilianTaxId => "brazilian_tax_id",
            Self::Secret => "secret",
            Self::HealthData => "health_data",
            Self::FinancialAccount => "financial_account",
            Self::IpAddress => "ip_address",
            Self::Custom => "custom",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "email" => Some(Self::Email),
            "phone" => Some(Self::Phone),
            "credit_card" => Some(Self::CreditCard),
            "brazilian_tax_id" => Some(Self::BrazilianTaxId),
            "secret" => Some(Self::Secret),
            "health_data" => Some(Self::HealthData),
            "financial_account" => Some(Self::FinancialAccount),
            "ip_address" => Some(Self::IpAddress),
            "custom" => Some(Self::Custom),
            _ => None,
        }
    }
}

/// Uma detecção de dado sensível
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensitiveFinding {
    pub category: String,
    pub matched_text: String,
    pub redacted_text: String,
    pub start: usize,
    pub end: usize,
    pub confidence: f32,
    pub rule_name: String,
}

/// Resultado de uma verificação de egress
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EgressCheckResult {
    pub passed: bool,
    pub findings: Vec<SensitiveFinding>,
    pub redacted_text: Option<String>,
    pub blocked_categories: Vec<String>,
}

/// Regra customizada de detecção
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EgressRule {
    pub id: String,
    pub name: String,
    pub pattern: String,
    pub category: String,
    pub blocks: bool,
    pub enabled: bool,
}

/// Request body para verificação de egress
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EgressCheckRequest {
    pub text: String,
    pub bot_id: Option<String>,
    pub destination: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_category_roundtrip() {
        let categories = [
            SensitiveCategory::Email,
            SensitiveCategory::Phone,
            SensitiveCategory::CreditCard,
            SensitiveCategory::BrazilianTaxId,
            SensitiveCategory::Secret,
            SensitiveCategory::HealthData,
            SensitiveCategory::FinancialAccount,
            SensitiveCategory::IpAddress,
            SensitiveCategory::Custom,
        ];
        for c in categories {
            let s = c.as_str();
            let parsed = SensitiveCategory::parse(s).unwrap();
            assert_eq!(c, parsed);
        }
    }

    #[test]
    fn test_invalid_category_returns_none() {
        assert!(SensitiveCategory::parse("invalid").is_none());
    }
}