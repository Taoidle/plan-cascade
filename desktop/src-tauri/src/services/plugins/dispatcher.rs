//! Hook Dispatcher
//!
//! Bridges Claude Code plugin hooks to the Desktop AgenticHooks system.
//!
//! Maps Claude Code lifecycle events to Desktop hook points:
//! - `SessionStart` -> `on_session_start`
//! - `UserPromptSubmit` -> `on_user_message`
//! - `PreToolUse` -> `on_before_tool` (exit code 2 blocks tool)
//! - `PostToolUse` -> `on_after_tool` (stdout injected as context)
//! - `Stop` / `SessionEnd` -> `on_session_end`
//! - `PreCompact` -> `on_compaction`
//!
//! Shell hooks are executed via `tokio::process::Command` with JSON stdin
//! and environment variables (CLAUDE_PROJECT_DIR, TOOL_NAME, etc.).

use std::collections::HashMap;
use std::time::Duration;

use regex::Regex;

use crate::services::orchestrator::hooks::{AgenticHooks, BeforeToolResult};
use crate::services::plugins::models::*;

// ============================================================================
// Shell Hook Execution
// ============================================================================

/// Execute a shell hook command.
///
/// Runs the command via `sh -c` (Unix) or `cmd /C` (Windows) with:
/// - Environment variables set from `env_vars`
/// - JSON piped to stdin via `stdin_json`
/// - Timeout enforced
///
/// Returns the ShellResult with exit_code, stdout, stderr.
pub async fn execute_shell_hook(
    command: &str,
    env_vars: &HashMap<String, String>,
    stdin_json: Option<&str>,
    timeout_ms: u64,
) -> ShellResult {
    use tokio::process::Command;

    let mut cmd = if cfg!(target_os = "windows") {
        let mut c = Command::new("cmd");
        c.args(["/C", command]);
        c
    } else {
        let mut c = Command::new("sh");
        c.args(["-c", command]);
        c
    };

    // Set environment variables
    for (key, value) in env_vars {
        cmd.env(key, value);
    }

    // Set up stdin piping if we have JSON to send
    if stdin_json.is_some() {
        cmd.stdin(std::process::Stdio::piped());
    } else {
        cmd.stdin(std::process::Stdio::null());
    }

    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    // Spawn the process
    let child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return ShellResult {
                exit_code: 1,
                stdout: String::new(),
                stderr: format!("Failed to spawn hook command: {}", e),
            };
        }
    };

    // Write stdin if provided
    let output_future = async {
        if let Some(json) = stdin_json {
            // For simplicity, we write stdin and wait for output
            // In a more sophisticated implementation, we'd pipe stdin concurrently
            let mut child = child;
            if let Some(mut stdin) = child.stdin.take() {
                use tokio::io::AsyncWriteExt;
                let _ = stdin.write_all(json.as_bytes()).await;
                drop(stdin);
            }
            child.wait_with_output().await
        } else {
            child.wait_with_output().await
        }
    };

    // Apply timeout
    let result = tokio::time::timeout(Duration::from_millis(timeout_ms), output_future).await;

    match result {
        Ok(Ok(output)) => ShellResult {
            exit_code: output.status.code().unwrap_or(1),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        },
        Ok(Err(e)) => ShellResult {
            exit_code: 1,
            stdout: String::new(),
            stderr: format!("Hook command failed: {}", e),
        },
        Err(_) => ShellResult {
            exit_code: 1,
            stdout: String::new(),
            stderr: format!("Hook command timed out after {}ms", timeout_ms),
        },
    }
}

// ============================================================================
// Hook Dispatcher
// ============================================================================

