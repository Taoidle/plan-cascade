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
    add_mcp_server,
    aggregate_by_model,
    aggregate_by_project,
    calculate_usage_cost,
    cancel_execution,
    cancel_standalone_execution,
    check_analytics_health,
    // LSP commands
    detect_lsp_servers,
    get_enrichment_report,
    get_lsp_status,
    trigger_lsp_enrichment,
    LspState,
    // Embedding configuration commands
    check_embedding_provider_health,
    check_provider_health,
    check_quality_gates_health,
    cleanup_gate_results,
    cleanup_standalone_sessions,
    compile_spec,
    complete_worktree,
    configure_provider,
    count_usage_records,
    create_agent,
    // Timeline commands
    create_checkpoint,
    create_claude_md,
    // Worktree commands
    create_worktree,
    delete_agent,
    delete_branch,
    delete_checkpoint,
    delete_standalone_session,
    delete_usage_records,
    // Recovery commands
    detect_incomplete_tasks,
    detect_project_type_cmd,
    discard_task,
    execute_standalone,
    // Session-based standalone commands
    execute_standalone_with_session,
    export_agents,
    export_by_model,
    export_by_project,
    export_pricing,
    export_time_series,
    export_usage,
    fork_branch,
    // Design Document commands
    generate_design_doc,
    get_agent,
    get_agent_history,
    get_agent_run,
    get_agent_stats,
    get_available_gates,
    get_branch,
    get_checkpoint,
    get_checkpoint_diff,
    get_claude_md_metadata,
    get_dashboard_summary,
    get_default_gates_for_type,
    get_design_doc,
    get_diff_from_current,
    get_embedding_api_key,
    get_embedding_config,
    get_gate_result,
    get_gate_results,
    // Health commands
    get_health,
    get_interview_state,
    get_model_pricing,
    get_project,
    get_provider_api_key,
    get_session,
    get_session_gate_results,
    get_session_history,
    get_session_info,
    // Settings commands
    get_settings,
    get_standalone_progress,
    get_standalone_session,
    get_standalone_status,
    get_summary_statistics,
    get_time_series,
    get_timeline,
    get_tracking_session,
    get_usage_statistics,
    get_usage_stats,
    get_version,
    get_working_directory,
    get_worktree,
    get_worktree_status,
    import_agents,
    import_design_doc,
    import_from_claude_desktop,
    init_analytics,
    // Init commands
    init_app,
    init_quality_gates,
    list_active_sessions,
    // Agent commands
    list_agents,
    list_agents_with_stats,
    list_all_gates,
    list_branches,
    list_checkpoints,
    list_configured_api_key_providers,
    // Embedding configuration commands
    list_embedding_providers,
    // MCP commands
    connect_mcp_server,
    disconnect_mcp_server,
    list_connected_mcp_servers,
    list_mcp_servers,
    list_mcp_tools,
    list_model_pricing,
    // Memory commands
    add_project_memory,
    clear_project_memories,
    delete_project_memory,
    get_memory_stats,
    list_project_memories,
    search_project_memories,
    update_project_memory,
    // Project commands
    list_projects,
    // Standalone LLM commands
    list_providers,
    // Session commands
    list_sessions,
    list_standalone_sessions,
    list_usage_records,
    list_worktrees,
    prune_agent_runs,
    read_claude_md,
    remove_custom_pricing,
    remove_mcp_server,
    remove_session,
    remove_worktree,
    rename_branch,
    restore_checkpoint,
    resume_session,
    resume_standalone_execution,
    resume_task,
    run_agent,
    run_custom_gates,
    run_quality_gates,
    run_specific_gates,
    save_claude_md,
    save_output_export,
    // Markdown commands
    scan_claude_md,
    search_projects,
    search_sessions,
    send_message,
    set_custom_pricing,
    set_tracking_session,
    set_working_directory,
    // Claude Code commands
    start_chat,
    start_spec_interview,
    submit_interview_answer,
    switch_branch,
    test_mcp_server,
    toggle_mcp_server,
    toggle_generated_skill,
    toggle_skill,
    track_usage,
    update_agent,
    update_mcp_server,
    update_settings,
    // Skill commands
    create_skill_file,
    delete_skill,
    detect_applicable_skills,
    get_skill,
    get_skills_overview,
    list_skills,
    refresh_skill_index,
    search_skills,
    set_embedding_api_key,
    set_embedding_config,
    // Plugin commands
    get_plugin_detail,
    install_plugin,
    list_plugins,
    refresh_plugins,
    toggle_plugin,
    PluginState,
    // Guardrail commands
    add_custom_rule,
    clear_trigger_log,
    get_trigger_log,
    list_guardrails,
    remove_custom_rule,
    toggle_guardrail,
    GuardrailState,
    // Agent Composer commands
    create_agent_pipeline,
    delete_agent_pipeline,
    get_agent_pipeline,
    list_agent_pipelines,
    update_agent_pipeline,
    // Analytics commands
    AnalyticsState,
    ClaudeCodeState,
    InitResult,
    // MCP runtime state
    McpRuntimeState,
    // Quality Gates commands
    QualityGatesState,
    // Spec Interview commands
    SpecInterviewState,
    StandaloneState,
    WorktreeState,
};
// Re-export models (avoiding settings module conflict)
pub use models::response::*;
pub use models::settings::{AppConfig, SettingsUpdate};
pub use state::AppState;
pub use utils::error::{AppError, AppResult};
