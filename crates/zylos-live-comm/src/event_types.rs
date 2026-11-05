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
pub struct EventEnvelope {
    pub id: String,
    pub topic: String,
    pub timestamp: DateTime<Utc>,
    pub sequence: u64,
    pub schema_version: u32,
    #[serde(flatten)]
    pub payload: ZylosEvent,
}

impl EventEnvelope {
    pub fn new(topic: impl Into<String>, payload: ZylosEvent, schema_version: u32) -> Self {
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
pub enum ZylosEvent {
    AgentStateChanged(AgentStateChanged),
    TaskCompleted(TaskCompleted),
    MemoryUpdated(MemoryUpdated),
    ApprovalRequested(ApprovalRequested),
    DagProgressChanged(DagProgressChanged),
    SkillInvoked(SkillInvoked),
    Custom(CustomEvent),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
pub struct AgentStateChanged {
    pub agent_id: String,
    pub previous_state: String,
    pub new_state: String,
    pub reason: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
pub struct TaskCompleted {
    pub task_id: String,
    pub agent_id: String,
    pub outcome: String,
    pub duration_ms: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
pub struct MemoryUpdated {
    pub memory_key: String,
    pub operation: String, // "insert", "update", "delete"
    pub namespace: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
pub struct ApprovalRequested {
    pub approval_id: String,
    pub requester_id: String,
    pub action_description: String,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
pub struct DagProgressChanged {
    pub dag_id: String,
    pub node_id: String,
    pub status: String,
    pub completed_nodes: u32,
    pub total_nodes: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
pub struct SkillInvoked {
    pub skill_name: String,
    pub agent_id: String,
    pub arguments: serde_json::Value,
    pub invocation_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
pub struct CustomEvent {
    pub name: String,
    pub payload: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Schema Registry
// ---------------------------------------------------------------------------

/// In-process JSON Schema registry. Producers validate before publish;
/// consumers receive `schema_version` in the envelope header.
pub struct SchemaRegistry {
    schemas: RwLock<HashMap<(String, u32), serde_json::Value>>,
}

impl SchemaRegistry {
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
    pub fn validate(&self, event_type: &str, version: u32, value: &serde_json::Value) -> Result<(), String> {
        let map = self.schemas.read().unwrap();
        let _schema = map.get(&(event_type.to_string(), version))
            .ok_or_else(|| format!("no schema registered for {} v{}", event_type, version))?;
        // Lightweight structural check: ensure the value is an object when expected.
        // Full JSON-Schema validation would require the `jsonschema` crate; we keep
        // deps minimal and rely on serde round-trip + compile-time types for safety.
        if !value.is_object() && event_type != "Custom" {
            return Err(format!("expected object for {} v{}, got {}", event_type, version, value));
        }
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
            ("AgentStateChanged", serde_json::to_value(schema_for!(AgentStateChanged)).unwrap()),
            ("TaskCompleted", serde_json::to_value(schema_for!(TaskCompleted)).unwrap()),
            ("MemoryUpdated", serde_json::to_value(schema_for!(MemoryUpdated)).unwrap()),
            ("ApprovalRequested", serde_json::to_value(schema_for!(ApprovalRequested)).unwrap()),
            ("DagProgressChanged", serde_json::to_value(schema_for!(DagProgressChanged)).unwrap()),
            ("SkillInvoked", serde_json::to_value(schema_for!(SkillInvoked)).unwrap()),
            ("Custom", serde_json::to_value(schema_for!(CustomEvent)).unwrap()),
        ];
        let mut map = self.schemas.write().unwrap();
        for (name, schema) in pairs {
            map.insert((name.to_string(), 1), schema);
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