//! Tool Permission Policy v2
//!
//! Defines permission levels, coarse risk categories, and a rule-based policy
//! evaluator used by `PermissionGate`.

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use url::Url;

/// Versioned built-in allowlist configuration for Bash network auto-approval.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltinNetworkDomainAllowlistConfig {
    /// Version id for this built-in preset.
    pub version: &'static str,
    /// Domain entries (exact or parent domain match for subdomains).
    pub domains: &'static [&'static str],
}

/// Current built-in allowlist version to apply at runtime.
const CURRENT_BUILTIN_NETWORK_ALLOWLIST_VERSION: &str = "1.0.0";

/// Built-in allowlist preset v1.0.0.
///
/// Focused on common developer package/source distribution endpoints.
const BUILTIN_NETWORK_DOMAIN_ALLOWLIST_V1_0_0: BuiltinNetworkDomainAllowlistConfig =
    BuiltinNetworkDomainAllowlistConfig {
        version: "1.0.0",
        domains: &[
            "github.com",
            "api.github.com",
            "raw.githubusercontent.com",
            "registry.npmjs.org",
            "pypi.org",
            "files.pythonhosted.org",
            "crates.io",
            "index.crates.io",
            "proxy.golang.org",
            "sum.golang.org",
            "repo.maven.apache.org",
            "rubygems.org",
        ],
    };

/// All built-in allowlist presets known by this binary.
const BUILTIN_NETWORK_ALLOWLIST_CONFIGS: &[BuiltinNetworkDomainAllowlistConfig] =
    &[BUILTIN_NETWORK_DOMAIN_ALLOWLIST_V1_0_0];

/// Get the built-in domain allowlist used by Policy v2.
pub fn builtin_network_domain_allowlist() -> &'static [&'static str] {
    builtin_network_domain_allowlist_config().domains
}

/// Get the current built-in allowlist config.
pub fn builtin_network_domain_allowlist_config() -> &'static BuiltinNetworkDomainAllowlistConfig {
    builtin_network_domain_allowlist_config_by_version(CURRENT_BUILTIN_NETWORK_ALLOWLIST_VERSION)
        .unwrap_or(&BUILTIN_NETWORK_DOMAIN_ALLOWLIST_V1_0_0)
}

/// Get a built-in allowlist config by version id.
pub fn builtin_network_domain_allowlist_config_by_version(
    version: &str,
) -> Option<&'static BuiltinNetworkDomainAllowlistConfig> {
    BUILTIN_NETWORK_ALLOWLIST_CONFIGS
        .iter()
        .find(|cfg| cfg.version == version)
}

/// Get the current built-in allowlist version id.
pub fn builtin_network_domain_allowlist_version() -> &'static str {
    builtin_network_domain_allowlist_config().version
}

/// Get all available built-in allowlist version ids.
pub fn builtin_network_domain_allowlist_available_versions() -> Vec<&'static str> {
    BUILTIN_NETWORK_ALLOWLIST_CONFIGS
        .iter()
        .map(|cfg| cfg.version)
        .collect()
}

/// Session-level permission mode. Determines which risk categories require approval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionLevel {
    /// All write and dangerous operations require approval
    Strict,
    /// Only dangerous operations require approval
    Standard,
    /// All operations auto-approved by default (policy rules can still prompt/deny)
    Permissive,
}

impl Default for PermissionLevel {
    fn default() -> Self {
        Self::Strict
    }
}

/// Risk classification for a tool invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ToolRisk {
    /// Read-only operations: never require approval by level-only fallback
    ReadOnly,
    /// Safe write operations (file create/edit): require approval in Strict mode
    SafeWrite,
    /// Dangerous operations (shell, browser, sub-agents): require approval in Strict + Standard
    Dangerous,
}

impl ToolRisk {
    /// Serialization name for the streaming event.
    pub fn as_str(&self) -> &'static str {
        match self {
            ToolRisk::ReadOnly => "ReadOnly",
            ToolRisk::SafeWrite => "SafeWrite",
            ToolRisk::Dangerous => "Dangerous",
        }
    }
}

/// Action decided by Policy v2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyAction {
    Allow,
    Prompt,
    Deny,
}

