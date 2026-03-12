//! Command Router
//!
//! Parses incoming text messages into structured RemoteCommand variants.
//! Full implementation in story-002.

use super::types::RemoteCommand;
use crate::services::orchestrator::permissions::PermissionLevel;
use crate::services::workflow_kernel::WorkflowMode;

/// Stateless command parser for remote messages.
pub struct CommandRouter;

impl CommandRouter {
    /// Parse incoming message text into a RemoteCommand.
    ///
    /// Supports slash commands and plain text:
    /// - `/new <path> [provider] [model]` -> NewSession
    /// - `/send <message>` -> SendMessage
    /// - `/sessions` -> ListSessions
    /// - `/switch <session_id>` -> SwitchSession
    /// - `/status` -> Status
    /// - `/cancel` -> Cancel
    /// - `/close` -> CloseSession
    /// - `/help` -> Help
    /// - Plain text -> SendMessage
    pub fn parse(text: &str) -> RemoteCommand {
        let text = text.trim();

        if matches!(text, "/start" | "/home") {
            RemoteCommand::Home
        } else if text == "/menu" {
            RemoteCommand::Menu
        } else if text == "/chat" {
            RemoteCommand::SwitchMode {
                mode: WorkflowMode::Chat,
            }
        } else if text == "/plan" {
            RemoteCommand::SwitchMode {
                mode: WorkflowMode::Plan,
            }
        } else if text == "/task" {
            RemoteCommand::SwitchMode {
                mode: WorkflowMode::Task,
            }
        } else if text == "/debug" {
            RemoteCommand::SwitchMode {
                mode: WorkflowMode::Debug,
            }
        } else if text == "/context" {
            RemoteCommand::Context
        } else if text == "/permission" {
            RemoteCommand::Permission
        } else if text == "/resume" {
            RemoteCommand::Resume
        } else if text == "/artifacts" {
            RemoteCommand::Artifacts
        } else if text.starts_with("/new ") || text == "/new" {
            let args_str = if text.len() > 5 { text[5..].trim() } else { "" };
            let args: Vec<&str> = args_str.splitn(3, ' ').collect();
            RemoteCommand::NewSession {
                project_path: args.first().unwrap_or(&"").to_string(),
                provider: args.get(1).map(|s| s.to_string()),
                model: args.get(2).map(|s| s.to_string()),
            }
        } else if text == "/sessions" {
            RemoteCommand::ListSessions
        } else if text.starts_with("/switch ") {
            RemoteCommand::SwitchSession {
                session_id: text[8..].trim().to_string(),
            }
        } else if text == "/status" {
            RemoteCommand::Status
        } else if text == "/cancel" {
            RemoteCommand::Cancel
        } else if text == "/close" {
            RemoteCommand::CloseSession
        } else if text == "/whoami" {
            RemoteCommand::WhoAmI
        } else if text == "/help" {
            RemoteCommand::Help
        } else if text.starts_with("/send ") {
            RemoteCommand::SendMessage {
                content: text[6..].to_string(),
            }
        } else {
            // Plain text -> treat as message to active session
            RemoteCommand::SendMessage {
                content: text.to_string(),
            }
        }
    }

