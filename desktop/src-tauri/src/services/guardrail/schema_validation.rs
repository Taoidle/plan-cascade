//! Schema Validation Guardrail
//!
//! Validates agent JSON output against registered JSON Schemas.
//! Implements the `Guardrail` trait to integrate with the guardrail
//! lifecycle hooks.
//!
//! ## Usage
//!
//! ```rust,ignore
//! let mut guardrail = SchemaValidationGuardrail::new();
//! guardrail.register_schema("prd", prd_schema_json)?;
//! guardrail.set_active_schema("prd");
//!
//! let result = guardrail.validate(json_output, Direction::Output).await;
//! ```

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::RwLock;

use super::{Direction, Guardrail, GuardrailResult};

// ---------------------------------------------------------------------------
// SchemaRegistry
// ---------------------------------------------------------------------------

/// Registry of JSON Schemas keyed by task type.
pub struct SchemaRegistry {
    schemas: HashMap<String, Value>,
}

impl SchemaRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            schemas: HashMap::new(),
        }
    }

    /// Register a JSON Schema for a task type.
    pub fn register_schema(&mut self, task_type: &str, schema: Value) -> Result<(), String> {
        // Basic validation: must be an object with "type" or "properties"
        if !schema.is_object() {
            return Err("Schema must be a JSON object".to_string());
        }
        self.schemas.insert(task_type.to_string(), schema);
        Ok(())
    }

    /// Get a schema by task type.
    pub fn get_schema(&self, task_type: &str) -> Option<&Value> {
        self.schemas.get(task_type)
    }

    /// Check if a task type has a registered schema.
    pub fn has_schema(&self, task_type: &str) -> bool {
        self.schemas.contains_key(task_type)
    }

    /// List all registered task types.
    pub fn task_types(&self) -> Vec<String> {
        self.schemas.keys().cloned().collect()
    }
}

impl Default for SchemaRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// SchemaValidationGuardrail
// ---------------------------------------------------------------------------

/// Guardrail that validates agent JSON output against registered JSON Schemas.
///
/// Only validates `Direction::Output`. For other directions or when no schema
/// is active, returns `GuardrailResult::Pass`.
pub struct SchemaValidationGuardrail {
    registry: RwLock<SchemaRegistry>,
    active_schema: RwLock<Option<String>>,
}

impl SchemaValidationGuardrail {
    /// Create a new SchemaValidationGuardrail with built-in schemas.
    pub fn new() -> Self {
        let mut registry = SchemaRegistry::new();

        // Register built-in schemas
        let _ = registry.register_schema("prd", prd_schema());
        let _ = registry.register_schema("analysis_report", analysis_report_schema());

        Self {
            registry: RwLock::new(registry),
            active_schema: RwLock::new(None),
        }
    }

    /// Create a new SchemaValidationGuardrail with no built-in schemas.
    pub fn new_empty() -> Self {
        Self {
            registry: RwLock::new(SchemaRegistry::new()),
            active_schema: RwLock::new(None),
        }
    }

    /// Register a new schema for a task type.
    pub fn register_schema(&self, task_type: &str, schema: Value) -> Result<(), String> {
        let mut registry = self.registry.write().map_err(|e| e.to_string())?;
        registry.register_schema(task_type, schema)
    }

    /// Set the active schema for validation.
    pub fn set_active_schema(&self, task_type: &str) {
        if let Ok(mut active) = self.active_schema.write() {
            *active = Some(task_type.to_string());
        }
    }

    /// Clear the active schema (no validation will occur).
    pub fn clear_active_schema(&self) {
        if let Ok(mut active) = self.active_schema.write() {
            *active = None;
        }
    }

    /// Get the currently active schema task type.
    pub fn get_active_schema(&self) -> Option<String> {
        self.active_schema
            .read()
            .ok()
            .and_then(|guard| guard.clone())
    }

