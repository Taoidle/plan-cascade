//! Design Document Loader
//!
//! Service for loading, caching, and querying design documents.
//! Supports project-level and feature-level design documents with
//! path conventions for automatic discovery.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::models::design_doc::{
    Component, Decision, DesignDoc, DesignDocError, DesignDocLevel, FeatureMapping, Pattern,
};

/// Default name for design document files
pub const DESIGN_DOC_FILENAME: &str = "design_doc.json";

/// Default worktrees directory name
pub const WORKTREES_DIR: &str = ".worktrees";

/// Service for loading and querying design documents
///
/// Manages a two-level hierarchy of design documents:
/// - Project level: `{project_root}/design_doc.json`
/// - Feature level: `{project_root}/.worktrees/{feature}/design_doc.json`
///
/// Documents are cached after loading for efficient repeated access.
pub struct DesignDocLoader {
    /// Cached project-level design document
    project_doc: Arc<RwLock<Option<DesignDoc>>>,
    /// Cached feature-level design documents (feature_id -> doc)
    feature_docs: Arc<RwLock<HashMap<String, DesignDoc>>>,
    /// Root path of the project (for reloading)
    project_root: Arc<RwLock<Option<PathBuf>>>,
}

impl DesignDocLoader {
    /// Create a new DesignDocLoader
    pub fn new() -> Self {
        Self {
            project_doc: Arc::new(RwLock::new(None)),
            feature_docs: Arc::new(RwLock::new(HashMap::new())),
            project_root: Arc::new(RwLock::new(None)),
        }
    }

    // ========================================================================
    // Story S004: Loading Methods
    // ========================================================================

    /// Get the path to the project-level design document
    pub fn project_doc_path(project_root: &Path) -> PathBuf {
        project_root.join(DESIGN_DOC_FILENAME)
    }

    /// Get the path to a feature-level design document
    pub fn feature_doc_path(project_root: &Path, feature_id: &str) -> PathBuf {
        project_root
            .join(WORKTREES_DIR)
            .join(feature_id)
            .join(DESIGN_DOC_FILENAME)
    }

    /// Load the project-level design document
    ///
    /// Loads from `{project_root}/design_doc.json` and caches the result.
    /// Subsequent calls return the cached version unless `reload_all` is called.
    pub async fn load_project_doc(
        &self,
        project_root: &Path,
    ) -> Result<(), DesignDocError> {
        let path = Self::project_doc_path(project_root);
        let doc = DesignDoc::from_file(&path)?;

        // Verify it's a project-level document
        if doc.level() != DesignDocLevel::Project {
            return Err(DesignDocError::InvalidLevel(format!(
                "Expected project-level document, got {:?}",
                doc.level()
            )));
        }

        // Cache the document and project root
        {
            let mut cached = self.project_doc.write().await;
            *cached = Some(doc);
        }
        {
            let mut root = self.project_root.write().await;
            *root = Some(project_root.to_path_buf());
        }

        Ok(())
    }

    /// Load a feature-level design document
    ///
    /// Loads from `{worktree_path}/design_doc.json` and caches by feature ID.
    /// The feature ID is extracted from the worktree path.
    pub async fn load_feature_doc(
        &self,
        worktree_path: &Path,
    ) -> Result<(), DesignDocError> {
        let path = worktree_path.join(DESIGN_DOC_FILENAME);
        let doc = DesignDoc::from_file(&path)?;

        // Extract feature ID from path (last component before design_doc.json)
        let feature_id = worktree_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| {
                DesignDocError::ValidationError("Cannot extract feature ID from path".to_string())
            })?;

        // Cache the document
        {
            let mut cached = self.feature_docs.write().await;
            cached.insert(feature_id.to_string(), doc);
        }

