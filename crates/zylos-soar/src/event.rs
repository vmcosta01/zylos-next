//! Core event types for the SOAR engine.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Severity levels for security events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// Enumeração Severity.
pub enum Severity {
    /// Variante Critical.
    Critical,
    /// Variante High.
    High,
    /// Variante Medium.
    Medium,
    /// Variante Low.
    Low,
    /// Variante Info.
    Info,
}

impl Severity {
    /// Returns numeric priority (lower = more urgent).
    pub fn priority(&self) -> u8 {
        match self {
            Self::Critical => 0,
            Self::High => 1,
            Self::Medium => 2,
            Self::Low => 3,
            Self::Info => 4,
        }
    }
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Critical => write!(f, "Critical"),
            Self::High => write!(f, "High"),
            Self::Medium => write!(f, "Medium"),
            Self::Low => write!(f, "Low"),
            Self::Info => write!(f, "Info"),
        }
    }
}

/// Category of security event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// Enumeração EventCategory.
pub enum EventCategory {
    /// Variante Breach.
    Breach,
    /// Variante Anomaly.
    Anomaly,
    /// Variante Vulnerability.
    Vulnerability,
    /// Variante Compliance.
    Compliance,
    /// Variante FalsePositive.
    FalsePositive,
}

impl std::fmt::Display for EventCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Breach => write!(f, "Breach"),
            Self::Anomaly => write!(f, "Anomaly"),
            Self::Vulnerability => write!(f, "Vulnerability"),
            Self::Compliance => write!(f, "Compliance"),
            Self::FalsePositive => write!(f, "FalsePositive"),
        }
    }
}

/// Normalized security event from any source.
#[derive(Debug, Clone, Serialize, Deserialize)]
/// Estrutura ZylosSecurityEvent.
pub struct ZylosSecurityEvent {
    /// Campo id.
    pub id: Uuid,
    /// Campo source.
    pub source: String,
    /// Campo severity.
    pub severity: Severity,
    /// Campo category.
    pub category: EventCategory,
    /// Campo title.
    pub title: String,
    /// Campo description.
    pub description: String,
    /// Campo affected assets.
    pub affected_assets: Vec<String>,
    /// Campo raw payload.
    pub raw_payload: serde_json::Value,
    /// Campo received at.
    pub received_at: DateTime<Utc>,
}

impl ZylosSecurityEvent {
    /// Create a new event with auto-generated ID and current timestamp.
    pub fn new(
        source: impl Into<String>,
        severity: Severity,
        category: EventCategory,
        title: impl Into<String>,
        description: impl Into<String>,
        affected_assets: Vec<String>,
        raw_payload: serde_json::Value,
    ) -> Self {
        // Variante Self.
        Self {
            id: Uuid::new_v4(),
            source: source.into(),
            severity,
            category,
            title: title.into(),
            description: description.into(),
            affected_assets,
            raw_payload,
            received_at: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_priority_ordering() {
        assert!(Severity::Critical.priority() < Severity::High.priority());
        assert!(Severity::High.priority() < Severity::Medium.priority());
        assert!(Severity::Medium.priority() < Severity::Low.priority());
        assert!(Severity::Low.priority() < Severity::Info.priority());
    }

    #[test]
    fn event_serialization_roundtrip() {
        let event = ZylosSecurityEvent::new(
            "prometheus",
            Severity::Critical,
            EventCategory::Breach,
            "Test breach",
            "Detected unauthorized access",
            vec!["host-01".to_string()],
            serde_json::json!({"alert": "test"}),
        );
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: ZylosSecurityEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, event.id);
        assert_eq!(deserialized.severity, Severity::Critical);
    }
}