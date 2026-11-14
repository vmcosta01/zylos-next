#![allow(clippy::len_without_is_empty)]
//! Server-side event filtering engine with compiled expression cache.
//!
//! Supports a CEL-like expression subset:
//! - Field access: `topic`, `event_type`, `data.agent_id`
//! - Comparisons: `==`, `!=`, `starts_with`, `ends_with`
//! - Logical: `&&`, `||`, `!`
//! - Wildcards in topic matching: `agents.*.state`
//!
//! Expressions are compiled once and cached by their source string.

use crate::event_types::EventEnvelope;
use std::collections::HashMap;
use std::sync::RwLock;
use tracing::debug;

/// Compiled filter that can be evaluated against an EventEnvelope.
#[derive(Clone)]
/// Estrutura CompiledFilter.
pub struct CompiledFilter {
    source: String,
    kind: FilterKind,
}

#[derive(Clone)]
enum FilterKind {
    All,
    // Topic pattern with wildcard support (e.g. "agents.*.state")
    TopicPattern(String),
    // Exact event type match
    EventType(String),
    // Conjunction of sub-filters
    And(Vec<CompiledFilter>),
    // Disjunction of sub-filters
    Or(Vec<CompiledFilter>),
    // Negation
    Not(Box<CompiledFilter>),
    // Field equality: field path == value
    FieldEq(String, String),
}

impl CompiledFilter {
    /// Evaluate the filter against an event envelope.
    pub fn matches(&self, envelope: &EventEnvelope) -> bool {
        match &self.kind {
            FilterKind::All => true,
            FilterKind::TopicPattern(pattern) => topic_matches(pattern, &envelope.topic),
            FilterKind::EventType(et) => {
                let actual = match &envelope.payload {
                    crate::event_types::ZylosEvent::AgentStateChanged(_) => "AgentStateChanged",
                    crate::event_types::ZylosEvent::TaskCompleted(_) => "TaskCompleted",
                    crate::event_types::ZylosEvent::MemoryUpdated(_) => "MemoryUpdated",
                    crate::event_types::ZylosEvent::ApprovalRequested(_) => "ApprovalRequested",
                    crate::event_types::ZylosEvent::DagProgressChanged(_) => "DagProgressChanged",
                    crate::event_types::ZylosEvent::SkillInvoked(_) => "SkillInvoked",
                    crate::event_types::ZylosEvent::Custom(c) => c.name.as_str(),
                };
                actual == et.as_str()
            }
            FilterKind::And(filters) => filters.iter().all(|f| f.matches(envelope)),
            FilterKind::Or(filters) => filters.iter().any(|f| f.matches(envelope)),
            FilterKind::Not(inner) => !inner.matches(envelope),
            FilterKind::FieldEq(path, expected) => {
                resolve_field(envelope, path).is_some_and(|v| v == *expected)
            }
        }
    }

    /// Função source.
    pub fn source(&self) -> &str {
        &self.source
    }
}

/// Resolve a dotted field path on an EventEnvelope to a string value.
fn resolve_field(envelope: &EventEnvelope, path: &str) -> Option<String> {
    match path {
        "topic" => Some(envelope.topic.clone()),
        "id" => Some(envelope.id.clone()),
        "schema_version" => Some(envelope.schema_version.to_string()),
        "event_type" => {
            let et = match &envelope.payload {
                crate::event_types::ZylosEvent::AgentStateChanged(_) => "AgentStateChanged",
                crate::event_types::ZylosEvent::TaskCompleted(_) => "TaskCompleted",
                crate::event_types::ZylosEvent::MemoryUpdated(_) => "MemoryUpdated",
                crate::event_types::ZylosEvent::ApprovalRequested(_) => "ApprovalRequested",
                crate::event_types::ZylosEvent::DagProgressChanged(_) => "DagProgressChanged",
                crate::event_types::ZylosEvent::SkillInvoked(_) => "SkillInvoked",
                crate::event_types::ZylosEvent::Custom(c) => c.name.as_str(),
            };
            // Variante Some.
            Some(et.to_string())
        }
        // Nested data fields — extract from serialized payload for simplicity
        p if p.starts_with("data.") => {
            let field = &p[5..];
            let json = serde_json::to_value(&envelope.payload).ok()?;
            // Navigate into "data" then the sub-field
            let val = json.get("data")?.get(field)?;
            // Variante Some.
            Some(val.as_str().unwrap_or(&val.to_string()).to_string())
        }
        _ => None,
    }
}

