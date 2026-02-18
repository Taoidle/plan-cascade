//! Builder Pattern & Session State Prefixes
//!
//! Provides builder patterns for configuration structs and validated
//! session state key prefixes.
//!
//! ## Builders
//!
//! Each builder follows the standard Rust builder pattern:
//! 1. Create with `::new()` or `::default()`
//! 2. Chain `.field(value)` calls
//! 3. Call `.build()` which validates and returns `CoreResult<Config>`
//!
//! Validation happens at build time, catching configuration errors
//! before they cause runtime failures.
//!
//! ## Session State Key Prefixes
//!
//! All session state keys must use one of three prefixes:
//! - `user:` - User-visible state, persisted across sessions
//! - `app:` - Application-internal state, persisted but not user-visible
//! - `temp:` - Temporary state, cleared when the session ends
//!
//! This convention prevents key collisions between user data,
//! application internals, and transient execution state.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{CoreError, CoreResult};

// ============================================================================
// SessionStateKey
// ============================================================================

/// Validated session state key with enforced prefix convention.
///
/// Keys must use one of the following prefixes:
/// - `user:` - User-facing state (persisted across sessions)
/// - `app:` - Application-internal state (persisted, not user-visible)
/// - `temp:` - Temporary state (cleared on session end)
///
/// # Examples
/// ```ignore
/// let key = SessionStateKey::new("user:preferred_model")?;
/// let key = SessionStateKey::new("app:last_compaction_time")?;
/// let key = SessionStateKey::new("temp:current_tool_call_id")?;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionStateKey(String);

/// The valid prefixes for session state keys.
pub const SESSION_KEY_PREFIXES: [&str; 3] = ["user:", "app:", "temp:"];

impl SessionStateKey {
    /// Validate and create a session state key.
    ///
    /// Returns an error if the key doesn't start with a valid prefix.
    pub fn new(key: impl Into<String>) -> CoreResult<Self> {
        let key = key.into();
        if key.is_empty() {
            return Err(CoreError::validation("Session state key cannot be empty"));
        }
        if SESSION_KEY_PREFIXES.iter().any(|p| key.starts_with(p)) {
            // Validate that there's something after the prefix
            let after_prefix = if key.starts_with("user:") {
                &key[5..]
            } else if key.starts_with("app:") {
                &key[4..]
            } else {
                &key[5..]
            };
            if after_prefix.is_empty() {
                return Err(CoreError::validation(format!(
                    "Session state key must have a name after the prefix. Got: '{}'",
                    key
                )));
            }
            Ok(Self(key))
        } else {
            Err(CoreError::validation(format!(
                "Session state key must start with 'user:', 'app:', or 'temp:'. Got: '{}'",
                key
            )))
        }
    }

    /// Create a user-scoped key.
    pub fn user(name: impl Into<String>) -> CoreResult<Self> {
        let name = name.into();
        if name.is_empty() {
            return Err(CoreError::validation("Key name cannot be empty"));
        }
        Ok(Self(format!("user:{}", name)))
    }

    /// Create an app-scoped key.
    pub fn app(name: impl Into<String>) -> CoreResult<Self> {
        let name = name.into();
        if name.is_empty() {
            return Err(CoreError::validation("Key name cannot be empty"));
        }
        Ok(Self(format!("app:{}", name)))
    }

    /// Create a temp-scoped key.
    pub fn temp(name: impl Into<String>) -> CoreResult<Self> {
        let name = name.into();
        if name.is_empty() {
            return Err(CoreError::validation("Key name cannot be empty"));
        }
        Ok(Self(format!("temp:{}", name)))
    }

    /// Get the key string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Get the prefix of this key.
    pub fn prefix(&self) -> &str {
        if self.0.starts_with("user:") {
            "user:"
        } else if self.0.starts_with("app:") {
            "app:"
        } else {
            "temp:"
        }
    }

    /// Get the name part (after the prefix).
    pub fn name(&self) -> &str {
        let prefix_len = self.prefix().len();
        &self.0[prefix_len..]
    }

    /// Check if this is a user key.
    pub fn is_user(&self) -> bool {
        self.0.starts_with("user:")
    }