    pub fn parse_callback(callback: &str) -> Option<RemoteCommand> {
        if let Some(request_id) = callback.trim().strip_prefix("remote:approval:allow-once:") {
            return Some(RemoteCommand::RespondPermission {
                request_id: request_id.to_string(),
                allowed: true,
                always_allow: false,
            });
        }
        if let Some(request_id) = callback.trim().strip_prefix("remote:approval:always-allow:") {
            return Some(RemoteCommand::RespondPermission {
                request_id: request_id.to_string(),
                allowed: true,
                always_allow: true,
            });
        }
        if let Some(request_id) = callback.trim().strip_prefix("remote:approval:deny:") {
            return Some(RemoteCommand::RespondPermission {
                request_id: request_id.to_string(),
                allowed: false,
                always_allow: false,
            });
        }
        match callback.trim() {
            "remote:home" | "remote:menu" => Some(RemoteCommand::Home),
            "remote:status" => Some(RemoteCommand::Status),
            "remote:context" => Some(RemoteCommand::Context),
            "remote:permission" => Some(RemoteCommand::Permission),
            "remote:resume" => Some(RemoteCommand::Resume),
            "remote:artifacts" => Some(RemoteCommand::Artifacts),
            "remote:cancel" => Some(RemoteCommand::Cancel),
            "remote:sessions" => Some(RemoteCommand::ListSessions),
            "remote:whoami" => Some(RemoteCommand::WhoAmI),
            "remote:help" => Some(RemoteCommand::Help),
            "remote:plan:generate" => Some(RemoteCommand::PlanGenerate),
            "remote:plan:approve" => Some(RemoteCommand::PlanApprove),
            "remote:task:confirm-config" => Some(RemoteCommand::TaskConfirmConfig),
            "remote:task:generate-prd" => Some(RemoteCommand::TaskGeneratePrd),
            "remote:task:approve-prd" => Some(RemoteCommand::TaskApprovePrd),
            "remote:debug:approve-patch" => Some(RemoteCommand::DebugApprovePatch),
            "remote:context:preset:default" => Some(RemoteCommand::SetContextPreset {
                preset: "default".to_string(),
            }),
            "remote:context:preset:focused" => Some(RemoteCommand::SetContextPreset {
                preset: "focused".to_string(),
            }),
            "remote:context:preset:knowledge" => Some(RemoteCommand::SetContextPreset {
                preset: "knowledge".to_string(),
            }),
            "remote:context:preset:memory" => Some(RemoteCommand::SetContextPreset {
                preset: "memory".to_string(),
            }),
            "remote:context:preset:skills" => Some(RemoteCommand::SetContextPreset {
                preset: "skills".to_string(),
            }),
            "remote:context:toggle:knowledge" => Some(RemoteCommand::ToggleContextSource {
                source: "knowledge".to_string(),
            }),
            "remote:context:toggle:memory" => Some(RemoteCommand::ToggleContextSource {
                source: "memory".to_string(),
            }),
            "remote:context:toggle:skills" => Some(RemoteCommand::ToggleContextSource {
                source: "skills".to_string(),
            }),
            "remote:permission:set:strict" => Some(RemoteCommand::SetPermissionLevel {
                level: PermissionLevel::Strict,
            }),
            "remote:permission:set:standard" => Some(RemoteCommand::SetPermissionLevel {
                level: PermissionLevel::Standard,
            }),
            "remote:permission:set:permissive" => Some(RemoteCommand::SetPermissionLevel {
                level: PermissionLevel::Permissive,
            }),
            "remote:mode:switch:chat" => Some(RemoteCommand::SwitchMode {
                mode: WorkflowMode::Chat,
            }),
            "remote:mode:switch:plan" => Some(RemoteCommand::SwitchMode {
                mode: WorkflowMode::Plan,
            }),
            "remote:mode:switch:task" => Some(RemoteCommand::SwitchMode {
                mode: WorkflowMode::Task,
            }),
            "remote:mode:switch:debug" => Some(RemoteCommand::SwitchMode {
                mode: WorkflowMode::Debug,
            }),
            _ => None,
        }
    }
}

/// Help text displayed when user sends /help
pub const HELP_TEXT: &str = r#"Plan Cascade Remote Control

Available commands:
  /start, /home                    -- Open remote control console
  /chat | /plan | /task | /debug  -- Switch active mode
  /context                         -- Show context configuration
  /permission                      -- Show permission configuration
  /whoami                          -- Show your chat/user IDs for whitelist setup
  /resume                          -- Resume current workflow session
  /artifacts                       -- Show recent artifacts
  Plan/Task/Debug review actions  -- Use inline buttons for generate/approve steps
  /new <path> [provider] [model]  -- Create new session
  /send <message>                 -- Send message (or just type directly)
  /sessions                       -- List active sessions
  /switch <id>                    -- Switch to a session
  /status                         -- Current session status
  /cancel                         -- Cancel running execution
  /close                          -- Close current session
  /auth <password>                -- Authenticate when password gate is enabled
  /help                           -- Show this help

Quick start:
  1. Send /whoami and add your user_id to Allowed User IDs in Settings
  2. If password gate is enabled, send /auth <password>
  3. Send /start to open the console
  4. Send /new or /new ~/projects/myapp to open a workspace
  5. Use /chat, /plan, /task, or /debug and then type normally