/// Runtime policy decision returned by Policy v2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyDecision {
    pub action: PolicyAction,
    pub risk: ToolRisk,
    pub reason: String,
    /// Scope key used by "always allow" caching in the permission gate.
    pub approval_scope_key: String,
}

impl PolicyDecision {
    fn allow(risk: ToolRisk, reason: impl Into<String>) -> Self {
        Self {
            action: PolicyAction::Allow,
            risk,
            reason: reason.into(),
            approval_scope_key: String::new(),
        }
    }

    fn prompt(risk: ToolRisk, reason: impl Into<String>, approval_scope_key: String) -> Self {
        Self {
            action: PolicyAction::Prompt,
            risk,
            reason: reason.into(),
            approval_scope_key,
        }
    }

    fn deny(risk: ToolRisk, reason: impl Into<String>) -> Self {
        Self {
            action: PolicyAction::Deny,
            risk,
            reason: reason.into(),
            approval_scope_key: String::new(),
        }
    }
}

/// Configurable Policy v2 options.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct PermissionPolicyConfig {
    /// Auto-allow Bash network calls only when every resolved destination domain
    /// matches either this custom allowlist or the built-in allowlist
    /// (exact or subdomain match).
    pub network_domain_allowlist: Vec<String>,
}

impl Default for PermissionPolicyConfig {
    fn default() -> Self {
        Self {
            network_domain_allowlist: Vec::new(),
        }
    }
}

/// Input context for Policy v2 evaluation.
pub struct PolicyInput<'a> {
    pub tool_name: &'a str,
    pub args: &'a Value,
    pub level: PermissionLevel,
    pub working_dir: &'a Path,
    pub project_root: &'a Path,
}

/// Classify the baseline risk level of a tool invocation.
///
/// | Tool                | Risk        |
/// |---------------------|-------------|
/// | Read, Glob, Grep, LS, Cwd, CodebaseSearch, WebFetch, WebSearch, Analyze | ReadOnly |
/// | Write, Edit, NotebookEdit | SafeWrite |
/// | Bash, Browser, Task | Dangerous |
pub fn classify_tool_risk(tool_name: &str, _args: &Value) -> ToolRisk {
    match normalize_tool_name(tool_name).as_str() {
        // Read-only tools
        "read" | "glob" | "grep" | "ls" | "cwd" | "codebasesearch" | "webfetch" | "websearch"
        | "analyze" => ToolRisk::ReadOnly,

        // Safe write tools
        "write" | "edit" | "notebookedit" => ToolRisk::SafeWrite,

        // Dangerous tools
        "bash" | "browser" | "task" => ToolRisk::Dangerous,

        // Unknown tools default to Dangerous for safety
        _ => ToolRisk::Dangerous,
    }
}

/// Determine whether a risk class needs user approval given the session's permission level.
pub fn needs_approval_by_level(risk: ToolRisk, level: PermissionLevel) -> bool {
    match level {
        PermissionLevel::Strict => matches!(risk, ToolRisk::SafeWrite | ToolRisk::Dangerous),
        PermissionLevel::Standard => matches!(risk, ToolRisk::Dangerous),
        PermissionLevel::Permissive => false,
    }
}

/// Evaluate Policy v2 for a single tool invocation.
///
/// Rule order is explicit and stable: `deny > prompt > allow`.
pub fn evaluate_policy(input: PolicyInput<'_>, config: &PermissionPolicyConfig) -> PolicyDecision {
    let normalized = normalize_tool_name(input.tool_name);

    // Rule 1: Path-based file rules.
    if let Some(path_info) = extract_file_path_info(
        &normalized,
        input.args,
        input.working_dir,
        input.project_root,
    ) {
        if path_info.outside_workspace {
            if normalized == "read" {
                return PolicyDecision::prompt(
                    ToolRisk::ReadOnly,
                    format!(
                        "Read outside workspace requires user approval: {}",
                        path_info.display_path
                    ),
                    format!("read:outside:{}", path_info.display_path),
                );
            }

            if matches!(normalized.as_str(), "write" | "edit" | "notebookedit") {
                return PolicyDecision::deny(
                    ToolRisk::SafeWrite,
                    format!(
                        "Write/Edit outside workspace is blocked by policy: {}",
                        path_info.display_path
                    ),
                );
            }
        }
    }

    // Rule 2: Bash network policy.
    if normalized == "bash" {
        if let Some(decision) = evaluate_bash_network_policy(input.args, config) {
            return decision;
        }
    }

    // Rule 3: Baseline mode fallback.
    let risk = classify_tool_risk(input.tool_name, input.args);
    if needs_approval_by_level(risk, input.level) {
        return PolicyDecision::prompt(
            risk,
            format!(
                "Tool '{}' requires approval in {:?} mode",
                input.tool_name, input.level
            ),
            format!("tool:{}", normalized),
        );
    }

    PolicyDecision::allow(
        risk,
        format!(
            "Tool '{}' allowed by {:?} mode policy",
            input.tool_name, input.level
        ),
    )
}