    /// Check if this is an app key.
    pub fn is_app(&self) -> bool {
        self.0.starts_with("app:")
    }

    /// Check if this is a temporary key.
    pub fn is_temp(&self) -> bool {
        self.0.starts_with("temp:")
    }
}

impl std::fmt::Display for SessionStateKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ============================================================================
// SessionState - validated key-value store
// ============================================================================

/// A validated session state store that enforces key prefixes.
#[derive(Debug, Clone, Default)]
pub struct SessionState {
    entries: HashMap<SessionStateKey, Value>,
}

impl SessionState {
    /// Create an empty session state.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Set a value with a validated key.
    pub fn set(&mut self, key: SessionStateKey, value: Value) {
        self.entries.insert(key, value);
    }

    /// Get a value by key.
    pub fn get(&self, key: &SessionStateKey) -> Option<&Value> {
        self.entries.get(key)
    }

    /// Remove a value by key.
    pub fn remove(&mut self, key: &SessionStateKey) -> Option<Value> {
        self.entries.remove(key)
    }

    /// Remove all temporary keys.
    pub fn clear_temp(&mut self) {
        self.entries.retain(|k, _| !k.is_temp());
    }

    /// Get all keys with a given prefix.
    pub fn keys_with_prefix(&self, prefix: &str) -> Vec<&SessionStateKey> {
        self.entries
            .keys()
            .filter(|k| k.as_str().starts_with(prefix))
            .collect()
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ============================================================================
// AgentConfigBuilder
// ============================================================================

/// Built agent configuration (output of AgentConfigBuilder).
///
/// This is the core-layer representation. It maps to the existing
/// `AgentConfig` in `agent_composer::types` but with validated fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuiltAgentConfig {
    pub max_iterations: u32,
    pub max_total_tokens: u32,
    pub streaming: bool,
    pub enable_compaction: bool,
    pub temperature: Option<f32>,
}

/// Builder for agent configuration with validation at build time.
///
/// # Example
/// ```ignore
/// let config = AgentConfigBuilder::new()
///     .max_iterations(100)
///     .temperature(0.7)
///     .build()?;
/// ```
#[derive(Debug, Default)]
pub struct AgentConfigBuilder {
    max_iterations: Option<u32>,
    max_total_tokens: Option<u32>,
    streaming: Option<bool>,
    enable_compaction: Option<bool>,
    temperature: Option<f32>,
}

impl AgentConfigBuilder {
    /// Create a new builder with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set maximum iterations (must be > 0 and <= 10000).
    pub fn max_iterations(mut self, n: u32) -> Self {
        self.max_iterations = Some(n);
        self
    }

    /// Set maximum total tokens (must be > 0).
    pub fn max_total_tokens(mut self, n: u32) -> Self {
        self.max_total_tokens = Some(n);
        self
    }

    /// Enable or disable streaming.
    pub fn streaming(mut self, enabled: bool) -> Self {
        self.streaming = Some(enabled);
        self
    }

    /// Enable or disable context compaction.
    pub fn enable_compaction(mut self, enabled: bool) -> Self {
        self.enable_compaction = Some(enabled);
        self
    }

    /// Set the temperature (must be between 0.0 and 2.0).
    pub fn temperature(mut self, t: f32) -> Self {
        self.temperature = Some(t);
        self
    }

