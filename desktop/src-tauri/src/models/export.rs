//! Unified Settings Export/Import Models (v6.0)
//!
//! Data structures for the unified settings export/import system that covers
//! both frontend (Zustand) and backend (SQLite, config, secrets) settings.

use serde::{Deserialize, Serialize};

/// Top-level unified settings export structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedSettingsExport {
    /// Format version (currently "6.0")
    pub version: String,
    /// ISO 8601 export timestamp
    pub exported_at: String,
    /// Whether encrypted secrets are included
    pub has_encrypted_secrets: bool,
    /// Frontend Zustand store state (passed through from frontend)
    pub frontend: serde_json::Value,
    /// All backend settings collected from various data sources
    pub backend: BackendSettingsExport,
    /// Password-encrypted API keys: base64(salt[16] || nonce[12] || ciphertext || tag)
    pub encrypted_secrets: Option<String>,
}

/// Backend settings collected from SQLite, config files, etc.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendSettingsExport {
    /// AppConfig from ~/.plan-cascade/config.json
    pub config: serde_json::Value,
    /// Embedding configuration from SQLite settings table
    pub embedding: Option<serde_json::Value>,
    /// Proxy settings (global + per-strategy + custom)
    pub proxy: ProxyExport,
    /// Webhook channel configurations
    pub webhooks: Vec<serde_json::Value>,
    /// Custom guardrail rules
    pub guardrails: Vec<GuardrailRuleExport>,
    /// Remote control settings (gateway + telegram)
    pub remote: RemoteExport,
    /// A2A registered remote agents
    pub a2a_agents: Vec<serde_json::Value>,
    /// MCP server configurations (status reset to "unknown")
    pub mcp_servers: Vec<serde_json::Value>,
    /// Plugin settings from ~/.plan-cascade/plugin-settings.json
    pub plugin_settings: Option<serde_json::Value>,
}

/// Proxy settings export structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyExport {
    /// Global proxy config
    pub global: Option<serde_json::Value>,
    /// Per-provider strategy settings (proxy_strategy_* keys)
    pub strategies: serde_json::Value,
    /// Custom proxy configurations (proxy_custom_* keys)
    pub custom_configs: serde_json::Value,
}

/// Guardrail rule export structure (matches guardrail_rules table schema).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardrailRuleExport {
    pub id: String,
    pub name: String,
    pub pattern: String,
    pub action: String,
    pub enabled: bool,
}

/// Remote control settings export.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteExport {
    /// Remote gateway configuration
    pub gateway: Option<serde_json::Value>,
    /// Telegram bot configuration
    pub telegram: Option<serde_json::Value>,
}

/// Result of an import operation, with per-section status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsImportResult {
    /// Overall success (true if no errors, even with warnings)
    pub success: bool,
    /// Frontend state to apply to Zustand (returned to frontend)
    pub frontend: Option<serde_json::Value>,
    /// Sections that were successfully imported
    pub imported_sections: Vec<String>,
    /// Sections that were skipped (missing or unchanged)
    pub skipped_sections: Vec<String>,
    /// Non-fatal warnings
    pub warnings: Vec<String>,
    /// Fatal errors per section
    pub errors: Vec<String>,
}