/// Response from frontend to backend for tool approval.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionResponse {
    pub request_id: String,
    /// Whether the tool execution is allowed
    pub allowed: bool,
    /// If true, auto-allow this approval scope for the remainder of the session
    pub always_allow: bool,
}

#[derive(Debug)]
struct FilePathInfo {
    outside_workspace: bool,
    display_path: String,
}

fn normalize_tool_name(tool_name: &str) -> String {
    tool_name.trim().to_ascii_lowercase()
}

fn extract_file_path_info(
    normalized_tool_name: &str,
    args: &Value,
    working_dir: &Path,
    project_root: &Path,
) -> Option<FilePathInfo> {
    if !matches!(
        normalized_tool_name,
        "read" | "write" | "edit" | "notebookedit"
    ) {
        return None;
    }

    let path_str = args.get("file_path")?.as_str()?;
    let path = resolve_path(path_str, working_dir);
    Some(FilePathInfo {
        outside_workspace: is_outside_workspace(&path, project_root),
        display_path: path.display().to_string(),
    })
}

fn resolve_path(path_str: &str, working_dir: &Path) -> PathBuf {
    let p = Path::new(path_str);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        working_dir.join(p)
    }
}

fn is_outside_workspace(path: &Path, project_root: &Path) -> bool {
    let canonical_root = match project_root.canonicalize() {
        Ok(p) => p,
        Err(_) => return true,
    };

    // Same strategy as path validation: resolve nearest existing ancestor so
    // non-existent target paths still participate in boundary checks.
    let mut check_path = path;
    while !check_path.exists() {
        match check_path.parent() {
            Some(parent) => check_path = parent,
            None => return true,
        }
    }

    match check_path.canonicalize() {
        Ok(canonical) => !canonical.starts_with(&canonical_root),
        Err(_) => true,
    }
}

fn evaluate_bash_network_policy(
    args: &Value,
    config: &PermissionPolicyConfig,
) -> Option<PolicyDecision> {
    let command = args.get("command").and_then(|v| v.as_str())?.trim();
    if command.is_empty() {
        return None;
    }

    let domains = extract_domains_from_command(command);
    let looks_network = network_command_regex().is_match(command) || !domains.is_empty();
    if !looks_network {
        return None;
    }

    if !domains.is_empty()
        && domains
            .iter()
            .all(|d| is_domain_allowlisted(d, &config.network_domain_allowlist))
    {
        return Some(PolicyDecision::allow(
            ToolRisk::Dangerous,
            format!(
                "Network command auto-allowed by domain allowlist: {}",
                domains.join(", ")
            ),
        ));
    }

    let reason = if domains.is_empty() {
        "Network command requires user approval".to_string()
    } else {
        format!(
            "Network command to domain(s) [{}] requires user approval",
            domains.join(", ")
        )
    };
    let scope_key = if domains.is_empty() {
        "bash:network:any".to_string()
    } else {
        format!("bash:network:{}", domains.join(","))
    };

    Some(PolicyDecision::prompt(
        ToolRisk::Dangerous,
        reason,
        scope_key,
    ))
}