    /// Build and validate the configuration.
    pub fn build(self) -> CoreResult<BuiltAgentConfig> {
        let max_iterations = self.max_iterations.unwrap_or(50);
        let max_total_tokens = self.max_total_tokens.unwrap_or(1_000_000);
        let streaming = self.streaming.unwrap_or(true);
        let enable_compaction = self.enable_compaction.unwrap_or(true);

        // Validation
        if max_iterations == 0 {
            return Err(CoreError::validation("max_iterations must be > 0"));
        }
        if max_iterations > 10_000 {
            return Err(CoreError::validation(
                "max_iterations must be <= 10000",
            ));
        }
        if max_total_tokens == 0 {
            return Err(CoreError::validation("max_total_tokens must be > 0"));
        }
        if let Some(t) = self.temperature {
            if !(0.0..=2.0).contains(&t) {
                return Err(CoreError::validation(
                    "temperature must be between 0.0 and 2.0",
                ));
            }
        }

        Ok(BuiltAgentConfig {
            max_iterations,
            max_total_tokens,
            streaming,
            enable_compaction,
            temperature: self.temperature,
        })
    }
}

// ============================================================================
// ExecutionConfigBuilder
// ============================================================================

/// Built execution configuration (output of ExecutionConfigBuilder).
#[derive(Debug, Clone)]
pub struct BuiltExecutionConfig {
    pub session_id: String,
    pub project_root: PathBuf,
    pub max_iterations: u32,
    pub max_total_tokens: u32,
    pub enable_compaction: bool,
}

/// Builder for execution configuration.
///
/// `session_id` and `project_root` are required fields.
///
/// # Example
/// ```ignore
/// let config = ExecutionConfigBuilder::new()
///     .session_id("sess-123")
///     .project_root("/path/to/project")
///     .max_iterations(100)
///     .build()?;
/// ```
#[derive(Debug, Default)]
pub struct ExecutionConfigBuilder {
    session_id: Option<String>,
    project_root: Option<PathBuf>,
    max_iterations: Option<u32>,
    max_total_tokens: Option<u32>,
    enable_compaction: Option<bool>,
}

impl ExecutionConfigBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the session ID (required).
    pub fn session_id(mut self, id: impl Into<String>) -> Self {
        self.session_id = Some(id.into());
        self
    }

    /// Set the project root path (required).
    pub fn project_root(mut self, path: impl Into<PathBuf>) -> Self {
        self.project_root = Some(path.into());
        self
    }

    /// Set maximum iterations.
    pub fn max_iterations(mut self, n: u32) -> Self {
        self.max_iterations = Some(n);
        self
    }

    /// Set maximum total tokens.
    pub fn max_total_tokens(mut self, n: u32) -> Self {
        self.max_total_tokens = Some(n);
        self
    }

    /// Enable or disable context compaction.
    pub fn enable_compaction(mut self, enabled: bool) -> Self {
        self.enable_compaction = Some(enabled);
        self
    }

    /// Build and validate the configuration.
    pub fn build(self) -> CoreResult<BuiltExecutionConfig> {
        let session_id = self
            .session_id
            .ok_or_else(|| CoreError::validation("session_id is required"))?;
        let project_root = self
            .project_root
            .ok_or_else(|| CoreError::validation("project_root is required"))?;

        if session_id.is_empty() {
            return Err(CoreError::validation("session_id cannot be empty"));
        }

        let max_iterations = self.max_iterations.unwrap_or(50);
        let max_total_tokens = self.max_total_tokens.unwrap_or(1_000_000);

        if max_iterations == 0 {
            return Err(CoreError::validation("max_iterations must be > 0"));
        }

        Ok(BuiltExecutionConfig {
            session_id,
            project_root,
            max_iterations,
            max_total_tokens,
            enable_compaction: self.enable_compaction.unwrap_or(true),
        })
    }
}

// ============================================================================
// QualityGateConfigBuilder
// ============================================================================

/// Built quality gate configuration (output of QualityGateConfigBuilder).
#[derive(Debug, Clone)]
pub struct BuiltQualityGateConfig {
    pub gates: Vec<String>,
    pub fail_fast: bool,
    pub timeout_secs: u64,
}

/// Builder for quality gate configuration.
///
/// # Example
/// ```ignore
/// let config = QualityGateConfigBuilder::new()
///     .gate("typecheck")
///     .gate("lint")
///     .gate("test")
///     .fail_fast(true)
///     .timeout_secs(300)
///     .build()?;
/// ```
#[derive(Debug, Default)]
pub struct QualityGateConfigBuilder {
    gates: Vec<String>,
    fail_fast: Option<bool>,
    timeout_secs: Option<u64>,
}

