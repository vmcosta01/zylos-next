//! Schema de dados para Action Audit Log
//!
//! Tipos serializáveis para persistência SQLite e comunicação via API REST.
//! Arguments são armazenados como hash SHA-256 (não plaintext) para segurança.

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Uma entrada no audit log
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AuditEntry {
    pub id: String,
    pub bot_id: String,
    pub action_type: String,
    pub target: Option<String>,
    pub approval_status: String,
    pub approval_decision: Option<String>,
    pub arguments_hash: String,
    pub result_status: Option<String>,
    pub result_summary: Option<String>,
    pub created_at: String,
    pub decided_at: Option<String>,
    pub executed_at: Option<String>,
}

/// Estatísticas agregadas por período
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditStats {
    pub period: String,
    pub total_actions: i64,
    pub approved: i64,
    pub refused: i64,
    pub auto_approved: i64,
    pub expired: i64,
    pub by_action_type: Vec<ActionTypeCount>,
}

/// Contagem por tipo de ação
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ActionTypeCount {
    pub action_type: String,
    pub count: i64,
}

/// Filtros para consulta paginada
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditFilter {
    pub bot_id: Option<String>,
    pub action_type: Option<String>,
    pub approval_status: Option<String>,
    pub since: Option<String>,
    pub until: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

impl Default for AuditFilter {
    fn default() -> Self {
        Self {
            bot_id: None,
            action_type: None,
            approval_status: None,
            since: None,
            until: None,
            limit: Some(50),
            offset: Some(0),
        }
    }
}

/// Request body para registrar nova ação
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAuditEntry {
    pub bot_id: String,
    pub action_type: String,
    pub target: Option<String>,
    pub approval_status: String,
    pub approval_decision: Option<String>,
    pub arguments_json: String,
    pub result_status: Option<String>,
    pub result_summary: Option<String>,
}

/// Response de listagem paginada
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditListResponse {
    pub entries: Vec<AuditEntry>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_filter_default() {
        let filter = AuditFilter::default();
        assert_eq!(filter.limit, Some(50));
        assert_eq!(filter.offset, Some(0));
        assert!(filter.bot_id.is_none());
        assert!(filter.since.is_none());
    }

    #[test]
    fn test_create_audit_entry_serialization() {
        let entry = CreateAuditEntry {
            bot_id: "bot-1".into(),
            action_type: "send_email".into(),
            target: Some("client@example.com".into()),
            approval_status: "approved".into(),
            approval_decision: Some("allow_once".into()),
            arguments_json: r#"{"to":"client@example.com"}"#.into(),
            result_status: Some("success".into()),
            result_summary: Some("Email sent".into()),
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("bot-1"));
        assert!(json.contains("send_email"));
    }
}