fn extract_domains_from_command(command: &str) -> Vec<String> {
    let mut domains: Vec<String> = Vec::new();

    // URL extraction (http/https/ws/wss/ftp)
    for m in url_regex().find_iter(command) {
        if let Ok(url) = Url::parse(m.as_str()) {
            if let Some(host) = url.host_str() {
                maybe_push_host(host, &mut domains);
            }
        }
    }

    // Direct host arguments to common network commands.
    for caps in host_arg_regex().captures_iter(command) {
        if let Some(raw) = caps.get(1) {
            maybe_push_host(raw.as_str(), &mut domains);
        }
    }

    // SSH-like tokens, e.g. git@github.com:org/repo.git
    for token in command.split_whitespace() {
        if let Some(host) = extract_host_from_token(token) {
            maybe_push_host(&host, &mut domains);
        }
    }

    domains.sort();
    domains.dedup();
    domains
}

fn maybe_push_host(raw: &str, out: &mut Vec<String>) {
    if let Some(host) = normalize_host(raw) {
        out.push(host);
    }
}

fn extract_host_from_token(token: &str) -> Option<String> {
    let token = token
        .trim_matches('"')
        .trim_matches('\'')
        .trim_matches(',')
        .trim_matches(';');
    if token.is_empty() {
        return None;
    }

    // URL-like token.
    if token.contains("://") {
        if let Ok(url) = Url::parse(token) {
            return url.host_str().and_then(normalize_host);
        }
    }

    // git@github.com:org/repo.git
    if let Some((user_host, _)) = token.split_once(':') {
        if let Some((_, host)) = user_host.rsplit_once('@') {
            return normalize_host(host);
        }
    }

    None
}

fn normalize_host(raw: &str) -> Option<String> {
    let mut host = raw
        .trim()
        .trim_matches('[')
        .trim_matches(']')
        .trim_matches('"')
        .trim_matches('\'')
        .trim_end_matches('.');

    if let Some((_, rhs)) = host.rsplit_once('@') {
        host = rhs;
    }

    // Strip path suffix if any.
    if let Some((h, _)) = host.split_once('/') {
        host = h;
    }

    // Strip :port for non-IPv6 host strings.
    if host.matches(':').count() == 1 {
        if let Some((h, p)) = host.split_once(':') {
            if p.chars().all(|c| c.is_ascii_digit()) {
                host = h;
            }
        }
    }

    if host.is_empty() {
        return None;
    }
    if !host
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
    {
        return None;
    }
    Some(host.to_ascii_lowercase())
}

fn is_domain_allowlisted(domain: &str, allowlist: &[String]) -> bool {
    let domain = domain.trim().trim_end_matches('.').to_ascii_lowercase();
    if domain.is_empty() {
        return false;
    }

    allowlist
        .iter()
        .any(|entry| domain_matches_allowlist_entry(&domain, entry))
        || builtin_network_domain_allowlist()
            .iter()
            .any(|entry| domain_matches_allowlist_entry(&domain, entry))
}

fn domain_matches_allowlist_entry(domain: &str, entry: &str) -> bool {
    let allow = entry
        .trim()
        .trim_start_matches('.')
        .trim_end_matches('.')
        .to_ascii_lowercase();
    !allow.is_empty() && (domain == allow || domain.ends_with(&format!(".{}", allow)))
}

fn url_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"(?i)\b(?:https?|wss?|ftp)://[^\s"'`]+"#).expect("valid URL extraction regex")
    })
}

fn host_arg_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r#"(?i)\b(?:ssh|scp|sftp|ping|nc|ncat|telnet|dig|nslookup|traceroute)\s+([^\s"'`]+)"#,
        )
        .expect("valid host argument regex")
    })
}