Examples:
  /whoami
  /new ~/projects/myapp
  /new ~/projects/api anthropic claude-sonnet-4-5-20250929
  How do I fix the login bug?
  /cancel
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::remote::types::RemoteCommand;
    use crate::services::workflow_kernel::WorkflowMode;

    // -----------------------------------------------------------------------
    // Slash command parsing
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_new_with_path() {
        let cmd = CommandRouter::parse("/new ~/projects/myapp");
        assert_eq!(
            cmd,
            RemoteCommand::NewSession {
                project_path: "~/projects/myapp".to_string(),
                provider: None,
                model: None,
            }
        );
    }

    #[test]
    fn test_parse_new_with_path_and_provider() {
        let cmd = CommandRouter::parse("/new ~/projects/api anthropic");
        assert_eq!(
            cmd,
            RemoteCommand::NewSession {
                project_path: "~/projects/api".to_string(),
                provider: Some("anthropic".to_string()),
                model: None,
            }
        );
    }

    #[test]
    fn test_parse_new_with_all_args() {
        let cmd = CommandRouter::parse("/new ~/projects/api anthropic claude-sonnet-4-5-20250929");
        assert_eq!(
            cmd,
            RemoteCommand::NewSession {
                project_path: "~/projects/api".to_string(),
                provider: Some("anthropic".to_string()),
                model: Some("claude-sonnet-4-5-20250929".to_string()),
            }
        );
    }

    #[test]
    fn test_parse_new_no_args() {
        let cmd = CommandRouter::parse("/new");
        assert_eq!(
            cmd,
            RemoteCommand::NewSession {
                project_path: "".to_string(),
                provider: None,
                model: None,
            }
        );
    }

    #[test]
    fn test_parse_new_with_trailing_space() {
        let cmd = CommandRouter::parse("/new ");
        assert_eq!(
            cmd,
            RemoteCommand::NewSession {
                project_path: "".to_string(),
                provider: None,
                model: None,
            }
        );
    }

    #[test]
    fn test_parse_send() {
        let cmd = CommandRouter::parse("/send fix the login bug");
        assert_eq!(
            cmd,
            RemoteCommand::SendMessage {
                content: "fix the login bug".to_string(),
            }
        );
    }

    #[test]
    fn test_parse_sessions() {
        let cmd = CommandRouter::parse("/sessions");
        assert_eq!(cmd, RemoteCommand::ListSessions);
    }

    #[test]
    fn test_parse_switch() {
        let cmd = CommandRouter::parse("/switch abc-123-def");
        assert_eq!(
            cmd,
            RemoteCommand::SwitchSession {
                session_id: "abc-123-def".to_string(),
            }
        );
    }

    #[test]
    fn test_parse_status() {
        let cmd = CommandRouter::parse("/status");
        assert_eq!(cmd, RemoteCommand::Status);
    }

    #[test]
    fn test_parse_cancel() {
        let cmd = CommandRouter::parse("/cancel");
        assert_eq!(cmd, RemoteCommand::Cancel);
    }

    #[test]
    fn test_parse_close() {
        let cmd = CommandRouter::parse("/close");
        assert_eq!(cmd, RemoteCommand::CloseSession);
    }

    #[test]
    fn test_parse_help() {
        let cmd = CommandRouter::parse("/help");
        assert_eq!(cmd, RemoteCommand::Help);
    }

    #[test]
    fn test_parse_whoami() {
        let cmd = CommandRouter::parse("/whoami");
        assert_eq!(cmd, RemoteCommand::WhoAmI);
    }

    #[test]
    fn test_parse_mode_commands() {
        assert_eq!(
            CommandRouter::parse("/chat"),
            RemoteCommand::SwitchMode {
                mode: WorkflowMode::Chat,
            }
        );
        assert_eq!(
            CommandRouter::parse("/plan"),
            RemoteCommand::SwitchMode {
                mode: WorkflowMode::Plan,
            }
        );
    }

    #[test]
    fn test_parse_home_and_menu() {
        assert_eq!(CommandRouter::parse("/start"), RemoteCommand::Home);
        assert_eq!(CommandRouter::parse("/menu"), RemoteCommand::Menu);
    }

    // -----------------------------------------------------------------------
    // Plain text and edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_plain_text() {
        let cmd = CommandRouter::parse("How do I fix the login bug?");
        assert_eq!(
            cmd,
            RemoteCommand::SendMessage {
                content: "How do I fix the login bug?".to_string(),
            }
        );
    }

    #[test]
    fn test_parse_plain_text_with_slash_in_middle() {
        let cmd = CommandRouter::parse("Use src/main.rs file");
        assert_eq!(
            cmd,
            RemoteCommand::SendMessage {
                content: "Use src/main.rs file".to_string(),
            }
        );
    }

    #[test]
    fn test_parse_unknown_command() {
        let cmd = CommandRouter::parse("/unknown something");
        // Unknown commands become SendMessage
        assert_eq!(
            cmd,
            RemoteCommand::SendMessage {
                content: "/unknown something".to_string(),
            }
        );
    }

    #[test]
    fn test_parse_whitespace_trimming() {
        let cmd = CommandRouter::parse("  /help  ");
        assert_eq!(cmd, RemoteCommand::Help);
    }

    #[test]
    fn test_parse_whitespace_trimming_plain_text() {
        let cmd = CommandRouter::parse("  hello world  ");
        assert_eq!(
            cmd,
            RemoteCommand::SendMessage {
                content: "hello world".to_string(),
            }
        );
    }

    #[test]
    fn test_parse_empty_input() {
        let cmd = CommandRouter::parse("");
        assert_eq!(
            cmd,
            RemoteCommand::SendMessage {
                content: "".to_string(),
            }
        );
    }

    #[test]
    fn test_parse_new_with_extra_whitespace_between_args() {
        let cmd = CommandRouter::parse("/new  ~/projects/myapp   anthropic   model-name");
        // splitn(3, ' ') on "~/projects/myapp   anthropic   model-name" after trim
        // The first space split separates the first empty part from rest
        match cmd {
            RemoteCommand::NewSession { project_path, .. } => {
                // After trimming "/new " we get " ~/projects/myapp   anthropic   model-name"
                // which trimmed gives "~/projects/myapp   anthropic   model-name"
                assert!(!project_path.is_empty());
            }
            _ => panic!("Expected NewSession"),
        }
    }

    #[test]
    fn test_parse_switch_with_whitespace() {
        let cmd = CommandRouter::parse("/switch   abc-123  ");
        assert_eq!(
            cmd,
            RemoteCommand::SwitchSession {
                session_id: "abc-123".to_string(),
            }
        );
    }

    // -----------------------------------------------------------------------
    // Help text
    // -----------------------------------------------------------------------

    #[test]
    fn test_help_text_contains_all_commands() {
        assert!(HELP_TEXT.contains("/start"));
        assert!(HELP_TEXT.contains("/chat"));
        assert!(HELP_TEXT.contains("/new"));
        assert!(HELP_TEXT.contains("/send"));
        assert!(HELP_TEXT.contains("/whoami"));
        assert!(HELP_TEXT.contains("/sessions"));
        assert!(HELP_TEXT.contains("/switch"));
        assert!(HELP_TEXT.contains("/status"));
        assert!(HELP_TEXT.contains("/cancel"));
        assert!(HELP_TEXT.contains("/close"));
        assert!(HELP_TEXT.contains("/help"));
        assert!(HELP_TEXT.contains("/auth <password>"));
    }

    #[test]
    fn test_help_text_contains_examples() {
        assert!(HELP_TEXT.contains("~/projects/myapp"));
        assert!(HELP_TEXT.contains("anthropic"));
    }

    // -----------------------------------------------------------------------
    // Commands are case-sensitive (Telegram convention)
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_case_sensitive() {
        // Uppercase should not match slash commands
        let cmd = CommandRouter::parse("/HELP");
        assert_eq!(
            cmd,
            RemoteCommand::SendMessage {
                content: "/HELP".to_string(),
            }
        );
    }

    #[test]
    fn test_parse_mixed_case() {
        let cmd = CommandRouter::parse("/Status");
        assert_eq!(
            cmd,
            RemoteCommand::SendMessage {
                content: "/Status".to_string(),
            }
        );
    }

    #[test]
    fn test_parse_callback() {
        assert_eq!(
            CommandRouter::parse_callback("remote:mode:switch:debug"),
            Some(RemoteCommand::SwitchMode {
                mode: WorkflowMode::Debug,
            })
        );
        assert_eq!(
            CommandRouter::parse_callback("remote:status"),
            Some(RemoteCommand::Status)
        );
        assert_eq!(
            CommandRouter::parse_callback("remote:whoami"),
            Some(RemoteCommand::WhoAmI)
        );
        assert_eq!(
            CommandRouter::parse_callback("remote:approval:allow-once:req-1"),
            Some(RemoteCommand::RespondPermission {
                request_id: "req-1".to_string(),
                allowed: true,
                always_allow: false,
            })
        );
        assert_eq!(
            CommandRouter::parse_callback("remote:permission:set:permissive"),
            Some(RemoteCommand::SetPermissionLevel {
                level: PermissionLevel::Permissive,
            })
        );
    }
}