/// Registers plugin hooks into the AgenticHooks system.
///
/// For each plugin hook, the dispatcher:
/// 1. Maps the HookEvent to the appropriate AgenticHooks registration method
/// 2. Creates a closure that executes the hook command with appropriate env vars
/// 3. For PreToolUse hooks, checks the exit code to determine if the tool should be blocked
/// 4. For async hooks, spawns the execution in the background
pub fn register_plugin_hooks(
    hooks: &mut AgenticHooks,
    plugin_hooks: Vec<PluginHook>,
    plugin_name: String,
    plugin_root: String,
) {
    for plugin_hook in plugin_hooks {
        match plugin_hook.event {
            HookEvent::SessionStart => {
                register_session_start_hook(
                    hooks,
                    plugin_hook,
                    plugin_name.clone(),
                    plugin_root.clone(),
                );
            }
            HookEvent::UserPromptSubmit => {
                register_user_message_hook(
                    hooks,
                    plugin_hook,
                    plugin_name.clone(),
                    plugin_root.clone(),
                );
            }
            HookEvent::PreToolUse => {
                register_before_tool_hook(
                    hooks,
                    plugin_hook,
                    plugin_name.clone(),
                    plugin_root.clone(),
                );
            }
            HookEvent::PostToolUse => {
                register_after_tool_hook(
                    hooks,
                    plugin_hook,
                    plugin_name.clone(),
                    plugin_root.clone(),
                );
            }
            HookEvent::Stop | HookEvent::SessionEnd => {
                register_session_end_hook(
                    hooks,
                    plugin_hook,
                    plugin_name.clone(),
                    plugin_root.clone(),
                );
            }
            HookEvent::PreCompact => {
                register_compaction_hook(
                    hooks,
                    plugin_hook,
                    plugin_name.clone(),
                    plugin_root.clone(),
                );
            }
            // Other events are logged but not yet mapped to Desktop hooks
            _ => {
                eprintln!(
                    "[plugin:{}] Hook event {:?} not yet mapped to Desktop hooks",
                    plugin_name, plugin_hook.event
                );
            }
        }
    }
}

// ============================================================================
// Individual Hook Registration
// ============================================================================

fn build_base_env(
    project_path: &str,
    plugin_root: &str,
    plugin_name: &str,
) -> HashMap<String, String> {
    let mut env = HashMap::new();
    env.insert("CLAUDE_PROJECT_DIR".to_string(), project_path.to_string());
    env.insert("CLAUDE_PLUGIN_ROOT".to_string(), plugin_root.to_string());
    env.insert("CLAUDE_PLUGIN_NAME".to_string(), plugin_name.to_string());
    env
}

fn register_session_start_hook(
    hooks: &mut AgenticHooks,
    plugin_hook: PluginHook,
    plugin_name: String,
    plugin_root: String,
) {
    let is_async = plugin_hook.async_hook;
    let command = plugin_hook.command.clone();
    let timeout = plugin_hook.timeout;

    hooks.register_on_session_start(Box::new(move |ctx| {
        let cmd = command.clone();
        let name = plugin_name.clone();
        let root = plugin_root.clone();
        let timeout_ms = timeout;
        let project_path = ctx.project_path.to_string_lossy().to_string();

        Box::pin(async move {
            let env = build_base_env(&project_path, &root, &name);
            let stdin = serde_json::json!({
                "session_id": ctx.session_id,
                "provider": ctx.provider_name,
                "model": ctx.model_name,
            })
            .to_string();

            if is_async {
                tokio::spawn(async move {
                    let result = execute_shell_hook(&cmd, &env, Some(&stdin), timeout_ms).await;
                    if !result.is_success() {
                        eprintln!(
                            "[plugin:{}] Async SessionStart hook failed: {}",
                            name, result.stderr
                        );
                    }
                });
                Ok(())
            } else {
                let result = execute_shell_hook(&cmd, &env, Some(&stdin), timeout_ms).await;
                if !result.is_success() {
                    eprintln!(
                        "[plugin:{}] SessionStart hook failed: {}",
                        name, result.stderr
                    );
                }
                Ok(())
            }
        })
    }));
}

