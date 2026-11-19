//! Typed event system with serde tagged enums and schema registry.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;
use uuid::Uuid;

/// Versioned Zylos event envelope — all events carry metadata for routing,
/// persistence, replay and observability.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
/// Estrutura EventEnvelope.
pub struct EventEnvelope {
    /// Campo id.
    pub id: String,
    /// Campo topic.
    pub topic: String,
    /// Campo timestamp.
    pub timestamp: DateTime<Utc>,
    /// Campo sequence.
    pub sequence: u64,
    /// Campo schema version.
    pub schema_version: u32,
    #[serde(flatten)]
    /// Campo payload.
    pub payload: ZylosEvent,
}

impl EventEnvelope {
    /// Função new.
    pub fn new(topic: impl Into<String>, payload: ZylosEvent, schema_version: u32) -> Self {
        // Variante Self.
        Self {
            id: Uuid::new_v4().to_string(),
            topic: topic.into(),
            timestamp: Utc::now(),
            sequence: 0, // assigned by persistence layer
            schema_version,
            payload,
        }
    }
}

/// Core event taxonomy — each variant is independently versioned via
/// the envelope's `schema_version` field.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(tag = "event_type", content = "data")]
/// Enumeração ZylosEvent.
pub enum ZylosEvent {
    // Variante AgentStateChanged.
    AgentStateChanged(AgentStateChanged),
    // Variante TaskCompleted.
    TaskCompleted(TaskCompleted),
    // Variante MemoryUpdated.
    MemoryUpdated(MemoryUpdated),
    // Variante ApprovalRequested.
    ApprovalRequested(ApprovalRequested),
    // Variante DagProgressChanged.
    DagProgressChanged(DagProgressChanged),
    // Variante SkillInvoked.
    SkillInvoked(SkillInvoked),
    // Variante Custom.
    Custom(CustomEvent),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
/// Estrutura AgentStateChanged.
pub struct AgentStateChanged {
    /// Campo agent id.
    pub agent_id: String,
    /// Campo previous state.
    pub previous_state: String,
    /// Campo new state.
    pub new_state: String,
    /// Campo reason.
    pub reason: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
/// Estrutura TaskCompleted.
pub struct TaskCompleted {
    /// Campo task id.
    pub task_id: String,
    /// Campo agent id.
    pub agent_id: String,
    /// Campo outcome.
    pub outcome: String,
    /// Campo duration ms.
    pub duration_ms: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
/// Estrutura MemoryUpdated.
pub struct MemoryUpdated {
    /// Campo memory key.
    pub memory_key: String,
    /// Campo operation.
    pub operation: String, // "insert", "update", "delete"
    /// Campo namespace.
    pub namespace: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
/// Estrutura ApprovalRequested.
pub struct ApprovalRequested {
    /// Campo approval id.
    pub approval_id: String,
    /// Campo requester id.
    pub requester_id: String,
    /// Campo action description.
    pub action_description: String,
    /// Campo expires at.
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
/// Estrutura DagProgressChanged.
pub struct DagProgressChanged {
    /// Campo dag id.
    pub dag_id: String,
    /// Campo node id.
    pub node_id: String,
    /// Campo status.
    pub status: String,
    /// Campo completed nodes.
    pub completed_nodes: u32,
    /// Campo total nodes.
    pub total_nodes: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
/// Estrutura SkillInvoked.
pub struct SkillInvoked {
    /// Campo skill name.
    pub skill_name: String,
    /// Campo agent id.
    pub agent_id: String,
    /// Campo arguments.
    pub arguments: serde_json::Value,
    /// Campo invocation id.
    pub invocation_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
/// Estrutura CustomEvent.
pub struct CustomEvent {
    /// Campo name.
    pub name: String,
    /// Campo payload.
    pub payload: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Schema Registry
// ---------------------------------------------------------------------------

/// Nomes de eventos `Custom` tratados como aviso pelo cockpit (toast e atualização do HITL).
pub const NOTICE_EVENT_NAMES: [&str; 3] = ["ApprovalResolved", "ApprovalTimeout", "SentinelAlert"];

/// In-process JSON Schema registry. Producers validate before publish;
/// consumers receive `schema_version` in the envelope header.
pub struct SchemaRegistry {
    schemas: RwLock<HashMap<(String, u32), serde_json::Value>>,
}

impl SchemaRegistry {
    /// Função new.
    pub fn new() -> Self {
        let mut reg = Self {
            schemas: RwLock::new(HashMap::new()),
        };
        reg.register_defaults();
        reg
    }