        Ok(())
    }

    /// Load a feature-level design document by ID
    ///
    /// Requires project root to be set via `load_project_doc` first.
    pub async fn load_feature_doc_by_id(
        &self,
        feature_id: &str,
    ) -> Result<(), DesignDocError> {
        let root = {
            let root = self.project_root.read().await;
            root.clone().ok_or_else(|| {
                DesignDocError::ValidationError("Project root not set. Call load_project_doc first.".to_string())
            })?
        };

        let path = Self::feature_doc_path(&root, feature_id);
        let doc = DesignDoc::from_file(&path)?;

        // Cache the document
        {
            let mut cached = self.feature_docs.write().await;
            cached.insert(feature_id.to_string(), doc);
        }

        Ok(())
    }

    /// Get the cached project-level design document
    pub async fn get_project_doc(&self) -> Option<DesignDoc> {
        let cached = self.project_doc.read().await;
        cached.clone()
    }

    /// Get a cached feature-level design document by ID
    pub async fn get_feature_doc(&self, feature_id: &str) -> Option<DesignDoc> {
        let cached = self.feature_docs.read().await;
        cached.get(feature_id).cloned()
    }

    /// Reload all cached design documents
    ///
    /// Re-reads the project document and all cached feature documents from disk.
    pub async fn reload_all(&self) -> Result<(), DesignDocError> {
        // Get project root
        let root = {
            let root = self.project_root.read().await;
            match root.as_ref() {
                Some(r) => r.clone(),
                None => return Ok(()), // Nothing to reload
            }
        };

        // Reload project doc
        let project_path = Self::project_doc_path(&root);
        if project_path.exists() {
            let doc = DesignDoc::from_file(&project_path)?;
            let mut cached = self.project_doc.write().await;
            *cached = Some(doc);
        }

        // Reload all feature docs
        let feature_ids: Vec<String> = {
            let cached = self.feature_docs.read().await;
            cached.keys().cloned().collect()
        };

        for feature_id in feature_ids {
            let feature_path = Self::feature_doc_path(&root, &feature_id);
            if feature_path.exists() {
                let doc = DesignDoc::from_file(&feature_path)?;
                let mut cached = self.feature_docs.write().await;
                cached.insert(feature_id, doc);
            }
        }

        Ok(())
    }

    /// Check if project document is loaded
    pub async fn has_project_doc(&self) -> bool {
        let cached = self.project_doc.read().await;
        cached.is_some()
    }

    /// Check if a feature document is loaded
    pub async fn has_feature_doc(&self, feature_id: &str) -> bool {
        let cached = self.feature_docs.read().await;
        cached.contains_key(feature_id)
    }

    /// Get all loaded feature IDs
    pub async fn loaded_feature_ids(&self) -> Vec<String> {
        let cached = self.feature_docs.read().await;
        cached.keys().cloned().collect()
    }

    // ========================================================================
    // Story S005: Query Methods
    // ========================================================================

    /// Get a component by name from the project document
    pub async fn get_component(&self, name: &str) -> Option<Component> {
        let cached = self.project_doc.read().await;
        cached
            .as_ref()
            .and_then(|doc| doc.get_component(name).cloned())
    }

    /// Get all components associated with a feature
    ///
    /// Searches both the project document's components (by features field)
    /// and the feature's mapping (by components field).
    pub async fn get_components_for_feature(&self, feature_id: &str) -> Vec<Component> {
        let mut result = Vec::new();

        // Check project doc
        if let Some(doc) = self.get_project_doc().await {
            // Get components that list this feature
            for comp in &doc.architecture.components {
                if comp.features.contains(&feature_id.to_string()) {
                    result.push(comp.clone());
                }
            }

            // Get components listed in the feature mapping
            if let Some(mapping) = doc.get_feature_mapping(feature_id) {
                for comp_name in &mapping.components {
                    if let Some(comp) = doc.get_component(comp_name) {
                        if !result.iter().any(|c| c.name == comp.name) {
                            result.push(comp.clone());
                        }
                    }
                }
            }
        }

        // Check feature doc for additional components
        if let Some(doc) = self.get_feature_doc(feature_id).await {
            for comp in &doc.architecture.components {
                if !result.iter().any(|c| c.name == comp.name) {
                    result.push(comp.clone());
                }
            }
        }

        result
    }

    /// List all components from the project document
    pub async fn list_all_components(&self) -> Vec<Component> {
        let cached = self.project_doc.read().await;
        cached
            .as_ref()
            .map(|doc| doc.architecture.components.clone())
            .unwrap_or_default()
    }

    /// Get a pattern by name from the project document
    pub async fn get_pattern(&self, name: &str) -> Option<Pattern> {
        let cached = self.project_doc.read().await;
        cached
            .as_ref()
            .and_then(|doc| doc.get_pattern(name).cloned())
    }

    /// Get all patterns associated with a feature
    pub async fn get_patterns_for_feature(&self, feature_id: &str) -> Vec<Pattern> {
        let mut result = Vec::new();

        // Check project doc
        if let Some(doc) = self.get_project_doc().await {
            // Get patterns that apply to this feature
            for pattern in &doc.architecture.patterns {
                if pattern.applies_to.contains(&feature_id.to_string()) {
                    result.push(pattern.clone());
                }
            }

            // Get patterns listed in the feature mapping
            if let Some(mapping) = doc.get_feature_mapping(feature_id) {
                for pattern_name in &mapping.patterns {
                    if let Some(pattern) = doc.get_pattern(pattern_name) {
                        if !result.iter().any(|p| p.name == pattern.name) {
                            result.push(pattern.clone());
                        }
                    }
                }
            }
        }

        // Check feature doc for additional patterns
        if let Some(doc) = self.get_feature_doc(feature_id).await {
            for pattern in &doc.architecture.patterns {
                if !result.iter().any(|p| p.name == pattern.name) {
                    result.push(pattern.clone());
                }
            }
        }

        result
    }

    /// List all patterns from the project document
    pub async fn list_all_patterns(&self) -> Vec<Pattern> {
        let cached = self.project_doc.read().await;
        cached
            .as_ref()
            .map(|doc| doc.architecture.patterns.clone())
            .unwrap_or_default()
    }

    /// Get a decision by ID from the project document
    pub async fn get_decision(&self, id: &str) -> Option<Decision> {
        let cached = self.project_doc.read().await;
        cached
            .as_ref()
            .and_then(|doc| doc.get_decision(id).cloned())
    }

    /// Get all decisions associated with a feature
    pub async fn get_decisions_for_feature(&self, feature_id: &str) -> Vec<Decision> {
        let mut result = Vec::new();

        // Check project doc
        if let Some(doc) = self.get_project_doc().await {
            // Get decisions that apply to this feature
            for decision in &doc.decisions {
                if decision.applies_to.contains(&feature_id.to_string()) {
                    result.push(decision.clone());
                }
            }

            // Get decisions listed in the feature mapping
            if let Some(mapping) = doc.get_feature_mapping(feature_id) {
                for decision_id in &mapping.decisions {
                    if let Some(decision) = doc.get_decision(decision_id) {
                        if !result.iter().any(|d| d.id == decision.id) {
                            result.push(decision.clone());
                        }
                    }
                }
            }
        }

        // Check feature doc for additional decisions
        if let Some(doc) = self.get_feature_doc(feature_id).await {
            for decision in &doc.decisions {
                if !result.iter().any(|d| d.id == decision.id) {
                    result.push(decision.clone());
                }
            }
        }

        result
    }

    /// List all decisions from the project document
    pub async fn list_all_decisions(&self) -> Vec<Decision> {
        let cached = self.project_doc.read().await;
        cached
            .as_ref()
            .map(|doc| doc.decisions.clone())
            .unwrap_or_default()
    }

    /// Get a feature mapping by feature ID
    pub async fn get_feature_mapping(&self, feature_id: &str) -> Option<FeatureMapping> {
        // First check project doc
        if let Some(doc) = self.get_project_doc().await {
            if let Some(mapping) = doc.get_feature_mapping(feature_id) {
                return Some(mapping.clone());
            }
        }

        // Then check feature doc
        if let Some(doc) = self.get_feature_doc(feature_id).await {
            // Feature docs may have mappings for their own sub-features
            if let Some(mapping) = doc.get_feature_mapping(feature_id) {
                return Some(mapping.clone());
            }
        }

        None
    }

    /// Get all feature IDs that use a specific component
    pub async fn get_features_for_component(&self, component_name: &str) -> Vec<String> {
        let mut result = Vec::new();

        if let Some(doc) = self.get_project_doc().await {
            // Check component's features field
            if let Some(comp) = doc.get_component(component_name) {
                result.extend(comp.features.clone());
            }

            // Check feature mappings
            for (feature_id, mapping) in &doc.feature_mappings {
                if mapping.components.contains(&component_name.to_string()) {
                    if !result.contains(feature_id) {
                        result.push(feature_id.clone());
                    }
                }
            }
        }

        result
    }

    /// Get all feature IDs that use a specific pattern
    pub async fn get_features_for_pattern(&self, pattern_name: &str) -> Vec<String> {
        let mut result = Vec::new();

        if let Some(doc) = self.get_project_doc().await {
            // Check pattern's applies_to field
            if let Some(pattern) = doc.get_pattern(pattern_name) {
                result.extend(pattern.applies_to.clone());
            }

            // Check feature mappings
            for (feature_id, mapping) in &doc.feature_mappings {
                if mapping.patterns.contains(&pattern_name.to_string()) {
                    if !result.contains(feature_id) {
                        result.push(feature_id.clone());
                    }
                }
            }
        }

        result
    }

    /// Get all feature IDs that reference a specific decision
    pub async fn get_features_for_decision(&self, decision_id: &str) -> Vec<String> {
        let mut result = Vec::new();

        if let Some(doc) = self.get_project_doc().await {
            // Check decision's applies_to field
            if let Some(decision) = doc.get_decision(decision_id) {
                result.extend(decision.applies_to.clone());
            }

            // Check feature mappings
            for (feature_id, mapping) in &doc.feature_mappings {
                if mapping.decisions.contains(&decision_id.to_string()) {
                    if !result.contains(feature_id) {
                        result.push(feature_id.clone());
                    }
                }
            }
        }

        result
    }
}