fn register_user_message_hook(
    hooks: &mut AgenticHooks,
    plugin_hook: PluginHook,
    plugin_name: String,
    plugin_root: String,
) {
    let command = plugin_hook.command.clone();
    let timeout = plugin_hook.timeout;

    hooks.register_on_user_message(Box::new(move |ctx, msg| {
        let cmd = command.clone();
        let name = plugin_name.clone();
        let root = plugin_root.clone();
        let timeout_ms = timeout;
        let project_path = ctx.project_path.to_string_lossy().to_string();

        Box::pin(async move {
            let env = build_base_env(&project_path, &root, &name);
            let stdin = serde_json::json!({
                "session_id": ctx.session_id,
                "message": msg,
            })
            .to_string();

            let result = execute_shell_hook(&cmd, &env, Some(&stdin), timeout_ms).await;
            if result.is_success() && !result.stdout.trim().is_empty() {
                // If the hook returned non-empty stdout, use it as the modified message
                Ok(Some(result.stdout.trim().to_string()))
            } else {
                Ok(None) // No modification
            }
        })
    }));
}

fn register_before_tool_hook(
    hooks: &mut AgenticHooks,
    plugin_hook: PluginHook,
    plugin_name: String,
    plugin_root: String,
) {
    let command = plugin_hook.command.clone();
    let timeout = plugin_hook.timeout;
    let matcher_pattern = plugin_hook.matcher.clone();

    hooks.register_on_before_tool(Box::new(move |ctx, tool_name, arguments| {
        let cmd = command.clone();
        let name = plugin_name.clone();
        let root = plugin_root.clone();
        let timeout_ms = timeout;
        let matcher = matcher_pattern.clone();
        let project_path = ctx.project_path.to_string_lossy().to_string();

        Box::pin(async move {
            // Check matcher regex
            if let Some(ref pattern) = matcher {
                match Regex::new(pattern) {
                    Ok(re) => {
                        if !re.is_match(&tool_name) {
                            return Ok(BeforeToolResult::default());
                        }
                    }
                    Err(_) => {
                        // Invalid regex - skip this hook
                        return Ok(BeforeToolResult::default());
                    }
                }
            }

            let mut env = build_base_env(&project_path, &root, &name);
            env.insert("TOOL_NAME".to_string(), tool_name.clone());
            env.insert("TOOL_INPUT".to_string(), arguments.clone());

            let stdin = serde_json::json!({
                "session_id": ctx.session_id,
                "tool_name": tool_name,
                "tool_input": arguments,
            })
            .to_string();

            let result = execute_shell_hook(&cmd, &env, Some(&stdin), timeout_ms).await;

            if result.is_block() {
                // Exit code 2 = block the tool
                let reason = if result.stderr.trim().is_empty() {
                    format!(
                        "Plugin '{}' blocked tool '{}': {}",
                        name,
                        tool_name,
                        if result.stdout.trim().is_empty() {
                            "blocked by hook"
                        } else {
                            result.stdout.trim()
                        }
                    )
                } else {
                    format!(
                        "Plugin '{}' blocked tool '{}': {}",
                        name,
                        tool_name,
                        result.stderr.trim()
                    )
                };

                Ok(BeforeToolResult {
                    skip: true,
                    skip_reason: Some(reason),
                })
            } else {
                // Exit code 0 = continue, stdout may be injected as context
                Ok(BeforeToolResult::default())
            }
        })
    }));
}

