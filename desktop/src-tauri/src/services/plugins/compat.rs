//! Plugin Compatibility Evaluator
//!
//! Produces a machine-readable compatibility report for Claude Code plugin
//! semantics and highlights degraded/partial capabilities.

use crate::services::plugins::models::{
    CompatCapability, CompatGap, HookEvent, LoadedPlugin, PluginCompatLevel, PluginCompatReport,
};

/// Whether a hook event is mapped in Desktop runtime.
pub fn is_hook_event_supported(event: &HookEvent) -> bool {
    matches!(
        event,
        HookEvent::SessionStart
            | HookEvent::UserPromptSubmit
            | HookEvent::PreCompact
            | HookEvent::PostCompact
            | HookEvent::PreToolUse
            | HookEvent::PostToolUse
            | HookEvent::Stop
            | HookEvent::PreLlmCall
            | HookEvent::PostLlmCall
            | HookEvent::SessionEnd
            | HookEvent::QualityGateRegistration
    )
}

fn build_capability(name: &str, supported: bool, details: Option<String>) -> CompatCapability {
    CompatCapability {
        name: name.to_string(),
        supported,
        details,
    }
}

/// Evaluate one loaded plugin and return a compatibility report.
pub fn evaluate_plugin_compat(plugin: &LoadedPlugin) -> PluginCompatReport {
    let mut capabilities = Vec::new();
    let mut gaps = Vec::new();

    let has_skills = !plugin.skills.is_empty();
    let has_commands = !plugin.commands.is_empty();
    let has_hooks = !plugin.hooks.is_empty();
    let has_instructions = plugin
        .instructions
        .as_ref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    let has_invocable_skill = plugin.skills.iter().any(|s| s.user_invocable);

    capabilities.push(build_capability("manifest", true, None));
    capabilities.push(build_capability(
        "skills",
        true,
        Some(has_skills.to_string()),
    ));
    capabilities.push(build_capability(
        "commands",
        true,
        Some(has_commands.to_string()),
    ));
    capabilities.push(build_capability("hooks", true, Some(has_hooks.to_string())));
    capabilities.push(build_capability(
        "instructions",
        true,
        Some(has_instructions.to_string()),
    ));
    capabilities.push(build_capability(
        "invocable_skills",
        true,
        Some(has_invocable_skill.to_string()),
    ));
    capabilities.push(build_capability(
        "permissions_allow_deny",
        true,
        Some(format!(
            "allow={}, deny={}",
            plugin.permissions.allow.len(),
            plugin.permissions.deny.len()
        )),
    ));

    // always_approve is parsed and merged, but not used by the runtime gate yet.
    let always_approve_supported = plugin.permissions.always_approve.is_empty();
    capabilities.push(build_capability(
        "permissions_always_approve",
        always_approve_supported,
        Some(format!(
            "always_approve={}",
            plugin.permissions.always_approve.len()
        )),
    ));
    if !always_approve_supported {
        gaps.push(CompatGap {
            capability: "permissions_always_approve".to_string(),
            reason: "always_approve is parsed but not fully enforced by permission gate"
                .to_string(),
            severity: "medium".to_string(),
        });
    }

    for hook in &plugin.hooks {
        if !is_hook_event_supported(&hook.event) {
            gaps.push(CompatGap {
                capability: format!("hook_event:{:?}", hook.event),
                reason: "hook event not mapped in Desktop runtime".to_string(),
                severity: "high".to_string(),
            });
        }
    }

    // Detect unsupported "agents/" plugin capability.
    let has_agents_dir = std::path::Path::new(&plugin.root_path)
        .join("agents")
        .is_dir();
    capabilities.push(build_capability(
        "agents_directory",
        !has_agents_dir,
        Some(has_agents_dir.to_string()),
    ));
    if has_agents_dir {
        gaps.push(CompatGap {
            capability: "agents_directory".to_string(),
            reason:
                "plugin agents directory is discovered but not executed as first-class capability"
                    .to_string(),
            severity: "low".to_string(),
        });
    }

    // Validate quality-gate registration payload shape when present.
    for hook in &plugin.hooks {
        if hook.event == HookEvent::QualityGateRegistration
            && serde_json::from_str::<crate::services::plugins::models::PluginQualityGate>(
                &hook.command,
            )
            .is_err()
        {
            gaps.push(CompatGap {
                capability: "quality_gate_registration".to_string(),
                reason: "QualityGateRegistration command is not valid PluginQualityGate JSON"
                    .to_string(),
                severity: "high".to_string(),
            });
        }
    }

    let prompt_budget_impact = plugin
        .instructions
        .as_ref()
        .map(|s| s.len())
        .unwrap_or_default()
        + plugin.skills.iter().map(|s| s.body.len()).sum::<usize>()
        + plugin.commands.iter().map(|c| c.body.len()).sum::<usize>();

    let has_high_gap = gaps.iter().any(|g| g.severity == "high");
    let level = if has_high_gap {
        PluginCompatLevel::Degraded
    } else if gaps.is_empty() {
        PluginCompatLevel::Full
    } else {
        PluginCompatLevel::Partial
    };
    let summary = match level {
        PluginCompatLevel::Full => "All detected plugin capabilities are supported".to_string(),
        PluginCompatLevel::Partial => format!("{} compatibility gaps detected", gaps.len()),
        PluginCompatLevel::Degraded => {
            format!("{} high-impact compatibility gaps detected", gaps.len())
        }
    };

    PluginCompatReport {
        plugin_name: plugin.manifest.name.clone(),
        level,
        summary,
        capabilities,
        gaps,
        checked_at: chrono::Utc::now().timestamp(),
        prompt_budget_impact,
        injection_truncated: false,
    }
}