impl Default for DesignDocLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for DesignDocLoader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DesignDocLoader").finish()
    }
}

// ============================================================================
// Story S006: Service Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Create a test project structure with design documents
    fn create_test_project() -> TempDir {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        // Create project-level design doc
        let project_doc = r#"{
            "metadata": {
                "version": "1.0.0",
                "level": "project"
            },
            "overview": {
                "title": "Test Project",
                "summary": "A test project for design doc loader"
            },
            "architecture": {
                "components": [
                    {
                        "name": "AuthComponent",
                        "description": "Handles authentication",
                        "features": ["feature-auth"]
                    },
                    {
                        "name": "DatabaseComponent",
                        "description": "Database access layer",
                        "features": ["feature-auth", "feature-data"]
                    }
                ],
                "patterns": [
                    {
                        "name": "Repository",
                        "description": "Repository pattern",
                        "applies_to": ["feature-data"]
                    },
                    {
                        "name": "JWT",
                        "description": "JWT authentication",
                        "applies_to": ["feature-auth"]
                    }
                ]
            },
            "decisions": [
                {
                    "id": "ADR-001",
                    "title": "Use Rust",
                    "status": "accepted",
                    "applies_to": ["feature-auth", "feature-data"]
                },
                {
                    "id": "ADR-002",
                    "title": "Use SQLite",
                    "status": "accepted",
                    "applies_to": ["feature-data"]
                }
            ],
            "feature_mappings": {
                "feature-auth": {
                    "components": ["AuthComponent", "DatabaseComponent"],
                    "patterns": ["JWT"],
                    "decisions": ["ADR-001"],
                    "description": "Authentication feature"
                },
                "feature-data": {
                    "components": ["DatabaseComponent"],
                    "patterns": ["Repository"],
                    "decisions": ["ADR-001", "ADR-002"],
                    "description": "Data access feature"
                }
            }
        }"#;

        fs::write(temp_dir.path().join(DESIGN_DOC_FILENAME), project_doc)
            .expect("Failed to write project doc");

        // Create worktrees directory and feature doc
        let worktree_path = temp_dir.path().join(WORKTREES_DIR).join("feature-auth");
        fs::create_dir_all(&worktree_path).expect("Failed to create worktree dir");

        let feature_doc = r#"{
            "metadata": {
                "version": "1.0.0",
                "level": "feature",
                "mega_plan_reference": "mega-plan-001"
            },
            "overview": {
                "title": "Auth Feature",
                "summary": "Authentication implementation"
            },
            "architecture": {
                "components": [
                    {
                        "name": "TokenService",
                        "description": "JWT token management"
                    }
                ],
                "patterns": [
                    {
                        "name": "Factory",
                        "description": "Factory pattern for token creation"
                    }
                ]
            },
            "decisions": [
                {
                    "id": "ADR-F001",
                    "title": "Use HS256 for JWT",
                    "status": "accepted"
                }
            ]
        }"#;

        fs::write(worktree_path.join(DESIGN_DOC_FILENAME), feature_doc)
            .expect("Failed to write feature doc");

        temp_dir
    }

    /// Test S006-5: test_loader_project_doc
    #[tokio::test]
    async fn test_loader_project_doc() {
        let temp_dir = create_test_project();
        let loader = DesignDocLoader::new();

        // Load project doc
        loader
            .load_project_doc(temp_dir.path())
            .await
            .expect("Failed to load project doc");

        // Verify it's loaded
        assert!(loader.has_project_doc().await);

        let doc = loader.get_project_doc().await.expect("Should have doc");
        assert_eq!(doc.overview.title, "Test Project");
        assert_eq!(doc.level(), DesignDocLevel::Project);
        assert_eq!(doc.architecture.components.len(), 2);
        assert_eq!(doc.architecture.patterns.len(), 2);
        assert_eq!(doc.decisions.len(), 2);
    }

    /// Test S006-6: test_loader_feature_doc
    #[tokio::test]
    async fn test_loader_feature_doc() {
        let temp_dir = create_test_project();
        let loader = DesignDocLoader::new();

        // Load project doc first (sets project root)
        loader
            .load_project_doc(temp_dir.path())
            .await
            .expect("Failed to load project doc");

        // Load feature doc by path
        let worktree_path = temp_dir.path().join(WORKTREES_DIR).join("feature-auth");
        loader
            .load_feature_doc(&worktree_path)
            .await
            .expect("Failed to load feature doc");

        // Verify it's loaded
        assert!(loader.has_feature_doc("feature-auth").await);

        let doc = loader
            .get_feature_doc("feature-auth")
            .await
            .expect("Should have doc");
        assert_eq!(doc.overview.title, "Auth Feature");
        assert_eq!(doc.architecture.components.len(), 1);
        assert_eq!(doc.architecture.components[0].name, "TokenService");
    }

    /// Test S006-7: test_loader_caching
    #[tokio::test]
    async fn test_loader_caching() {
        let temp_dir = create_test_project();
        let loader = DesignDocLoader::new();

        // Load docs
        loader
            .load_project_doc(temp_dir.path())
            .await
            .expect("Failed to load");

        let worktree_path = temp_dir.path().join(WORKTREES_DIR).join("feature-auth");
        loader
            .load_feature_doc(&worktree_path)
            .await
            .expect("Failed to load");

        // Verify caching - multiple gets should work
        let doc1 = loader.get_project_doc().await;
        let doc2 = loader.get_project_doc().await;
        assert!(doc1.is_some());
        assert!(doc2.is_some());

        // Check loaded feature IDs
        let feature_ids = loader.loaded_feature_ids().await;
        assert!(feature_ids.contains(&"feature-auth".to_string()));

        // Test reload
        loader.reload_all().await.expect("Failed to reload");

        // Should still have data after reload
        assert!(loader.has_project_doc().await);
        assert!(loader.has_feature_doc("feature-auth").await);
    }

    /// Test S006-8: test_query_component
    #[tokio::test]
    async fn test_query_component() {
        let temp_dir = create_test_project();
        let loader = DesignDocLoader::new();

        loader
            .load_project_doc(temp_dir.path())
            .await
            .expect("Failed to load");

        // Test get_component
        let comp = loader
            .get_component("AuthComponent")
            .await
            .expect("Should find component");
        assert_eq!(comp.name, "AuthComponent");
        assert_eq!(comp.description, "Handles authentication");

        // Test non-existent component
        assert!(loader.get_component("NonExistent").await.is_none());

        // Test list_all_components
        let all = loader.list_all_components().await;
        assert_eq!(all.len(), 2);

        // Test get_components_for_feature
        let auth_comps = loader.get_components_for_feature("feature-auth").await;
        assert_eq!(auth_comps.len(), 2); // AuthComponent + DatabaseComponent

        let data_comps = loader.get_components_for_feature("feature-data").await;
        assert_eq!(data_comps.len(), 1); // Only DatabaseComponent

        // Test get_features_for_component
        let auth_features = loader.get_features_for_component("AuthComponent").await;
        assert!(auth_features.contains(&"feature-auth".to_string()));

        let db_features = loader.get_features_for_component("DatabaseComponent").await;
        assert!(db_features.contains(&"feature-auth".to_string()));
        assert!(db_features.contains(&"feature-data".to_string()));
    }

    /// Test S006-9: test_query_pattern
    #[tokio::test]
    async fn test_query_pattern() {
        let temp_dir = create_test_project();
        let loader = DesignDocLoader::new();

        loader
            .load_project_doc(temp_dir.path())
            .await
            .expect("Failed to load");

        // Test get_pattern
        let pattern = loader
            .get_pattern("Repository")
            .await
            .expect("Should find pattern");
        assert_eq!(pattern.name, "Repository");

        // Test non-existent pattern
        assert!(loader.get_pattern("NonExistent").await.is_none());

        // Test list_all_patterns
        let all = loader.list_all_patterns().await;
        assert_eq!(all.len(), 2);

        // Test get_patterns_for_feature
        let data_patterns = loader.get_patterns_for_feature("feature-data").await;
        assert_eq!(data_patterns.len(), 1);
        assert_eq!(data_patterns[0].name, "Repository");

        let auth_patterns = loader.get_patterns_for_feature("feature-auth").await;
        assert_eq!(auth_patterns.len(), 1);
        assert_eq!(auth_patterns[0].name, "JWT");

        // Test get_features_for_pattern
        let repo_features = loader.get_features_for_pattern("Repository").await;
        assert!(repo_features.contains(&"feature-data".to_string()));
    }

    /// Test S006-10: test_query_decision
    #[tokio::test]
    async fn test_query_decision() {
        let temp_dir = create_test_project();
        let loader = DesignDocLoader::new();

        loader
            .load_project_doc(temp_dir.path())
            .await
            .expect("Failed to load");

        // Test get_decision
        let decision = loader
            .get_decision("ADR-001")
            .await
            .expect("Should find decision");
        assert_eq!(decision.id, "ADR-001");
        assert_eq!(decision.title, "Use Rust");

        // Test non-existent decision
        assert!(loader.get_decision("ADR-999").await.is_none());

        // Test list_all_decisions
        let all = loader.list_all_decisions().await;
        assert_eq!(all.len(), 2);

        // Test get_decisions_for_feature
        let auth_decisions = loader.get_decisions_for_feature("feature-auth").await;
        assert_eq!(auth_decisions.len(), 1);
        assert_eq!(auth_decisions[0].id, "ADR-001");

        let data_decisions = loader.get_decisions_for_feature("feature-data").await;
        assert_eq!(data_decisions.len(), 2); // ADR-001 and ADR-002

        // Test get_features_for_decision
        let adr001_features = loader.get_features_for_decision("ADR-001").await;
        assert!(adr001_features.contains(&"feature-auth".to_string()));
        assert!(adr001_features.contains(&"feature-data".to_string()));
    }

    /// Test S006-11: test_query_feature_mapping
    #[tokio::test]
    async fn test_query_feature_mapping() {
        let temp_dir = create_test_project();
        let loader = DesignDocLoader::new();

        loader
            .load_project_doc(temp_dir.path())
            .await
            .expect("Failed to load");

        // Test get_feature_mapping
        let mapping = loader
            .get_feature_mapping("feature-auth")
            .await
            .expect("Should find mapping");

        assert_eq!(mapping.description, "Authentication feature");
        assert!(mapping.components.contains(&"AuthComponent".to_string()));
        assert!(mapping.components.contains(&"DatabaseComponent".to_string()));
        assert!(mapping.patterns.contains(&"JWT".to_string()));
        assert!(mapping.decisions.contains(&"ADR-001".to_string()));

        // Test non-existent mapping
        assert!(loader.get_feature_mapping("non-existent").await.is_none());
    }

    /// Test S006-12: test_not_found_error
    #[tokio::test]
    async fn test_not_found_error() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let loader = DesignDocLoader::new();

        // Try to load non-existent project doc
        let result = loader.load_project_doc(temp_dir.path()).await;
        assert!(matches!(result, Err(DesignDocError::NotFound(_))));

        // Try to load non-existent feature doc
        let fake_path = temp_dir.path().join("non-existent-feature");
        fs::create_dir_all(&fake_path).expect("Failed to create dir");
        let result = loader.load_feature_doc(&fake_path).await;
        assert!(matches!(result, Err(DesignDocError::NotFound(_))));
    }

    #[tokio::test]
    async fn test_combined_project_and_feature_queries() {
        let temp_dir = create_test_project();
        let loader = DesignDocLoader::new();

        // Load both project and feature docs
        loader
            .load_project_doc(temp_dir.path())
            .await
            .expect("Failed to load project");

        let worktree_path = temp_dir.path().join(WORKTREES_DIR).join("feature-auth");
        loader
            .load_feature_doc(&worktree_path)
            .await
            .expect("Failed to load feature");

        // Query components for feature-auth should include both project and feature components
        let comps = loader.get_components_for_feature("feature-auth").await;

        // Should have: AuthComponent, DatabaseComponent (from project) + TokenService (from feature)
        let names: Vec<_> = comps.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"AuthComponent"));
        assert!(names.contains(&"DatabaseComponent"));
        assert!(names.contains(&"TokenService"));

        // Query patterns for feature-auth should include both
        let patterns = loader.get_patterns_for_feature("feature-auth").await;
        let pattern_names: Vec<_> = patterns.iter().map(|p| p.name.as_str()).collect();
        assert!(pattern_names.contains(&"JWT"));
        assert!(pattern_names.contains(&"Factory"));

        // Query decisions for feature-auth should include both
        let decisions = loader.get_decisions_for_feature("feature-auth").await;
        let decision_ids: Vec<_> = decisions.iter().map(|d| d.id.as_str()).collect();
        assert!(decision_ids.contains(&"ADR-001"));
        assert!(decision_ids.contains(&"ADR-F001"));
    }

    #[tokio::test]
    async fn test_load_feature_doc_by_id() {
        let temp_dir = create_test_project();
        let loader = DesignDocLoader::new();

        // Load project doc first
        loader
            .load_project_doc(temp_dir.path())
            .await
            .expect("Failed to load project");

        // Load feature by ID
        loader
            .load_feature_doc_by_id("feature-auth")
            .await
            .expect("Failed to load feature by ID");

        assert!(loader.has_feature_doc("feature-auth").await);
    }

    #[tokio::test]
    async fn test_path_conventions() {
        let project_root = Path::new("/project");

        let project_path = DesignDocLoader::project_doc_path(project_root);
        assert_eq!(
            project_path,
            PathBuf::from("/project/design_doc.json")
        );

        let feature_path = DesignDocLoader::feature_doc_path(project_root, "my-feature");
        assert_eq!(
            feature_path,
            PathBuf::from("/project/.worktrees/my-feature/design_doc.json")
        );
    }
}