    /// Validate a JSON value against a schema, returning detailed errors.
    fn validate_json_against_schema(&self, json_value: &Value, schema: &Value) -> Vec<String> {
        let mut errors = Vec::new();

        // Check required fields
        if let Some(required) = schema.get("required").and_then(|v| v.as_array()) {
            for req in required {
                if let Some(field_name) = req.as_str() {
                    if json_value.get(field_name).is_none() {
                        errors.push(format!("Missing required field: '{}'", field_name));
                    }
                }
            }
        }

        // Check property types
        if let Some(properties) = schema.get("properties").and_then(|v| v.as_object()) {
            for (prop_name, prop_schema) in properties {
                if let Some(value) = json_value.get(prop_name) {
                    // Check type constraint
                    if let Some(expected_type) = prop_schema.get("type").and_then(|v| v.as_str()) {
                        let actual_type = json_type_name(value);
                        if actual_type != expected_type {
                            errors.push(format!(
                                "Type mismatch for '{}': expected {}, got {}",
                                prop_name, expected_type, actual_type
                            ));
                        }
                    }

                    // Check items type for arrays
                    if value.is_array() {
                        if let Some(items_schema) = prop_schema.get("items") {
                            if let Some(item_type) =
                                items_schema.get("type").and_then(|v| v.as_str())
                            {
                                for (idx, item) in value.as_array().unwrap().iter().enumerate() {
                                    let item_actual_type = json_type_name(item);
                                    if item_actual_type != item_type {
                                        errors.push(format!(
                                            "Array item type mismatch for '{}[{}]': expected {}, got {}",
                                            prop_name, idx, item_type, item_actual_type
                                        ));
                                    }

                                    // Check required fields in array items
                                    if let Some(item_required) =
                                        items_schema.get("required").and_then(|v| v.as_array())
                                    {
                                        for req in item_required {
                                            if let Some(field) = req.as_str() {
                                                if item.get(field).is_none() {
                                                    errors.push(format!(
                                                        "Missing required field in '{}[{}]': '{}'",
                                                        prop_name, idx, field
                                                    ));
                                                }
                                            }
                                        }
                                    }

                                    // Check property constraints in array items
                                    if let Some(item_props) =
                                        items_schema.get("properties").and_then(|v| v.as_object())
                                    {
                                        for (item_prop_name, item_prop_schema) in item_props {
                                            if let Some(item_value) = item.get(item_prop_name) {
                                                // Type check
                                                if let Some(exp_type) = item_prop_schema
                                                    .get("type")
                                                    .and_then(|v| v.as_str())
                                                {
                                                    let act_type = json_type_name(item_value);
                                                    if act_type != exp_type {
                                                        errors.push(format!(
                                                            "Type mismatch for '{}[{}].{}': expected {}, got {}",
                                                            prop_name, idx, item_prop_name, exp_type, act_type
                                                        ));
                                                    }
                                                }
                                                // Pattern check
                                                if let Some(pat) = item_prop_schema
                                                    .get("pattern")
                                                    .and_then(|v| v.as_str())
                                                {
                                                    if let Some(sv) = item_value.as_str() {
                                                        if let Ok(re) = regex::Regex::new(pat) {
                                                            if !re.is_match(sv) {
                                                                errors.push(format!(
                                                                    "Pattern violation for '{}[{}].{}': value '{}' does not match '{}'",
                                                                    prop_name, idx, item_prop_name, sv, pat
                                                                ));
                                                            }
                                                        }
                                                    }
                                                }
                                                // minLength check
                                                if let Some(ml) = item_prop_schema
                                                    .get("minLength")
                                                    .and_then(|v| v.as_u64())
                                                {
                                                    if let Some(sv) = item_value.as_str() {
                                                        if (sv.len() as u64) < ml {
                                                            errors.push(format!(
                                                                "String too short for '{}[{}].{}': length {} < minimum {}",
                                                                prop_name, idx, item_prop_name, sv.len(), ml
                                                            ));
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Check pattern constraint
                    if let Some(pattern_str) = prop_schema.get("pattern").and_then(|v| v.as_str()) {
                        if let Some(str_value) = value.as_str() {
                            if let Ok(re) = regex::Regex::new(pattern_str) {
                                if !re.is_match(str_value) {
                                    errors.push(format!(
                                        "Pattern violation for '{}': value '{}' does not match pattern '{}'",
                                        prop_name, str_value, pattern_str
                                    ));
                                }
                            }
                        }
                    }

                    // Check minLength constraint
                    if let Some(min_len) = prop_schema.get("minLength").and_then(|v| v.as_u64()) {
                        if let Some(str_value) = value.as_str() {
                            if (str_value.len() as u64) < min_len {
                                errors.push(format!(
                                    "String too short for '{}': length {} < minimum {}",
                                    prop_name,
                                    str_value.len(),
                                    min_len
                                ));
                            }
                        }
                    }

                    // Check minimum for numbers
                    if let Some(min_val) = prop_schema.get("minimum").and_then(|v| v.as_f64()) {
                        if let Some(num_val) = value.as_f64() {
                            if num_val < min_val {
                                errors.push(format!(
                                    "Value too small for '{}': {} < minimum {}",
                                    prop_name, num_val, min_val
                                ));
                            }
                        }
                    }
                }
            }
        }

        errors
    }
}

impl Default for SchemaValidationGuardrail {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Guardrail for SchemaValidationGuardrail {
    fn name(&self) -> &str {
        "SchemaValidation"
    }

    fn description(&self) -> &str {
        "Validates agent JSON output against registered JSON Schemas"
    }

    async fn validate(&self, content: &str, direction: Direction) -> GuardrailResult {
        // Only validate Output direction
        if direction != Direction::Output {
            return GuardrailResult::Pass;
        }

        // Check if there's an active schema
        let active_task_type = match self.active_schema.read().ok().and_then(|g| g.clone()) {
            Some(task_type) => task_type,
            None => return GuardrailResult::Pass,
        };

        // Get the schema
        let schema = match self
            .registry
            .read()
            .ok()
            .and_then(|r| r.get_schema(&active_task_type).cloned())
        {
            Some(s) => s,
            None => return GuardrailResult::Pass,
        };

        // Parse JSON
        let json_value: Value = match serde_json::from_str(content) {
            Ok(v) => v,
            Err(e) => {
                return GuardrailResult::Block {
                    reason: format!("Invalid JSON: {}", e),
                };
            }
        };

        // Validate against schema
        let errors = self.validate_json_against_schema(&json_value, &schema);

        if errors.is_empty() {
            GuardrailResult::Pass
        } else {
            GuardrailResult::Block {
                reason: format!(
                    "Schema validation failed for '{}' ({} errors):\n{}",
                    active_task_type,
                    errors.len(),
                    errors
                        .iter()
                        .enumerate()
                        .map(|(i, e)| format!("  {}. {}", i + 1, e))
                        .collect::<Vec<_>>()
                        .join("\n")
                ),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Get the JSON Schema type name for a serde_json::Value.
fn json_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(n) => {
            if n.is_f64() && n.as_f64().map_or(false, |f| f.fract() != 0.0) {
                "number"
            } else {
                "integer"
            }
        }
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

// ---------------------------------------------------------------------------
// Built-in schemas
// ---------------------------------------------------------------------------

/// JSON Schema for PRD output format.
fn prd_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "required": ["goal", "stories"],
        "properties": {
            "goal": {
                "type": "string",
                "minLength": 10
            },
            "objectives": {
                "type": "array",
                "items": {
                    "type": "string"
                }
            },
            "stories": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["id", "title", "description"],
                    "properties": {
                        "id": { "type": "string", "pattern": "^story-\\d+" },
                        "title": { "type": "string", "minLength": 5 },
                        "description": { "type": "string", "minLength": 20 },
                        "priority": { "type": "string" },
                        "dependencies": { "type": "array", "items": { "type": "string" } },
                        "acceptance_criteria": { "type": "array", "items": { "type": "string" } }
                    }
                }
            }
        }
    })
}

/// JSON Schema for analysis report format.
fn analysis_report_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "required": ["summary", "recommendations"],
        "properties": {
            "summary": {
                "type": "string",
                "minLength": 20
            },
            "findings": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["title", "description"],
                    "properties": {
                        "title": { "type": "string" },
                        "description": { "type": "string" },
                        "severity": { "type": "string" }
                    }
                }
            },
            "recommendations": {
                "type": "array",
                "items": {
                    "type": "string"
                }
            },
            "risk_level": {
                "type": "string"
            }
        }
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ======================================================================
    // SchemaRegistry tests
    // ======================================================================

    #[test]
    fn registry_register_and_get() {
        let mut registry = SchemaRegistry::new();
        let schema = serde_json::json!({"type": "object", "properties": {}});
        registry.register_schema("test", schema.clone()).unwrap();
        assert!(registry.has_schema("test"));
        assert_eq!(registry.get_schema("test"), Some(&schema));
    }

    #[test]
    fn registry_rejects_non_object_schema() {
        let mut registry = SchemaRegistry::new();
        let result = registry.register_schema("test", serde_json::json!("not an object"));
        assert!(result.is_err());
    }

    #[test]
    fn registry_task_types_lists_all() {
        let mut registry = SchemaRegistry::new();
        registry
            .register_schema("a", serde_json::json!({"type": "object"}))
            .unwrap();
        registry
            .register_schema("b", serde_json::json!({"type": "object"}))
            .unwrap();
        let types = registry.task_types();
        assert_eq!(types.len(), 2);
        assert!(types.contains(&"a".to_string()));
        assert!(types.contains(&"b".to_string()));
    }

    // ======================================================================
    // SchemaValidationGuardrail basic tests
    // ======================================================================

    #[test]
    fn guardrail_name_and_description() {
        let g = SchemaValidationGuardrail::new();
        assert_eq!(g.name(), "SchemaValidation");
        assert!(!g.description().is_empty());
    }

    #[test]
    fn guardrail_has_builtin_schemas() {
        let g = SchemaValidationGuardrail::new();
        let registry = g.registry.read().unwrap();
        assert!(registry.has_schema("prd"));
        assert!(registry.has_schema("analysis_report"));
    }

    // ======================================================================
    // Validation: valid JSON passes
    // ======================================================================

    #[tokio::test]
    async fn valid_prd_passes() {
        let g = SchemaValidationGuardrail::new();
        g.set_active_schema("prd");

        let valid_prd = serde_json::json!({
            "goal": "Build a complete feature with multiple components",
            "objectives": ["Implement X", "Implement Y"],
            "stories": [
                {
                    "id": "story-001",
                    "title": "First story title",
                    "description": "A detailed description of the first story that is long enough",
                    "priority": "high",
                    "dependencies": [],
                    "acceptance_criteria": ["Criterion 1"]
                }
            ]
        });

        let result = g.validate(&valid_prd.to_string(), Direction::Output).await;
        assert!(result.is_pass(), "Valid PRD should pass, got: {:?}", result);
    }

    // ======================================================================
    // Validation: invalid JSON blocks with specific field errors
    // ======================================================================

    #[tokio::test]
    async fn invalid_json_blocks() {
        let g = SchemaValidationGuardrail::new();
        g.set_active_schema("prd");

        let result = g.validate("not valid json {{{", Direction::Output).await;
        assert!(result.is_block(), "Invalid JSON should block");
        if let GuardrailResult::Block { reason } = result {
            assert!(reason.contains("Invalid JSON"));
        }
    }

    // ======================================================================
    // Validation: missing required fields
    // ======================================================================

    #[tokio::test]
    async fn missing_required_fields_blocks() {
        let g = SchemaValidationGuardrail::new();
        g.set_active_schema("prd");

        // Missing "stories" field
        let missing_stories = serde_json::json!({
            "goal": "A sufficiently long goal description here"
        });

        let result = g
            .validate(&missing_stories.to_string(), Direction::Output)
            .await;
        assert!(result.is_block(), "Missing required field should block");
        if let GuardrailResult::Block { reason } = result {
            assert!(
                reason.contains("stories"),
                "Should mention missing 'stories': {}",
                reason
            );
        }
    }

    // ======================================================================
    // Validation: type mismatch detected
    // ======================================================================

    #[tokio::test]
    async fn type_mismatch_blocks() {
        let g = SchemaValidationGuardrail::new();
        g.set_active_schema("prd");

        // "goal" should be string, not number
        let type_mismatch = serde_json::json!({
            "goal": 42,
            "stories": []
        });

        let result = g
            .validate(&type_mismatch.to_string(), Direction::Output)
            .await;
        assert!(result.is_block(), "Type mismatch should block");
        if let GuardrailResult::Block { reason } = result {
            assert!(
                reason.contains("Type mismatch") || reason.contains("type"),
                "Should mention type mismatch: {}",
                reason
            );
        }
    }

    // ======================================================================
    // Validation: no active schema passes
    // ======================================================================

    #[tokio::test]
    async fn no_active_schema_passes() {
        let g = SchemaValidationGuardrail::new();
        // Don't set any active schema
        let result = g.validate("{}", Direction::Output).await;
        assert!(result.is_pass(), "No active schema should pass");
    }

    // ======================================================================
    // Validation: non-Output direction passes
    // ======================================================================

    #[tokio::test]
    async fn non_output_direction_passes() {
        let g = SchemaValidationGuardrail::new();
        g.set_active_schema("prd");

        // Input direction should pass regardless
        let result = g.validate("not even json", Direction::Input).await;
        assert!(result.is_pass(), "Input direction should pass");

        // Tool direction should pass regardless
        let result = g.validate("not even json", Direction::Tool).await;
        assert!(result.is_pass(), "Tool direction should pass");
    }

    // ======================================================================
    // Schema switching
    // ======================================================================

    #[tokio::test]
    async fn schema_switching_works() {
        let g = SchemaValidationGuardrail::new();

        // Set to PRD schema
        g.set_active_schema("prd");
        assert_eq!(g.get_active_schema(), Some("prd".to_string()));

        // Switch to analysis_report schema
        g.set_active_schema("analysis_report");
        assert_eq!(g.get_active_schema(), Some("analysis_report".to_string()));

        // Valid analysis report should pass
        let valid_report = serde_json::json!({
            "summary": "This is a sufficiently long summary of findings",
            "recommendations": ["Recommendation one"]
        });
        let result = g
            .validate(&valid_report.to_string(), Direction::Output)
            .await;
        assert!(result.is_pass(), "Valid report should pass: {:?}", result);

        // Clear active schema
        g.clear_active_schema();
        assert_eq!(g.get_active_schema(), None);

        // Any output should pass now
        let result = g.validate("not json", Direction::Output).await;
        assert!(result.is_pass());
    }

    // ======================================================================
    // Analysis report validation
    // ======================================================================

    #[tokio::test]
    async fn valid_analysis_report_passes() {
        let g = SchemaValidationGuardrail::new();
        g.set_active_schema("analysis_report");

        let report = serde_json::json!({
            "summary": "Comprehensive analysis of the project requirements and implementation",
            "findings": [
                {
                    "title": "Performance issue",
                    "description": "The database queries are not optimized",
                    "severity": "high"
                }
            ],
            "recommendations": ["Optimize DB queries", "Add caching layer"],
            "risk_level": "medium"
        });

        let result = g.validate(&report.to_string(), Direction::Output).await;
        assert!(result.is_pass(), "Valid report should pass: {:?}", result);
    }

    #[tokio::test]
    async fn invalid_analysis_report_blocks() {
        let g = SchemaValidationGuardrail::new();
        g.set_active_schema("analysis_report");

        // Missing required "recommendations"
        let report = serde_json::json!({
            "summary": "A summary that is long enough for validation"
        });

        let result = g.validate(&report.to_string(), Direction::Output).await;
        assert!(result.is_block(), "Missing recommendations should block");
    }

    // ======================================================================
    // Custom schema registration
    // ======================================================================

    #[tokio::test]
    async fn custom_schema_registration_and_validation() {
        let g = SchemaValidationGuardrail::new_empty();

        let schema = serde_json::json!({
            "type": "object",
            "required": ["name", "value"],
            "properties": {
                "name": { "type": "string" },
                "value": { "type": "integer" }
            }
        });

        g.register_schema("custom_task", schema).unwrap();
        g.set_active_schema("custom_task");

        // Valid
        let valid = serde_json::json!({"name": "test", "value": 42});
        let result = g.validate(&valid.to_string(), Direction::Output).await;
        assert!(result.is_pass());

        // Invalid: wrong type for value
        let invalid = serde_json::json!({"name": "test", "value": "not a number"});
        let result = g.validate(&invalid.to_string(), Direction::Output).await;
        assert!(result.is_block());
    }

    // ======================================================================
    // json_type_name helper tests
    // ======================================================================

    #[test]
    fn test_json_type_name() {
        assert_eq!(json_type_name(&Value::Null), "null");
        assert_eq!(json_type_name(&Value::Bool(true)), "boolean");
        assert_eq!(json_type_name(&serde_json::json!(42)), "integer");
        assert_eq!(json_type_name(&serde_json::json!(3.14)), "number");
        assert_eq!(json_type_name(&serde_json::json!("hello")), "string");
        assert_eq!(json_type_name(&serde_json::json!([])), "array");
        assert_eq!(json_type_name(&serde_json::json!({})), "object");
    }

    // ======================================================================
    // Pattern validation tests
    // ======================================================================

    #[tokio::test]
    async fn pattern_validation_works() {
        let g = SchemaValidationGuardrail::new();
        g.set_active_schema("prd");

        // Story with invalid id pattern (doesn't match ^story-\d+)
        let invalid_pattern = serde_json::json!({
            "goal": "A sufficiently long goal description here",
            "stories": [
                {
                    "id": "invalid-id",
                    "title": "A valid title here",
                    "description": "A long enough description for the story validation"
                }
            ]
        });

        let result = g
            .validate(&invalid_pattern.to_string(), Direction::Output)
            .await;
        assert!(
            result.is_block(),
            "Invalid story id pattern should block: {:?}",
            result
        );
    }

    // ======================================================================
    // Error detail quality
    // ======================================================================

    #[tokio::test]
    async fn error_details_are_comprehensive() {
        let g = SchemaValidationGuardrail::new();
        g.set_active_schema("prd");

        // Multiple errors: missing fields and type mismatch
        let multiple_errors = serde_json::json!({
            "goal": 123
        });

        let result = g
            .validate(&multiple_errors.to_string(), Direction::Output)
            .await;
        if let GuardrailResult::Block { reason } = result {
            assert!(
                reason.contains("errors"),
                "Should mention error count: {}",
                reason
            );
            assert!(
                reason.contains("stories") || reason.contains("goal"),
                "Should mention specific fields: {}",
                reason
            );
        } else {
            panic!("Should be Block, got: {:?}", result);
        }
    }
}
