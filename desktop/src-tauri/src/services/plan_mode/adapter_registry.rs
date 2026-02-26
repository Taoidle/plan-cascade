//! Adapter Registry
//!
//! Stores and retrieves domain adapters for Plan Mode.
//! Adapters are registered at startup and looked up by ID or domain.

use std::collections::HashMap;
use std::sync::Arc;

use super::adapter::DomainAdapter;
use super::types::TaskDomain;

/// Registry that stores domain adapters and provides lookup by ID or domain.
pub struct AdapterRegistry {
    adapters: HashMap<String, Arc<dyn DomainAdapter>>,
}

impl AdapterRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            adapters: HashMap::new(),
        }
    }

    /// Create a registry with all built-in adapters pre-registered.
    pub fn with_builtins() -> Self {
        let mut registry = Self::new();
        registry.register(Arc::new(super::adapters::general::GeneralAdapter));
        registry.register(Arc::new(super::adapters::writing::WritingAdapter));
        registry.register(Arc::new(super::adapters::research::ResearchAdapter));
        registry
    }

    /// Register a new adapter.
    pub fn register(&mut self, adapter: Arc<dyn DomainAdapter>) {
        self.adapters.insert(adapter.id().to_string(), adapter);
    }

    /// Get an adapter by its ID.
    pub fn get(&self, id: &str) -> Option<Arc<dyn DomainAdapter>> {
        self.adapters.get(id).cloned()
    }

    /// Find the best adapter for a given domain.
    /// Falls back to "general" if no domain-specific adapter is found.
    pub fn find_for_domain(&self, domain: &TaskDomain) -> Arc<dyn DomainAdapter> {
        // First, look for an adapter that explicitly supports this domain
        for adapter in self.adapters.values() {
            if adapter.supported_domains().contains(domain) {
                return adapter.clone();
            }
        }

        // Fall back to general adapter
        self.adapters
            .get("general")
            .cloned()
            .expect("GeneralAdapter must be registered")
    }

    /// List all registered adapters with their display names.
    pub fn list(&self) -> Vec<AdapterInfo> {
        let mut infos: Vec<_> = self
            .adapters
            .values()
            .map(|a| AdapterInfo {
                id: a.id().to_string(),
                display_name: a.display_name().to_string(),
                supported_domains: a
                    .supported_domains()
                    .iter()
                    .map(|d| d.to_string())
                    .collect(),
            })
            .collect();
        infos.sort_by(|a, b| a.id.cmp(&b.id));
        infos
    }
}

impl Default for AdapterRegistry {
    fn default() -> Self {
        Self::with_builtins()
    }
}

/// Summary info about a registered adapter.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterInfo {
    pub id: String,
    pub display_name: String,
    pub supported_domains: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_with_builtins() {
        let registry = AdapterRegistry::with_builtins();
        assert!(registry.get("general").is_some());
        assert!(registry.get("writing").is_some());
        assert!(registry.get("research").is_some());
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_find_for_domain() {
        let registry = AdapterRegistry::with_builtins();

        let general = registry.find_for_domain(&TaskDomain::General);
        assert_eq!(general.id(), "general");

        let writing = registry.find_for_domain(&TaskDomain::Writing);
        assert_eq!(writing.id(), "writing");

        let research = registry.find_for_domain(&TaskDomain::Research);
        assert_eq!(research.id(), "research");

        // Custom domain falls back to general
        let custom = registry.find_for_domain(&TaskDomain::Custom("unknown".to_string()));
        assert_eq!(custom.id(), "general");
    }

    #[test]
    fn test_list_adapters() {
        let registry = AdapterRegistry::with_builtins();
        let list = registry.list();
        assert_eq!(list.len(), 3);
        // Sorted alphabetically
        assert_eq!(list[0].id, "general");
        assert_eq!(list[1].id, "research");
        assert_eq!(list[2].id, "writing");
    }
}