fn network_command_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r#"(?i)\b(curl|wget|ssh|scp|sftp|ping|nc|ncat|telnet|dig|nslookup|traceroute|git\s+clone|git\s+fetch|git\s+pull)\b"#,
        )
        .expect("valid network command regex")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_classify_read_only_tools() {
        let args = serde_json::json!({});
        for name in &[
            "Read",
            "Glob",
            "Grep",
            "LS",
            "Cwd",
            "CodebaseSearch",
            "WebFetch",
            "WebSearch",
            "Analyze",
        ] {
            assert_eq!(
                classify_tool_risk(name, &args),
                ToolRisk::ReadOnly,
                "{} should be ReadOnly",
                name
            );
        }
    }

    #[test]
    fn test_classify_safe_write_tools() {
        let args = serde_json::json!({});
        for name in &["Write", "Edit", "NotebookEdit"] {
            assert_eq!(
                classify_tool_risk(name, &args),
                ToolRisk::SafeWrite,
                "{} should be SafeWrite",
                name
            );
        }
    }

    #[test]
    fn test_classify_dangerous_tools() {
        let args = serde_json::json!({});
        for name in &["Bash", "Browser", "Task"] {
            assert_eq!(
                classify_tool_risk(name, &args),
                ToolRisk::Dangerous,
                "{} should be Dangerous",
                name
            );
        }
    }

    #[test]
    fn test_classify_unknown_tool_defaults_to_dangerous() {
        let args = serde_json::json!({});
        assert_eq!(
            classify_tool_risk("UnknownTool", &args),
            ToolRisk::Dangerous
        );
    }

    #[test]
    fn test_needs_approval_strict() {
        let args = serde_json::json!({});
        // ReadOnly: no approval
        assert!(!needs_approval_by_level(
            classify_tool_risk("Read", &args),
            PermissionLevel::Strict
        ));
        assert!(!needs_approval_by_level(
            classify_tool_risk("Grep", &args),
            PermissionLevel::Strict
        ));
        // SafeWrite: approval needed
        assert!(needs_approval_by_level(
            classify_tool_risk("Write", &args),
            PermissionLevel::Strict
        ));
        assert!(needs_approval_by_level(
            classify_tool_risk("Edit", &args),
            PermissionLevel::Strict
        ));
        // Dangerous: approval needed
        assert!(needs_approval_by_level(
            classify_tool_risk("Bash", &args),
            PermissionLevel::Strict
        ));
        assert!(needs_approval_by_level(
            classify_tool_risk("Task", &args),
            PermissionLevel::Strict
        ));
    }

    #[test]
    fn test_needs_approval_standard() {
        let args = serde_json::json!({});
        // ReadOnly: no approval
        assert!(!needs_approval_by_level(
            classify_tool_risk("Read", &args),
            PermissionLevel::Standard
        ));
        // SafeWrite: no approval in Standard
        assert!(!needs_approval_by_level(
            classify_tool_risk("Write", &args),
            PermissionLevel::Standard
        ));
        assert!(!needs_approval_by_level(
            classify_tool_risk("Edit", &args),
            PermissionLevel::Standard
        ));
        // Dangerous: approval needed
        assert!(needs_approval_by_level(
            classify_tool_risk("Bash", &args),
            PermissionLevel::Standard
        ));
        assert!(needs_approval_by_level(
            classify_tool_risk("Browser", &args),
            PermissionLevel::Standard
        ));
        assert!(needs_approval_by_level(
            classify_tool_risk("Task", &args),
            PermissionLevel::Standard
        ));
    }

    #[test]
    fn test_needs_approval_permissive() {
        let args = serde_json::json!({});
        // Nothing needs approval in Permissive by level-only fallback
        assert!(!needs_approval_by_level(
            classify_tool_risk("Read", &args),
            PermissionLevel::Permissive
        ));
        assert!(!needs_approval_by_level(
            classify_tool_risk("Write", &args),
            PermissionLevel::Permissive
        ));
        assert!(!needs_approval_by_level(
            classify_tool_risk("Bash", &args),
            PermissionLevel::Permissive
        ));
        assert!(!needs_approval_by_level(
            classify_tool_risk("Browser", &args),
            PermissionLevel::Permissive
        ));
        assert!(!needs_approval_by_level(
            classify_tool_risk("Task", &args),
            PermissionLevel::Permissive
        ));
    }

    #[test]
    fn test_policy_v2_read_outside_workspace_prompts() {
        let workspace = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        let input = PolicyInput {
            tool_name: "Read",
            args: &serde_json::json!({
                "file_path": outside.path().join("a.txt").to_string_lossy().to_string()
            }),
            level: PermissionLevel::Permissive,
            working_dir: workspace.path(),
            project_root: workspace.path(),
        };
        let decision = evaluate_policy(input, &PermissionPolicyConfig::default());
        assert_eq!(decision.action, PolicyAction::Prompt);
        assert_eq!(decision.risk, ToolRisk::ReadOnly);
        assert!(decision.reason.contains("outside workspace"));
    }

    #[test]
    fn test_policy_v2_write_outside_workspace_denied() {
        let workspace = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        let input = PolicyInput {
            tool_name: "Write",
            args: &serde_json::json!({
                "file_path": outside.path().join("a.txt").to_string_lossy().to_string(),
                "content": "x"
            }),
            level: PermissionLevel::Permissive,
            working_dir: workspace.path(),
            project_root: workspace.path(),
        };
        let decision = evaluate_policy(input, &PermissionPolicyConfig::default());
        assert_eq!(decision.action, PolicyAction::Deny);
        assert_eq!(decision.risk, ToolRisk::SafeWrite);
    }

    #[test]
    fn test_policy_v2_bash_network_allowlisted_domain_allows() {
        let workspace = TempDir::new().unwrap();
        let input = PolicyInput {
            tool_name: "Bash",
            args: &serde_json::json!({
                "command": "curl https://api.github.com/repos"
            }),
            level: PermissionLevel::Permissive,
            working_dir: workspace.path(),
            project_root: workspace.path(),
        };
        let config = PermissionPolicyConfig {
            network_domain_allowlist: vec!["github.com".to_string()],
        };
        let decision = evaluate_policy(input, &config);
        assert_eq!(decision.action, PolicyAction::Allow);
        assert_eq!(decision.risk, ToolRisk::Dangerous);
    }

    #[test]
    fn test_policy_v2_bash_network_builtin_allowlist_allows_default_config() {
        let workspace = TempDir::new().unwrap();
        let input = PolicyInput {
            tool_name: "Bash",
            args: &serde_json::json!({
                "command": "curl https://github.com/anthropics/claude-code"
            }),
            level: PermissionLevel::Permissive,
            working_dir: workspace.path(),
            project_root: workspace.path(),
        };
        let decision = evaluate_policy(input, &PermissionPolicyConfig::default());
        assert_eq!(decision.action, PolicyAction::Allow);
        assert_eq!(decision.risk, ToolRisk::Dangerous);
    }

    #[test]
    fn test_builtin_allowlist_config_is_versioned() {
        let cfg = builtin_network_domain_allowlist_config();
        assert_eq!(cfg.version, builtin_network_domain_allowlist_version());
        assert!(!cfg.version.trim().is_empty());
        assert!(!cfg.domains.is_empty());
    }

    #[test]
    fn test_builtin_allowlist_current_version_is_listed() {
        let available = builtin_network_domain_allowlist_available_versions();
        let current = builtin_network_domain_allowlist_version();
        assert!(available.iter().any(|v| *v == current));
        assert!(builtin_network_domain_allowlist_config_by_version(current).is_some());
    }

    #[test]
    fn test_policy_v2_bash_network_non_allowlisted_prompts() {
        let workspace = TempDir::new().unwrap();
        let input = PolicyInput {
            tool_name: "Bash",
            args: &serde_json::json!({
                "command": "curl https://example.com/data"
            }),
            level: PermissionLevel::Permissive,
            working_dir: workspace.path(),
            project_root: workspace.path(),
        };
        let decision = evaluate_policy(input, &PermissionPolicyConfig::default());
        assert_eq!(decision.action, PolicyAction::Prompt);
        assert_eq!(decision.risk, ToolRisk::Dangerous);
        assert!(decision.approval_scope_key.starts_with("bash:network:"));
    }

    #[test]
    fn test_default_permission_level_is_strict() {
        assert_eq!(PermissionLevel::default(), PermissionLevel::Strict);
    }

    #[test]
    fn test_tool_risk_as_str() {
        assert_eq!(ToolRisk::ReadOnly.as_str(), "ReadOnly");
        assert_eq!(ToolRisk::SafeWrite.as_str(), "SafeWrite");
        assert_eq!(ToolRisk::Dangerous.as_str(), "Dangerous");
    }
}