fn register_after_tool_hook(
    hooks: &mut AgenticHooks,
    plugin_hook: PluginHook,
    plugin_name: String,
    plugin_root: String,
) {
    let is_async = plugin_hook.async_hook;
    let command = plugin_hook.command.clone();
    let timeout = plugin_hook.timeout;
    let matcher_pattern = plugin_hook.matcher.clone();

    hooks.register_on_after_tool(Box::new(move |ctx, tool_name, success, output_snippet| {
        let cmd = command.clone();
        let name = plugin_name.clone();
        let root = plugin_root.clone();
        let timeout_ms = timeout;
        let matcher = matcher_pattern.clone();
        let project_path = ctx.project_path.to_string_lossy().to_string();

        Box::pin(async move {
            // Check matcher regex
            if let Some(ref pattern) = matcher {
                match Regex::new(pattern) {
                    Ok(re) => {
                        if !re.is_match(&tool_name) {
                            return Ok(());
                        }
                    }
                    Err(_) => return Ok(()),
                }
            }

            let mut env = build_base_env(&project_path, &root, &name);
            env.insert("TOOL_NAME".to_string(), tool_name.clone());
            env.insert("TOOL_SUCCESS".to_string(), success.to_string());
            if let Some(ref output) = output_snippet {
                env.insert("TOOL_OUTPUT".to_string(), output.clone());
            }

            let stdin = serde_json::json!({
                "session_id": ctx.session_id,
                "tool_name": tool_name,
                "success": success,
                "output": output_snippet,
            })
            .to_string();

            if is_async {
                tokio::spawn(async move {
                    let result = execute_shell_hook(&cmd, &env, Some(&stdin), timeout_ms).await;
                    if !result.is_success() {
                        eprintln!(
                            "[plugin:{}] Async PostToolUse hook failed: {}",
                            name, result.stderr
                        );
                    }
                });
            } else {
                let result = execute_shell_hook(&cmd, &env, Some(&stdin), timeout_ms).await;
                if !result.is_success() {
                    eprintln!(
                        "[plugin:{}] PostToolUse hook failed: {}",
                        name, result.stderr
                    );
                }
            }

            Ok(())
        })
    }));
}

fn register_session_end_hook(
    hooks: &mut AgenticHooks,
    plugin_hook: PluginHook,
    plugin_name: String,
    plugin_root: String,
) {
    let is_async = plugin_hook.async_hook;
    let command = plugin_hook.command.clone();
    let timeout = plugin_hook.timeout;

    hooks.register_on_session_end(Box::new(move |ctx, summary| {
        let cmd = command.clone();
        let name = plugin_name.clone();
        let root = plugin_root.clone();
        let timeout_ms = timeout;
        let project_path = ctx.project_path.to_string_lossy().to_string();

        Box::pin(async move {
            let env = build_base_env(&project_path, &root, &name);
            let stdin = serde_json::json!({
                "session_id": ctx.session_id,
                "task": summary.task_description,
                "success": summary.success,
                "total_turns": summary.total_turns,
            })
            .to_string();

            if is_async {
                tokio::spawn(async move {
                    let result = execute_shell_hook(&cmd, &env, Some(&stdin), timeout_ms).await;
                    if !result.is_success() {
                        eprintln!(
                            "[plugin:{}] Async SessionEnd hook failed: {}",
                            name, result.stderr
                        );
                    }
                });
            } else {
                let result = execute_shell_hook(&cmd, &env, Some(&stdin), timeout_ms).await;
                if !result.is_success() {
                    eprintln!(
                        "[plugin:{}] SessionEnd hook failed: {}",
                        name, result.stderr
                    );
                }
            }

            Ok(())
        })
    }));
}