    /// Register a schema for a given event type name + version.
    pub fn register(&self, event_type: &str, version: u32, schema: serde_json::Value) {
        let mut map = self.schemas.write().unwrap();
        map.insert((event_type.to_string(), version), schema);
    }

    /// Validate a payload against the registered schema. Returns Ok(()) on
    /// success or an error description on mismatch / missing schema.
    pub fn validate(
        &self,
        event_type: &str,
        version: u32,
        value: &serde_json::Value,
    ) -> Result<(), String> {
        let map = self.schemas.read().unwrap();
        let _schema = map
            .get(&(event_type.to_string(), version))
            .ok_or_else(|| format!("no schema registered for {} v{}", event_type, version))?;
        // Lightweight structural check: ensure the value is an object when expected.
        // Full JSON-Schema validation would require the `jsonschema` crate; we keep
        // deps minimal and rely on serde round-trip + compile-time types for safety.
        if !value.is_object() && event_type != "Custom" {
            return Err(format!(
                "expected object for {} v{}, got {}",
                event_type, version, value
            ));
        }
        // Variante Ok.
        Ok(())
    }

    /// Retrieve the raw JSON Schema for introspection / API exposure.
    pub fn get_schema(&self, event_type: &str, version: u32) -> Option<serde_json::Value> {
        let map = self.schemas.read().unwrap();
        map.get(&(event_type.to_string(), version)).cloned()
    }

    fn register_defaults(&mut self) {
        use schemars::schema_for;
        let pairs: Vec<(&str, serde_json::Value)> = vec![
            (
                "AgentStateChanged",
                serde_json::to_value(schema_for!(AgentStateChanged)).unwrap(),
            ),
            (
                "TaskCompleted",
                serde_json::to_value(schema_for!(TaskCompleted)).unwrap(),
            ),
            (
                "MemoryUpdated",
                serde_json::to_value(schema_for!(MemoryUpdated)).unwrap(),
            ),
            (
                "ApprovalRequested",
                serde_json::to_value(schema_for!(ApprovalRequested)).unwrap(),
            ),
            (
                "DagProgressChanged",
                serde_json::to_value(schema_for!(DagProgressChanged)).unwrap(),
            ),
            (
                "SkillInvoked",
                serde_json::to_value(schema_for!(SkillInvoked)).unwrap(),
            ),
            (
                "Custom",
                serde_json::to_value(schema_for!(CustomEvent)).unwrap(),
            ),
        ];
        let custom = serde_json::to_value(schema_for!(CustomEvent)).unwrap();
        let mut map = self.schemas.write().unwrap();
        for (name, schema) in pairs {
            map.insert((name.to_string(), 1), schema);
        }
        // Avisos que o cockpit mostra como toast (publicados como `Custom` com o nome abaixo).
        for name in NOTICE_EVENT_NAMES {
            map.insert((name.to_string(), 1), custom.clone());
        }
    }
}

impl Default for SchemaRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_envelope_serde_roundtrip() {
        let evt = ZylosEvent::AgentStateChanged(AgentStateChanged {
            agent_id: "a1".into(),
            previous_state: "idle".into(),
            new_state: "running".into(),
            reason: None,
        });
        let env = EventEnvelope::new("agents.a1.state", evt, 1);
        let json = serde_json::to_string(&env).unwrap();
        let back: EventEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(env.topic, back.topic);
        assert_eq!(env.payload, back.payload);
    }

    #[test]
    fn test_custom_event_roundtrip() {
        let evt = ZylosEvent::Custom(CustomEvent {
            name: "deploy.started".into(),
            payload: serde_json::json!({"env": "staging"}),
        });
        let env = EventEnvelope::new("custom.deploy", evt, 1);
        let json = serde_json::to_string(&env).unwrap();
        let back: EventEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(env, back);
    }

    #[test]
    fn test_schema_registry_validate() {
        let reg = SchemaRegistry::new();
        let val = serde_json::json!({
            "agent_id": "a1",
            "previous_state": "idle",
            "new_state": "running"
        });
        assert!(reg.validate("AgentStateChanged", 1, &val).is_ok());
        assert!(reg.validate("AgentStateChanged", 99, &val).is_err());
    }
}