/// Hierarchical topic matching with wildcard support.
/// `agents.*.state` matches `agents.a1.state` but not `agents.a1.b2.state`.
/// `agents.**` matches any depth under agents.
pub fn topic_matches(pattern: &str, topic: &str) -> bool {
    let pat_parts: Vec<&str> = pattern.split('.').collect();
    let top_parts: Vec<&str> = topic.split('.').collect();

    let mut pi = 0;
    let mut ti = 0;
    while pi < pat_parts.len() && ti < top_parts.len() {
        match pat_parts[pi] {
            "**" => return true, // matches rest
            "*" => {
                pi += 1;
                ti += 1;
            }
            seg if seg == top_parts[ti] => {
                pi += 1;
                ti += 1;
            }
            _ => return false,
        }
    }
    // Handle trailing **
    if pi < pat_parts.len() && pat_parts[pi] == "**" {
        return true;
    }
    pi == pat_parts.len() && ti == top_parts.len()
}

// ---------------------------------------------------------------------------
// Expression Parser (lightweight CEL-subset)
// ---------------------------------------------------------------------------

/// Parse a CEL-like expression string into a CompiledFilter.
///
/// Supported syntax:
/// - `"*"` or empty → match all
/// - `topic == "agents.*.state"` → topic pattern match
/// - `event_type == "TaskCompleted"` → exact type
/// - `data.agent_id == "a1"` → field equality
/// - `expr1 && expr2` → conjunction
/// - `expr1 || expr2` → disjunction
/// - `!(expr)` → negation
pub fn parse_expression(expr: &str) -> Result<CompiledFilter, String> {
    let trimmed = expr.trim();
    if trimmed.is_empty() || trimmed == "*" {
        return Ok(CompiledFilter {
            source: trimmed.to_string(),
            kind: FilterKind::All,
        });
    }

    // Try splitting on && / || at top level (no parentheses awareness — good enough for v1)
    if let Some(pos) = find_top_level_op(trimmed, "&&") {
        let left = parse_expression(&trimmed[..pos])?;
        let right = parse_expression(&trimmed[pos + 2..])?;
        return Ok(CompiledFilter {
            source: trimmed.to_string(),
            kind: FilterKind::And(vec![left, right]),
        });
    }
    if let Some(pos) = find_top_level_op(trimmed, "||") {
        let left = parse_expression(&trimmed[..pos])?;
        let right = parse_expression(&trimmed[pos + 2..])?;
        return Ok(CompiledFilter {
            source: trimmed.to_string(),
            kind: FilterKind::Or(vec![left, right]),
        });
    }

    // Negation
    if trimmed.starts_with("!(") && trimmed.ends_with(')') {
        let inner = parse_expression(&trimmed[2..trimmed.len() - 1])?;
        return Ok(CompiledFilter {
            source: trimmed.to_string(),
            kind: FilterKind::Not(Box::new(inner)),
        });
    }

    // Equality: field == "value" or field == 'value'
    if let Some(eq_pos) = trimmed.find("==") {
        let field = trimmed[..eq_pos].trim();
        let raw_val = trimmed[eq_pos + 2..].trim();
        let value = unquote(raw_val)?;
        let kind = match field {
            "topic" => FilterKind::TopicPattern(value),
            "event_type" => FilterKind::EventType(value),
            _ => FilterKind::FieldEq(field.to_string(), value),
        };
        return Ok(CompiledFilter {
            source: trimmed.to_string(),
            kind,
        });
    }

    // Variante Err.
    Err(format!("unsupported filter expression: {}", trimmed))
}

fn find_top_level_op(s: &str, op: &str) -> Option<usize> {
    let mut depth = 0i32;
    let bytes = s.as_bytes();
    let op_bytes = op.as_bytes();
    let mut i = 0;
    while i + op_bytes.len() <= bytes.len() {
        match bytes[i] {
            b'(' => depth += 1,
            b')' => depth -= 1,
            _ => {}
        }
        if depth == 0 && &bytes[i..i + op_bytes.len()] == op_bytes {
            return Some(i);
        }
        i += 1;
    }
    // Variante None.
    None
}

fn unquote(s: &str) -> Result<String, String> {
    let t = s.trim();
    if (t.starts_with('"') && t.ends_with('"')) || (t.starts_with('\'') && t.ends_with('\'')) {
        // Variante Ok.
        Ok(t[1..t.len() - 1].to_string())
    } else {
        // Variante Err.
        Err(format!("expected quoted string, got: {}", s))
    }
}

// ---------------------------------------------------------------------------
// Filter Cache
// ---------------------------------------------------------------------------