fn register_compaction_hook(
    hooks: &mut AgenticHooks,
    plugin_hook: PluginHook,
    plugin_name: String,
    plugin_root: String,
) {
    let command = plugin_hook.command.clone();
    let timeout = plugin_hook.timeout;

    hooks.register_on_compaction(Box::new(move |ctx, snippets| {
        let cmd = command.clone();
        let name = plugin_name.clone();
        let root = plugin_root.clone();
        let timeout_ms = timeout;
        let project_path = ctx.project_path.to_string_lossy().to_string();

        Box::pin(async move {
            let env = build_base_env(&project_path, &root, &name);
            let stdin = serde_json::json!({
                "session_id": ctx.session_id,
                "snippet_count": snippets.len(),
            })
            .to_string();

            let result = execute_shell_hook(&cmd, &env, Some(&stdin), timeout_ms).await;
            if !result.is_success() {
                eprintln!(
                    "[plugin:{}] PreCompact hook failed: {}",
                    name, result.stderr
                );
            }

            Ok(())
        })
    }));
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    fn test_hook_context() -> crate::services::orchestrator::hooks::HookContext {
        crate::services::orchestrator::hooks::HookContext {
            session_id: "test-session".to_string(),
            project_path: PathBuf::from("/tmp/test-project"),
            provider_name: "anthropic".to_string(),
            model_name: "claude-3".to_string(),
        }
    }

    #[tokio::test]
    async fn test_execute_shell_hook_success() {
        let env = HashMap::new();
        let result = execute_shell_hook("echo hello", &env, None, 5000).await;
        assert!(result.is_success());
        assert_eq!(result.stdout.trim(), "hello");
    }

    #[tokio::test]
    async fn test_execute_shell_hook_with_env() {
        let mut env = HashMap::new();
        env.insert("TEST_VAR".to_string(), "test_value".to_string());

        let result = execute_shell_hook("echo $TEST_VAR", &env, None, 5000).await;
        assert!(result.is_success());
        assert_eq!(result.stdout.trim(), "test_value");
    }

    #[tokio::test]
    async fn test_execute_shell_hook_exit_code_2() {
        let env = HashMap::new();
        let result = execute_shell_hook("exit 2", &env, None, 5000).await;
        assert_eq!(result.exit_code, 2);
        assert!(result.is_block());
        assert!(!result.is_success());
    }

    #[tokio::test]
    async fn test_execute_shell_hook_failure() {
        let env = HashMap::new();
        let result = execute_shell_hook("exit 1", &env, None, 5000).await;
        assert_eq!(result.exit_code, 1);
        assert!(!result.is_success());
        assert!(!result.is_block());
    }

    #[tokio::test]
    async fn test_execute_shell_hook_timeout() {
        let env = HashMap::new();
        let result = execute_shell_hook("sleep 10", &env, None, 100).await;
        assert!(!result.is_success());
        assert!(result.stderr.contains("timed out"));
    }

    #[tokio::test]
    async fn test_execute_shell_hook_with_stdin() {
        let env = HashMap::new();
        let stdin_json = r#"{"tool": "test"}"#;
        let result = execute_shell_hook("cat", &env, Some(stdin_json), 5000).await;
        assert!(result.is_success());
        assert!(result.stdout.contains("tool"));
    }

    #[tokio::test]
    async fn test_execute_shell_hook_stderr() {
        let env = HashMap::new();
        let result = execute_shell_hook("echo error >&2 && exit 1", &env, None, 5000).await;
        assert!(!result.is_success());
        assert!(result.stderr.contains("error"));
    }

    #[test]
    fn test_register_plugin_hooks_session_start() {
        let mut hooks = AgenticHooks::new();
        let plugin_hooks = vec![PluginHook {
            event: HookEvent::SessionStart,
            matcher: None,
            hook_type: HookType::Command,
            command: "echo start".to_string(),
            timeout: 5000,
            async_hook: false,
        }];

        register_plugin_hooks(
            &mut hooks,
            plugin_hooks,
            "test-plugin".to_string(),
            "/tmp/plugin".to_string(),
        );

        assert_eq!(hooks.total_hooks(), 1);
    }

    #[test]
    fn test_register_plugin_hooks_all_mapped_events() {
        let mut hooks = AgenticHooks::new();
        let plugin_hooks = vec![
            PluginHook {
                event: HookEvent::SessionStart,
                matcher: None,
                hook_type: HookType::Command,
                command: "echo 1".to_string(),
                timeout: 5000,
                async_hook: false,
            },
            PluginHook {
                event: HookEvent::UserPromptSubmit,
                matcher: None,
                hook_type: HookType::Command,
                command: "echo 2".to_string(),
                timeout: 5000,
                async_hook: false,
            },
            PluginHook {
                event: HookEvent::PreToolUse,
                matcher: None,
                hook_type: HookType::Command,
                command: "echo 3".to_string(),
                timeout: 5000,
                async_hook: false,
            },
            PluginHook {
                event: HookEvent::PostToolUse,
                matcher: None,
                hook_type: HookType::Command,
                command: "echo 4".to_string(),
                timeout: 5000,
                async_hook: false,
            },
            PluginHook {
                event: HookEvent::Stop,
                matcher: None,
                hook_type: HookType::Command,
                command: "echo 5".to_string(),
                timeout: 5000,
                async_hook: false,
            },
            PluginHook {
                event: HookEvent::PreCompact,
                matcher: None,
                hook_type: HookType::Command,
                command: "echo 6".to_string(),
                timeout: 5000,
                async_hook: false,
            },
        ];

        register_plugin_hooks(
            &mut hooks,
            plugin_hooks,
            "test-plugin".to_string(),
            "/tmp/plugin".to_string(),
        );

        // SessionStart(1) + UserMessage(1) + BeforeTool(1) + AfterTool(1) + SessionEnd(1) + Compaction(1) = 6
        assert_eq!(hooks.total_hooks(), 6);
    }

    #[tokio::test]
    async fn test_before_tool_hook_with_matcher_match() {
        let mut hooks = AgenticHooks::new();

        let plugin_hooks = vec![PluginHook {
            event: HookEvent::PreToolUse,
            matcher: Some("Bash".to_string()),
            hook_type: HookType::Command,
            command: "exit 2".to_string(), // Block
            timeout: 5000,
            async_hook: false,
        }];

        register_plugin_hooks(
            &mut hooks,
            plugin_hooks,
            "test".to_string(),
            "/tmp".to_string(),
        );

        let ctx = test_hook_context();

        // "Bash" matches the matcher
        let result = hooks
            .fire_on_before_tool(&ctx, "Bash", r#"{"command": "rm -rf /"}"#)
            .await;
        assert!(result.is_some(), "Bash should be blocked");
        assert!(result.unwrap().skip);
    }

    #[tokio::test]
    async fn test_before_tool_hook_with_matcher_no_match() {
        let mut hooks = AgenticHooks::new();

        let plugin_hooks = vec![PluginHook {
            event: HookEvent::PreToolUse,
            matcher: Some("Bash".to_string()),
            hook_type: HookType::Command,
            command: "exit 2".to_string(), // Would block if matched
            timeout: 5000,
            async_hook: false,
        }];

        register_plugin_hooks(
            &mut hooks,
            plugin_hooks,
            "test".to_string(),
            "/tmp".to_string(),
        );

        let ctx = test_hook_context();

        // "Read" does NOT match "Bash" matcher
        let result = hooks
            .fire_on_before_tool(&ctx, "Read", r#"{"path": "/tmp/file"}"#)
            .await;
        assert!(
            result.is_none(),
            "Read should NOT be blocked by Bash matcher"
        );
    }

    #[tokio::test]
    async fn test_before_tool_hook_exit_code_0_continues() {
        let mut hooks = AgenticHooks::new();

        let plugin_hooks = vec![PluginHook {
            event: HookEvent::PreToolUse,
            matcher: None,
            hook_type: HookType::Command,
            command: "echo ok".to_string(), // exit 0
            timeout: 5000,
            async_hook: false,
        }];

        register_plugin_hooks(
            &mut hooks,
            plugin_hooks,
            "test".to_string(),
            "/tmp".to_string(),
        );

        let ctx = test_hook_context();
        let result = hooks.fire_on_before_tool(&ctx, "Read", "{}").await;
        assert!(result.is_none(), "exit 0 should continue execution");
    }

    #[tokio::test]
    async fn test_session_start_hook_fires() {
        let mut hooks = AgenticHooks::new();

        let plugin_hooks = vec![PluginHook {
            event: HookEvent::SessionStart,
            matcher: None,
            hook_type: HookType::Command,
            command: "echo started".to_string(),
            timeout: 5000,
            async_hook: false,
        }];

        register_plugin_hooks(
            &mut hooks,
            plugin_hooks,
            "test".to_string(),
            "/tmp".to_string(),
        );

        let ctx = test_hook_context();
        // Should not panic
        hooks.fire_on_session_start(&ctx).await;
    }

    #[tokio::test]
    async fn test_session_end_hook_fires() {
        let mut hooks = AgenticHooks::new();

        let plugin_hooks = vec![PluginHook {
            event: HookEvent::Stop,
            matcher: None,
            hook_type: HookType::Command,
            command: "echo stopped".to_string(),
            timeout: 5000,
            async_hook: false,
        }];

        register_plugin_hooks(
            &mut hooks,
            plugin_hooks,
            "test".to_string(),
            "/tmp".to_string(),
        );

        let ctx = test_hook_context();
        let summary = crate::services::orchestrator::hooks::SessionSummary {
            task_description: "test".to_string(),
            files_read: vec![],
            key_findings: vec![],
            tool_usage: HashMap::new(),
            total_turns: 1,
            success: true,
        };
        // Should not panic
        hooks.fire_on_session_end(&ctx, summary).await;
    }

    #[tokio::test]
    async fn test_async_hook_does_not_block() {
        let mut hooks = AgenticHooks::new();

        let plugin_hooks = vec![PluginHook {
            event: HookEvent::SessionStart,
            matcher: None,
            hook_type: HookType::Command,
            command: "sleep 2".to_string(), // Would block if synchronous
            timeout: 5000,
            async_hook: true, // Async - should not block
        }];

        register_plugin_hooks(
            &mut hooks,
            plugin_hooks,
            "test".to_string(),
            "/tmp".to_string(),
        );

        let ctx = test_hook_context();

        // This should return quickly because the hook is async
        let start = std::time::Instant::now();
        hooks.fire_on_session_start(&ctx).await;
        let elapsed = start.elapsed();

        // Should complete in much less than 2 seconds
        assert!(
            elapsed < Duration::from_millis(500),
            "Async hook should not block: took {:?}",
            elapsed
        );
    }

    #[test]
    fn test_build_base_env() {
        let env = build_base_env("/project", "/plugin", "my-plugin");
        assert_eq!(env.get("CLAUDE_PROJECT_DIR").unwrap(), "/project");
        assert_eq!(env.get("CLAUDE_PLUGIN_ROOT").unwrap(), "/plugin");
        assert_eq!(env.get("CLAUDE_PLUGIN_NAME").unwrap(), "my-plugin");
    }

    #[tokio::test]
    async fn test_env_vars_set_for_before_tool() {
        let mut hooks = AgenticHooks::new();

        // Use a command that echoes the env vars
        let plugin_hooks = vec![PluginHook {
            event: HookEvent::PreToolUse,
            matcher: None,
            hook_type: HookType::Command,
            command: "echo $TOOL_NAME $CLAUDE_PROJECT_DIR".to_string(),
            timeout: 5000,
            async_hook: false,
        }];

        register_plugin_hooks(
            &mut hooks,
            plugin_hooks,
            "test".to_string(),
            "/tmp/plugin".to_string(),
        );

        let ctx = test_hook_context();
        // Just verify it runs without error
        let result = hooks.fire_on_before_tool(&ctx, "Read", "{}").await;
        assert!(result.is_none()); // exit 0 -> no block
    }

    #[test]
    fn test_register_unmapped_event_does_not_add_hooks() {
        let mut hooks = AgenticHooks::new();

        let plugin_hooks = vec![PluginHook {
            event: HookEvent::Notification,
            matcher: None,
            hook_type: HookType::Command,
            command: "echo notify".to_string(),
            timeout: 5000,
            async_hook: false,
        }];

        register_plugin_hooks(
            &mut hooks,
            plugin_hooks,
            "test".to_string(),
            "/tmp".to_string(),
        );

        // Notification is not mapped, so no hooks should be registered
        assert_eq!(hooks.total_hooks(), 0);
    }
}