impl QualityGateConfigBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a quality gate by name.
    pub fn gate(mut self, name: impl Into<String>) -> Self {
        self.gates.push(name.into());
        self
    }

    /// Add multiple gates at once.
    pub fn gates(mut self, names: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.gates.extend(names.into_iter().map(|n| n.into()));
        self
    }

    /// Set fail-fast mode (stop on first gate failure).
    pub fn fail_fast(mut self, enabled: bool) -> Self {
        self.fail_fast = Some(enabled);
        self
    }

    /// Set timeout in seconds per gate.
    pub fn timeout_secs(mut self, secs: u64) -> Self {
        self.timeout_secs = Some(secs);
        self
    }

    /// Build and validate the configuration.
    pub fn build(self) -> CoreResult<BuiltQualityGateConfig> {
        if self.gates.is_empty() {
            return Err(CoreError::validation(
                "At least one quality gate must be specified",
            ));
        }

        // Check for duplicate gate names
        let mut seen = std::collections::HashSet::new();
        for gate in &self.gates {
            if !seen.insert(gate.as_str()) {
                return Err(CoreError::validation(format!(
                    "Duplicate quality gate: '{}'",
                    gate
                )));
            }
        }

        let timeout = self.timeout_secs.unwrap_or(300);
        if timeout == 0 {
            return Err(CoreError::validation("timeout_secs must be > 0"));
        }

        Ok(BuiltQualityGateConfig {
            gates: self.gates,
            fail_fast: self.fail_fast.unwrap_or(false),
            timeout_secs: timeout,
        })
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- SessionStateKey tests --

    #[test]
    fn test_session_state_key_valid_prefixes() {
        assert!(SessionStateKey::new("user:name").is_ok());
        assert!(SessionStateKey::new("app:config").is_ok());
        assert!(SessionStateKey::new("temp:scratch").is_ok());
    }

    #[test]
    fn test_session_state_key_invalid_prefix() {
        assert!(SessionStateKey::new("invalid:key").is_err());
        assert!(SessionStateKey::new("no_prefix").is_err());
        assert!(SessionStateKey::new("USER:name").is_err()); // case sensitive
    }

    #[test]
    fn test_session_state_key_empty() {
        assert!(SessionStateKey::new("").is_err());
    }

    #[test]
    fn test_session_state_key_prefix_only() {
        assert!(SessionStateKey::new("user:").is_err());
        assert!(SessionStateKey::new("app:").is_err());
        assert!(SessionStateKey::new("temp:").is_err());
    }

    #[test]
    fn test_session_state_key_convenience_constructors() {
        let key = SessionStateKey::user("model").unwrap();
        assert_eq!(key.as_str(), "user:model");
        assert!(key.is_user());
        assert!(!key.is_app());
        assert!(!key.is_temp());

        let key = SessionStateKey::app("version").unwrap();
        assert_eq!(key.as_str(), "app:version");
        assert!(key.is_app());

        let key = SessionStateKey::temp("scratch").unwrap();
        assert_eq!(key.as_str(), "temp:scratch");
        assert!(key.is_temp());
    }

    #[test]
    fn test_session_state_key_convenience_empty_name() {
        assert!(SessionStateKey::user("").is_err());
        assert!(SessionStateKey::app("").is_err());
        assert!(SessionStateKey::temp("").is_err());
    }

    #[test]
    fn test_session_state_key_prefix() {
        assert_eq!(SessionStateKey::new("user:x").unwrap().prefix(), "user:");
        assert_eq!(SessionStateKey::new("app:x").unwrap().prefix(), "app:");
        assert_eq!(SessionStateKey::new("temp:x").unwrap().prefix(), "temp:");
    }

    #[test]
    fn test_session_state_key_name() {
        assert_eq!(SessionStateKey::new("user:model").unwrap().name(), "model");
        assert_eq!(
            SessionStateKey::new("app:nested:key").unwrap().name(),
            "nested:key"
        );
    }

    #[test]
    fn test_session_state_key_display() {
        let key = SessionStateKey::new("user:name").unwrap();
        assert_eq!(format!("{}", key), "user:name");
    }

    #[test]
    fn test_session_state_key_serialization() {
        let key = SessionStateKey::new("app:config").unwrap();
        let json = serde_json::to_string(&key).unwrap();
        assert_eq!(json, r#""app:config""#);

        let parsed: SessionStateKey = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, key);
    }

    #[test]
    fn test_session_state_key_hash_map_key() {
        let mut map = HashMap::new();
        let key1 = SessionStateKey::user("a").unwrap();
        let key2 = SessionStateKey::user("b").unwrap();

        map.insert(key1.clone(), Value::Bool(true));
        map.insert(key2.clone(), Value::Bool(false));

        assert_eq!(map.get(&key1), Some(&Value::Bool(true)));
        assert_eq!(map.get(&key2), Some(&Value::Bool(false)));
    }

    // -- SessionState tests --

    #[test]
    fn test_session_state_basic_operations() {
        let mut state = SessionState::new();
        assert!(state.is_empty());

        let key = SessionStateKey::user("name").unwrap();
        state.set(key.clone(), Value::String("Alice".to_string()));
        assert_eq!(state.len(), 1);
        assert_eq!(
            state.get(&key),
            Some(&Value::String("Alice".to_string()))
        );
    }

    #[test]
    fn test_session_state_remove() {
        let mut state = SessionState::new();
        let key = SessionStateKey::app("x").unwrap();
        state.set(key.clone(), Value::Null);

        let removed = state.remove(&key);
        assert!(removed.is_some());
        assert!(state.is_empty());
    }

    #[test]
    fn test_session_state_clear_temp() {
        let mut state = SessionState::new();
        state.set(SessionStateKey::user("persist").unwrap(), Value::Bool(true));
        state.set(SessionStateKey::app("persist2").unwrap(), Value::Bool(true));
        state.set(SessionStateKey::temp("scratch1").unwrap(), Value::Null);
        state.set(SessionStateKey::temp("scratch2").unwrap(), Value::Null);

        assert_eq!(state.len(), 4);
        state.clear_temp();
        assert_eq!(state.len(), 2);

        // User and app keys preserved
        assert!(state.get(&SessionStateKey::user("persist").unwrap()).is_some());
        assert!(state.get(&SessionStateKey::app("persist2").unwrap()).is_some());
    }

    #[test]
    fn test_session_state_keys_with_prefix() {
        let mut state = SessionState::new();
        state.set(SessionStateKey::user("a").unwrap(), Value::Null);
        state.set(SessionStateKey::user("b").unwrap(), Value::Null);
        state.set(SessionStateKey::app("c").unwrap(), Value::Null);
        state.set(SessionStateKey::temp("d").unwrap(), Value::Null);

        let user_keys = state.keys_with_prefix("user:");
        assert_eq!(user_keys.len(), 2);

        let app_keys = state.keys_with_prefix("app:");
        assert_eq!(app_keys.len(), 1);

        let temp_keys = state.keys_with_prefix("temp:");
        assert_eq!(temp_keys.len(), 1);
    }

    // -- AgentConfigBuilder tests --

    #[test]
    fn test_agent_config_builder_defaults() {
        let config = AgentConfigBuilder::new().build().unwrap();
        assert_eq!(config.max_iterations, 50);
        assert_eq!(config.max_total_tokens, 1_000_000);
        assert!(config.streaming);
        assert!(config.enable_compaction);
        assert!(config.temperature.is_none());
    }

    #[test]
    fn test_agent_config_builder_custom_values() {
        let config = AgentConfigBuilder::new()
            .max_iterations(100)
            .max_total_tokens(500_000)
            .streaming(false)
            .enable_compaction(false)
            .temperature(0.7)
            .build()
            .unwrap();

        assert_eq!(config.max_iterations, 100);
        assert_eq!(config.max_total_tokens, 500_000);
        assert!(!config.streaming);
        assert!(!config.enable_compaction);
        assert_eq!(config.temperature, Some(0.7));
    }

    #[test]
    fn test_agent_config_builder_zero_iterations_fails() {
        let result = AgentConfigBuilder::new().max_iterations(0).build();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("max_iterations"));
    }

    #[test]
    fn test_agent_config_builder_too_many_iterations_fails() {
        let result = AgentConfigBuilder::new().max_iterations(10_001).build();
        assert!(result.is_err());
    }

    #[test]
    fn test_agent_config_builder_zero_tokens_fails() {
        let result = AgentConfigBuilder::new().max_total_tokens(0).build();
        assert!(result.is_err());
    }

    #[test]
    fn test_agent_config_builder_temperature_out_of_range() {
        assert!(AgentConfigBuilder::new().temperature(-0.1).build().is_err());
        assert!(AgentConfigBuilder::new().temperature(2.1).build().is_err());
        assert!(AgentConfigBuilder::new().temperature(0.0).build().is_ok());
        assert!(AgentConfigBuilder::new().temperature(2.0).build().is_ok());
    }

    // -- ExecutionConfigBuilder tests --

    #[test]
    fn test_execution_config_builder_valid() {
        let config = ExecutionConfigBuilder::new()
            .session_id("sess-123")
            .project_root("/path/to/project")
            .build()
            .unwrap();

        assert_eq!(config.session_id, "sess-123");
        assert_eq!(config.project_root, PathBuf::from("/path/to/project"));
        assert_eq!(config.max_iterations, 50);
        assert_eq!(config.max_total_tokens, 1_000_000);
        assert!(config.enable_compaction);
    }

    #[test]
    fn test_execution_config_builder_custom_values() {
        let config = ExecutionConfigBuilder::new()
            .session_id("s-1")
            .project_root("/proj")
            .max_iterations(200)
            .max_total_tokens(2_000_000)
            .enable_compaction(false)
            .build()
            .unwrap();

        assert_eq!(config.max_iterations, 200);
        assert_eq!(config.max_total_tokens, 2_000_000);
        assert!(!config.enable_compaction);
    }

    #[test]
    fn test_execution_config_builder_missing_session_id() {
        let result = ExecutionConfigBuilder::new()
            .project_root("/proj")
            .build();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("session_id"));
    }

    #[test]
    fn test_execution_config_builder_missing_project_root() {
        let result = ExecutionConfigBuilder::new()
            .session_id("s-1")
            .build();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("project_root"));
    }

    #[test]
    fn test_execution_config_builder_empty_session_id() {
        let result = ExecutionConfigBuilder::new()
            .session_id("")
            .project_root("/proj")
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn test_execution_config_builder_zero_iterations() {
        let result = ExecutionConfigBuilder::new()
            .session_id("s")
            .project_root("/p")
            .max_iterations(0)
            .build();
        assert!(result.is_err());
    }

    // -- QualityGateConfigBuilder tests --

    #[test]
    fn test_quality_gate_config_builder_basic() {
        let config = QualityGateConfigBuilder::new()
            .gate("typecheck")
            .gate("lint")
            .build()
            .unwrap();

        assert_eq!(config.gates, vec!["typecheck", "lint"]);
        assert!(!config.fail_fast);
        assert_eq!(config.timeout_secs, 300);
    }

    #[test]
    fn test_quality_gate_config_builder_custom() {
        let config = QualityGateConfigBuilder::new()
            .gate("typecheck")
            .gate("test")
            .fail_fast(true)
            .timeout_secs(60)
            .build()
            .unwrap();

        assert!(config.fail_fast);
        assert_eq!(config.timeout_secs, 60);
    }

    #[test]
    fn test_quality_gate_config_builder_gates_batch() {
        let config = QualityGateConfigBuilder::new()
            .gates(vec!["typecheck", "lint", "test"])
            .build()
            .unwrap();

        assert_eq!(config.gates.len(), 3);
    }

    #[test]
    fn test_quality_gate_config_builder_no_gates_fails() {
        let result = QualityGateConfigBuilder::new().build();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("At least one quality gate"));
    }

    #[test]
    fn test_quality_gate_config_builder_duplicate_gates_fails() {
        let result = QualityGateConfigBuilder::new()
            .gate("lint")
            .gate("lint")
            .build();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Duplicate"));
    }

    #[test]
    fn test_quality_gate_config_builder_zero_timeout_fails() {
        let result = QualityGateConfigBuilder::new()
            .gate("test")
            .timeout_secs(0)
            .build();
        assert!(result.is_err());
    }

    // -- BuiltAgentConfig serialization tests --

    #[test]
    fn test_built_agent_config_serialization() {
        let config = AgentConfigBuilder::new()
            .max_iterations(100)
            .temperature(0.5)
            .build()
            .unwrap();

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"max_iterations\":100"));
        assert!(json.contains("\"temperature\":0.5"));

        let parsed: BuiltAgentConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.max_iterations, 100);
        assert_eq!(parsed.temperature, Some(0.5));
    }
}
