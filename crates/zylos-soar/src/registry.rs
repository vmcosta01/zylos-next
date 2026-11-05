//! Runbook registry: maps runbook names to concrete `SoarRunnable` implementations.

use crate::runnable::SoarRunnable;
use std::collections::HashMap;

/// Registry of available runbooks, keyed by name.
pub struct RunbookRegistry {
    runbooks: HashMap<String, Box<dyn SoarRunnable>>,
}

impl RunbookRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            runbooks: HashMap::new(),
        }
    }

    /// Register a runbook implementation. Overwrites if a runbook with the same name exists.
    pub fn register(&mut self, runbook: Box<dyn SoarRunnable>) {
        let name = runbook.name().to_string();
        self.runbooks.insert(name, runbook);
    }

    /// Look up a runbook by name.
    pub fn get(&self, name: &str) -> Option<&dyn SoarRunnable> {
        self.runbooks.get(name).map(|r| r.as_ref())
    }

    /// Check if a runbook is registered.
    pub fn contains(&self, name: &str) -> bool {
        self.runbooks.contains_key(name)
    }

    /// List all registered runbook names.
    pub fn list(&self) -> Vec<&str> {
        self.runbooks.keys().map(|k| k.as_str()).collect()
    }

    /// Return the number of registered runbooks.
    pub fn len(&self) -> usize {
        self.runbooks.len()
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.runbooks.is_empty()
    }
}

impl Default for RunbookRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Result;
    use crate::runnable::{BoxFuture, IncidentContext, RunbookResult};

    struct MockRunbook {
        name: String,
    }

    impl SoarRunnable for MockRunbook {
        fn name(&self) -> &str {
            &self.name
        }

        fn validate_preconditions<'a>(
            &'a self,
            _context: &'a IncidentContext,
        ) -> BoxFuture<'a, Result<()>> {
            Box::pin(async { Ok(()) })
        }

        fn execute<'a>(
            &'a self,
            _context: IncidentContext,
        ) -> BoxFuture<'a, Result<RunbookResult>> {
            Box::pin(async { Ok(RunbookResult::ok("mock executed", 10)) })
        }
    }

    #[test]
    fn register_and_lookup() {
        let mut registry = RunbookRegistry::new();
        registry.register(Box::new(MockRunbook {
            name: "isolate-host".to_string(),
        }));

        assert!(registry.contains("isolate-host"));
        assert!(!registry.contains("nonexistent"));
        assert_eq!(registry.len(), 1);

        let runbook = registry.get("isolate-host").unwrap();
        assert_eq!(runbook.name(), "isolate-host");
    }

    #[test]
    fn list_returns_all_names() {
        let mut registry = RunbookRegistry::new();
        registry.register(Box::new(MockRunbook {
            name: "runbook-a".to_string(),
        }));
        registry.register(Box::new(MockRunbook {
            name: "runbook-b".to_string(),
        }));

        let mut names = registry.list();
        names.sort();
        assert_eq!(names, vec!["runbook-a", "runbook-b"]);
    }

    #[test]
    fn overwrite_on_duplicate_name() {
        let mut registry = RunbookRegistry::new();
        registry.register(Box::new(MockRunbook {
            name: "test".to_string(),
        }));
        registry.register(Box::new(MockRunbook {
            name: "test".to_string(),
        }));

        assert_eq!(registry.len(), 1);
    }
}