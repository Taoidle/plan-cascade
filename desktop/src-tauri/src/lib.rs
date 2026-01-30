//! Plan Cascade Desktop - Rust Backend Library
//!
//! This library provides the core backend functionality for the Plan Cascade Desktop application.
//! It includes:
//! - Tauri command handlers for frontend IPC
//! - Business logic services
//! - Storage layer (SQLite, Keyring, Config)
//! - Data models and utilities

pub mod commands;
pub mod models;
pub mod services;
pub mod state;
pub mod storage;
pub mod utils;

// Re-export commonly used items from commands
pub use commands::{
    // Init commands
    init_app, get_version,
    // Health commands
    get_health,
    // Settings commands
    get_settings, update_settings,
    // Project commands
    list_projects, get_project, search_projects,
    // Session commands
    list_sessions, get_session, resume_session, search_sessions,
    // MCP commands
    list_mcp_servers, add_mcp_server, update_mcp_server, remove_mcp_server,
    test_mcp_server, toggle_mcp_server, import_from_claude_desktop,
    // Claude Code commands
    start_chat, send_message, cancel_execution, get_session_history,
    list_active_sessions, remove_session, get_session_info,
    ClaudeCodeState,
    // Standalone LLM commands
    list_providers, configure_provider, check_provider_health,
    execute_standalone, get_usage_stats, StandaloneState,
    // Session-based standalone commands
    execute_standalone_with_session, cancel_standalone_execution, get_standalone_status,
    get_standalone_progress, resume_standalone_execution, get_standalone_session,
    list_standalone_sessions, delete_standalone_session, cleanup_standalone_sessions,
    // Timeline commands
    create_checkpoint, list_checkpoints, get_checkpoint, delete_checkpoint,
    get_timeline, restore_checkpoint, fork_branch, list_branches, get_branch,
    switch_branch, delete_branch, rename_branch, get_checkpoint_diff, get_diff_from_current,
    // Markdown commands
    scan_claude_md, read_claude_md, save_claude_md, create_claude_md, get_claude_md_metadata,
    // Analytics commands
    AnalyticsState, init_analytics, track_usage, get_tracking_session, set_tracking_session,
    get_usage_statistics, list_usage_records, count_usage_records,
    aggregate_by_model, aggregate_by_project, get_time_series, get_dashboard_summary,
    get_summary_statistics, calculate_usage_cost, get_model_pricing, list_model_pricing,
    set_custom_pricing, remove_custom_pricing, export_usage, export_by_model,
    export_by_project, export_time_series, export_pricing, delete_usage_records,
    check_analytics_health,
    // Agent commands
    list_agents, list_agents_with_stats, get_agent, create_agent, update_agent, delete_agent,
    get_agent_history, get_agent_stats, get_agent_run, prune_agent_runs, run_agent,
    export_agents, import_agents,
    // Worktree commands
    create_worktree, list_worktrees, get_worktree, get_worktree_status,
    remove_worktree, complete_worktree, WorktreeState,
    // Quality Gates commands
    QualityGatesState, init_quality_gates, detect_project_type_cmd,
    get_available_gates, list_all_gates, run_quality_gates, run_specific_gates,
    run_custom_gates, get_gate_results, get_session_gate_results, get_gate_result,
    cleanup_gate_results, get_default_gates_for_type, check_quality_gates_health,
};
// Re-export models (avoiding settings module conflict)
pub use models::response::*;
pub use models::settings::{AppConfig, SettingsUpdate};
pub use state::AppState;
pub use utils::error::{AppError, AppResult};