/// Thread-safe cache of compiled filters keyed by expression source.
pub struct FilterCache {
    cache: RwLock<HashMap<String, CompiledFilter>>,
}

impl FilterCache {
    /// Função new.
    pub fn new() -> Self {
        // Variante Self.
        Self {
            cache: RwLock::new(HashMap::new()),
        }
    }

    /// Get or compile-and-cache a filter expression.
    pub fn get_or_compile(&self, expr: &str) -> Result<CompiledFilter, String> {
        {
            let map = self.cache.read().unwrap();
            if let Some(f) = map.get(expr) {
                return Ok(f.clone());
            }
        }
        let compiled = parse_expression(expr)?;
        let mut map = self.cache.write().unwrap();
        map.insert(expr.to_string(), compiled.clone());
        debug!(expr, "filter compiled and cached");
        // Variante Ok.
        Ok(compiled)
    }

    /// Função len.
    pub fn len(&self) -> usize {
        self.cache.read().unwrap().len()
    }
}

impl Default for FilterCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_types::{AgentStateChanged, ZylosEvent};

    fn make_envelope(topic: &str, payload: ZylosEvent) -> EventEnvelope {
        EventEnvelope::new(topic, payload, 1)
    }

    #[test]
    fn test_topic_wildcard_single() {
        assert!(topic_matches("agents.*.state", "agents.a1.state"));
        assert!(topic_matches("agents.*.state", "agents.xyz.state"));
        assert!(!topic_matches("agents.*.state", "agents.a1.b2.state"));
        assert!(!topic_matches("agents.*.state", "tasks.t1.state"));
    }

    #[test]
    fn test_topic_wildcard_multi() {
        assert!(topic_matches("agents.**", "agents.a1.state"));
        assert!(topic_matches("agents.**", "agents.a1.b2.c3"));
        assert!(!topic_matches("agents.**", "tasks.t1.progress"));
    }

    #[test]
    fn test_parse_all() {
        let f = parse_expression("*").unwrap();
        let env = make_envelope(
            "x",
            ZylosEvent::AgentStateChanged(AgentStateChanged {
                agent_id: "a".into(),
                previous_state: "i".into(),
                new_state: "r".into(),
                reason: None,
            }),
        );
        assert!(f.matches(&env));
    }

    #[test]
    fn test_parse_topic_pattern() {
        let f = parse_expression(r#"topic == "agents.*.state""#).unwrap();
        let e1 = make_envelope(
            "agents.a1.state",
            ZylosEvent::AgentStateChanged(AgentStateChanged {
                agent_id: "a1".into(),
                previous_state: "i".into(),
                new_state: "r".into(),
                reason: None,
            }),
        );
        let e2 = make_envelope(
            "tasks.t1.progress",
            ZylosEvent::AgentStateChanged(AgentStateChanged {
                agent_id: "a1".into(),
                previous_state: "i".into(),
                new_state: "r".into(),
                reason: None,
            }),
        );
        assert!(f.matches(&e1));
        assert!(!f.matches(&e2));
    }

    #[test]
    fn test_parse_event_type() {
        let f = parse_expression(r#"event_type == "TaskCompleted""#).unwrap();
        let e1 = make_envelope(
            "tasks.t1.done",
            ZylosEvent::TaskCompleted(crate::event_types::TaskCompleted {
                task_id: "t1".into(),
                agent_id: "a1".into(),
                outcome: "success".into(),
                duration_ms: 100,
            }),
        );
        let e2 = make_envelope(
            "agents.a1.state",
            ZylosEvent::AgentStateChanged(AgentStateChanged {
                agent_id: "a1".into(),
                previous_state: "i".into(),
                new_state: "r".into(),
                reason: None,
            }),
        );
        assert!(f.matches(&e1));
        assert!(!f.matches(&e2));
    }

    #[test]
    fn test_parse_and() {
        let f =
            parse_expression(r#"topic == "agents.*.state" && event_type == "AgentStateChanged""#)
                .unwrap();
        let e = make_envelope(
            "agents.a1.state",
            ZylosEvent::AgentStateChanged(AgentStateChanged {
                agent_id: "a1".into(),
                previous_state: "i".into(),
                new_state: "r".into(),
                reason: None,
            }),
        );
        assert!(f.matches(&e));
    }

    #[test]
    fn test_filter_cache() {
        let cache = FilterCache::new();
        let f1 = cache.get_or_compile(r#"topic == "agents.**""#).unwrap();
        let f2 = cache.get_or_compile(r#"topic == "agents.**""#).unwrap();
        assert_eq!(f1.source(), f2.source());
        assert_eq!(cache.len(), 1);
    }
}
