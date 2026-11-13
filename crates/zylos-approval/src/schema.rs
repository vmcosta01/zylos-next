//! Schema de dados para Approval Gates HITL
//!
//! Tipos serializáveis para persistência SQLite e comunicação via API REST.

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Decisão do usuário para uma ação que requer aprovação
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    /// Aprova apenas esta instância
    AllowOnce,
    /// Aprova para esta sessão (até fechar o cockpit)
    AllowSession,
    /// Aprova permanentemente para este bot + ferramenta
    AllowAlways,
    /// Recusa esta instância (pode pedir motivo)
    Refuse,
}

impl ApprovalDecision {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AllowOnce => "allow_once",
            Self::AllowSession => "allow_session",
            Self::AllowAlways => "allow_always",
            Self::Refuse => "refuse",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "allow_once" => Some(Self::AllowOnce),
            "allow_session" => Some(Self::AllowSession),
            "allow_always" => Some(Self::AllowAlways),
            "refuse" => Some(Self::Refuse),
            _ => None,
        }
    }
}

/// Estado de uma requisição de aprovação
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Refused,
    Expired,
    Cancelled,
}

impl ApprovalStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Refused => "refused",
            Self::Expired => "expired",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(Self::Pending),
            "approved" => Some(Self::Approved),
            "refused" => Some(Self::Refused),
            "expired" => Some(Self::Expired),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }
}

/// Categoria de ação que determina se precisa de approval
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionCategory {
    /// Envia mensagem para pessoa externa (sempre pede approval)
    SendMessage,
    /// Faz deploy/mutação de infraestrutura (sempre pede)
    Deploy,
    /// Faz pagamento/transferência financeira (sempre pede)
    Payment,
    /// Assina documento legal (sempre pede)
    SignDocument,
    /// Deleta recurso (pede se for produção)
    DeleteResource,
    /// Lê dados sensíveis (pede se PII)
    ReadSensitive,
    /// Ação de baixo risco (não pede por padrão)
    LowRisk,
}

impl ActionCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SendMessage => "send_message",
            Self::Deploy => "deploy",
            Self::Payment => "payment",
            Self::SignDocument => "sign_document",
            Self::DeleteResource => "delete_resource",
            Self::ReadSensitive => "read_sensitive",
            Self::LowRisk => "low_risk",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "send_message" => Some(Self::SendMessage),
            "deploy" => Some(Self::Deploy),
            "payment" => Some(Self::Payment),
            "sign_document" => Some(Self::SignDocument),
            "delete_resource" => Some(Self::DeleteResource),
            "read_sensitive" => Some(Self::ReadSensitive),
            "low_risk" => Some(Self::LowRisk),
            _ => None,
        }
    }

    /// Se true, sempre exige approval independente de regras
    pub fn always_requires_approval(&self) -> bool {
        matches!(
            self,
            Self::SendMessage | Self::Deploy | Self::Payment | Self::SignDocument
        )
    }
}

/// Uma requisição de aprovação pendente
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ApprovalRequest {
    pub id: String,
    pub bot_id: String,
    pub tool_name: String,
    pub action_category: String,
    pub summary: String,
    pub arguments_json: String,
    pub status: String,
    pub decision: Option<String>,
    pub refuse_reason: Option<String>,
    pub created_at: String,
    pub decided_at: Option<String>,
    pub expires_at: Option<String>,
    pub receipt_json: Option<String>,
}

impl ApprovalRequest {
    pub fn is_expired(&self) -> bool {
        if let Some(ref expires) = self.expires_at {
            // Comparação lexicográfica funciona para ISO 8601
            let now = chrono::Utc::now().to_rfc3339();
            return expires < &now;
        }
        false
    }

    pub fn is_pending(&self) -> bool {
        self.status == "pending"
    }

    pub fn action_category_enum(&self) -> Option<ActionCategory> {
        ActionCategory::parse(&self.action_category)
    }

    pub fn status_enum(&self) -> Option<ApprovalStatus> {
        ApprovalStatus::parse(&self.status)
    }

    pub fn decision_enum(&self) -> Option<ApprovalDecision> {
        self.decision.as_deref().and_then(ApprovalDecision::parse)
    }
}

/// Regra de approval persistente (allow-always)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ApprovalRule {
    pub id: String,
    pub bot_id: String,
    pub tool_name: String,
    pub action_category: String,
    pub arguments_pattern: Option<String>,
    pub decision: String,
    pub created_at: String,
    pub created_by: String,
}

/// Request body para decidir uma aprovação
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecideRequest {
    pub decision: String,
    pub refuse_reason: Option<String>,
}

/// Response body após decisão
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecideResponse {
    pub id: String,
    pub status: String,
    pub decided_at: String,
}

/// Request body para criar nova requisição de aprovação
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateApprovalRequest {
    pub bot_id: String,
    pub tool_name: String,
    pub action_category: String,
    pub summary: String,
    pub arguments_json: String,
    pub expires_in_seconds: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_approval_decision_roundtrip() {
        let decisions = [
            ApprovalDecision::AllowOnce,
            ApprovalDecision::AllowSession,
            ApprovalDecision::AllowAlways,
            ApprovalDecision::Refuse,
        ];
        for d in decisions {
            let s = d.as_str();
            let parsed = ApprovalDecision::parse(s).unwrap();
            assert_eq!(d, parsed);
        }
    }

    #[test]
    fn test_action_category_always_requires() {
        assert!(ActionCategory::SendMessage.always_requires_approval());
        assert!(ActionCategory::Deploy.always_requires_approval());
        assert!(ActionCategory::Payment.always_requires_approval());
        assert!(ActionCategory::SignDocument.always_requires_approval());
        assert!(!ActionCategory::DeleteResource.always_requires_approval());
        assert!(!ActionCategory::ReadSensitive.always_requires_approval());
        assert!(!ActionCategory::LowRisk.always_requires_approval());
    }

    #[test]
    fn test_status_roundtrip() {
        let statuses = [
            ApprovalStatus::Pending,
            ApprovalStatus::Approved,
            ApprovalStatus::Refused,
            ApprovalStatus::Expired,
            ApprovalStatus::Cancelled,
        ];
        for s in statuses {
            let str_val = s.as_str();
            let parsed = ApprovalStatus::parse(str_val).unwrap();
            assert_eq!(s, parsed);
        }
    }

    #[test]
    fn test_invalid_from_str_returns_none() {
        assert!(ApprovalDecision::parse("invalid").is_none());
        assert!(ApprovalStatus::parse("invalid").is_none());
        assert!(ActionCategory::parse("invalid").is_none());
    }
}