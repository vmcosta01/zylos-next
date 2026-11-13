//! Password Masking Middleware
//!
//! Intercepta payloads JSON antes da persistência no transcript e mascara
//! campos sensíveis (password, senha, secret, token, api_key, etc.) para
//! prevenir vazamento em plaintext nos logs e histórico de conversas.
//!
//! ## Arquitetura
//! - Lista de field names sensíveis configurável
//! - Mascara valores em qualquer nível de aninhamento JSON
//! - Audit log registra que campo foi mascarado (sem valor original)
//! - Zero impacto em performance para payloads sem campos sensíveis
//!
//! ## Related Documents
//! - Implementation Plan: [IMPLEMENTATION_PLAN_DETAILED.md](../../docs/IMPLEMENTATION_PLAN_DETAILED.md) §2.3
//! - Document Compliance: [DOCUMENT_COMPLIANCE_FASE9.md](../../docs/DOCUMENT_COMPLIANCE_FASE9.md)

use serde_json::Value;
use std::collections::HashSet;
use std::sync::LazyLock;

/// Field names que devem ser mascarados (case-insensitive matching)
static SENSITIVE_FIELDS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "password",
        "passwd",
        "senha",
        "secret",
        "token",
        "api_key",
        "apikey",
        "api_secret",
        "access_token",
        "refresh_token",
        "private_key",
        "credentials",
        "auth",
        "authorization",
        "bearer",
        "client_secret",
        "connection_string",
        "database_url",
        "db_password",
        "smtp_password",
        "encryption_key",
        "signing_key",
    ]
    .iter()
    .copied()
    .collect()
});

const MASKED_VALUE: &str = "********";

/// Verifica se um field name é sensível (case-insensitive)
pub fn is_sensitive_field(field_name: &str) -> bool {
    let lower = field_name.to_lowercase();
    SENSITIVE_FIELDS.contains(lower.as_str())
}

/// Mascara campos sensíveis em um Value JSON recursivamente.
/// Retorna `(masked_value, fields_masked_count)`.
pub fn mask_sensitive_fields(value: &mut Value) -> usize {
    match value {
        Value::Object(map) => {
            let mut count = 0;
            for (key, val) in map.iter_mut() {
                if is_sensitive_field(key) && val.is_string() {
                    *val = Value::String(MASKED_VALUE.to_string());
                    count += 1;
                } else {
                    count += mask_sensitive_fields(val);
                }
            }
            count
        }
        Value::Array(arr) => arr.iter_mut().map(mask_sensitive_fields).sum(),
        _ => 0,
    }
}

/// Versão não-mutante que retorna novo Value com campos mascarados
pub fn mask_sensitive_fields_clone(value: &Value) -> (Value, usize) {
    let mut cloned = value.clone();
    let count = mask_sensitive_fields(&mut cloned);
    (cloned, count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_is_sensitive_field_case_insensitive() {
        assert!(is_sensitive_field("password"));
        assert!(is_sensitive_field("PASSWORD"));
        assert!(is_sensitive_field("Password"));
        assert!(is_sensitive_field("senha"));
        assert!(is_sensitive_field("SENHA"));
        assert!(is_sensitive_field("api_key"));
        assert!(is_sensitive_field("API_KEY"));
        assert!(is_sensitive_field("token"));
        assert!(is_sensitive_field("secret"));
        assert!(!is_sensitive_field("username"));
        assert!(!is_sensitive_field("email"));
        assert!(!is_sensitive_field("message"));
    }

    #[test]
    fn test_mask_top_level_password() {
        let mut val = json!({"username": "admin", "password": "supersecret123"});
        let count = mask_sensitive_fields(&mut val);
        assert_eq!(count, 1);
        assert_eq!(val["password"], "********");
        assert_eq!(val["username"], "admin");
    }

    #[test]
    fn test_mask_nested_fields() {
        let mut val = json!({
            "user": {
                "name": "test",
                "credentials": {
                    "password": "secret",
                    "api_key": "sk-abc123"
                }
            }
        });
        let count = mask_sensitive_fields(&mut val);
        assert_eq!(count, 2);
        assert_eq!(val["user"]["credentials"]["password"], "********");
        assert_eq!(val["user"]["credentials"]["api_key"], "********");
        assert_eq!(val["user"]["name"], "test");
    }

    #[test]
    fn test_mask_in_array() {
        let mut val = json!([
            {"password": "pass1"},
            {"username": "user2", "token": "tok123"}
        ]);
        let count = mask_sensitive_fields(&mut val);
        assert_eq!(count, 2);
        assert_eq!(val[0]["password"], "********");
        assert_eq!(val[1]["token"], "********");
        assert_eq!(val[1]["username"], "user2");
    }

    #[test]
    fn test_no_sensitive_fields_returns_zero() {
        let mut val = json!({"username": "admin", "email": "test@example.com", "message": "hello"});
        let count = mask_sensitive_fields(&mut val);
        assert_eq!(count, 0);
        assert_eq!(val["username"], "admin");
    }

    #[test]
    fn test_non_string_values_not_masked() {
        let mut val = json!({"password": 12345, "token": true, "secret": null});
        let count = mask_sensitive_fields(&mut val);
        assert_eq!(count, 0); // Only strings are masked
        assert_eq!(val["password"], 12345);
    }

    #[test]
    fn test_mask_senha_portuguese() {
        let mut val = json!({"usuario": "admin", "senha": "minhasenha123"});
        let count = mask_sensitive_fields(&mut val);
        assert_eq!(count, 1);
        assert_eq!(val["senha"], "********");
    }

    #[test]
    fn test_mask_clone_does_not_mutate_original() {
        let val = json!({"password": "secret", "user": "admin"});
        let (masked, count) = mask_sensitive_fields_clone(&val);
        assert_eq!(count, 1);
        assert_eq!(masked["password"], "********");
        assert_eq!(val["password"], "secret"); // Original unchanged
    }

    #[test]
    fn test_deeply_nested_masking() {
        let mut val = json!({
            "level1": {
                "level2": {
                    "level3": {
                        "db_password": "deep_secret"
                    }
                }
            }
        });
        let count = mask_sensitive_fields(&mut val);
        assert_eq!(count, 1);
        assert_eq!(val["level1"]["level2"]["level3"]["db_password"], "********");
    }

    #[test]
    fn test_multiple_sensitive_in_same_object() {
        let mut val = json!({
            "password": "p1",
            "token": "t1",
            "api_key": "k1",
            "name": "safe"
        });
        let count = mask_sensitive_fields(&mut val);
        assert_eq!(count, 3);
        assert_eq!(val["password"], "********");
        assert_eq!(val["token"], "********");
        assert_eq!(val["api_key"], "********");
        assert_eq!(val["name"], "safe");
    }
}