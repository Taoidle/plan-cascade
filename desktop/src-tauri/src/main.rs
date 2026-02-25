// Plan Cascade Desktop - Tauri Application Entry Point
// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use plan_cascade_desktop::commands::analytics::AnalyticsState;
use plan_cascade_desktop::commands::artifacts::ArtifactState;
use plan_cascade_desktop::commands::claude_code::ClaudeCodeState;
use plan_cascade_desktop::commands::file_changes::FileChangesState;
use plan_cascade_desktop::commands::git::GitState;
use plan_cascade_desktop::commands::guardrails::GuardrailState;
use plan_cascade_desktop::commands::knowledge::KnowledgeState;
use plan_cascade_desktop::commands::lsp::LspState;
use plan_cascade_desktop::commands::mcp::McpRuntimeState;
use plan_cascade_desktop::commands::permissions::PermissionState;
use plan_cascade_desktop::commands::pipeline_execution::ExecutionRegistry;
use plan_cascade_desktop::commands::plugins::PluginState;
use plan_cascade_desktop::commands::quality_gates::QualityGatesState;
use plan_cascade_desktop::commands::remote::RemoteState;
use plan_cascade_desktop::commands::spec_interview::SpecInterviewState;
use plan_cascade_desktop::commands::standalone::StandaloneState;
use plan_cascade_desktop::commands::task_mode::TaskModeState;
use plan_cascade_desktop::commands::webhook::WebhookState;
use plan_cascade_desktop::commands::worktree::WorktreeState;
use plan_cascade_desktop::state::AppState;

use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::new())
        .manage(ClaudeCodeState::new())
        .manage(AnalyticsState::new())
        .manage(QualityGatesState::new())
        .manage(WorktreeState::new())
        .manage(StandaloneState::new())
        .manage(SpecInterviewState::new())
        .manage(McpRuntimeState::new())
        .manage(LspState::new())
        .manage(PluginState::new())
        .manage(GuardrailState::new())
        .manage(WebhookState::new_empty())
        .manage(RemoteState::new())
        .manage(TaskModeState::new())
        .manage(ExecutionRegistry::new())
        .manage(KnowledgeState::new())
        .manage(ArtifactState::new())
        .manage(GitState::new())
        .manage(FileChangesState::new())
        .manage(PermissionState::new())
        .invoke_handler(tauri::generate_handler![
            // Initialization commands
            plan_cascade_desktop::commands::init::init_app,
            plan_cascade_desktop::commands::init::get_version,
            // Health commands
            plan_cascade_desktop::commands::health::get_health,
            // Settings commands
            plan_cascade_desktop::commands::settings::get_settings,
            plan_cascade_desktop::commands::settings::update_settings,
            // Project commands
            plan_cascade_desktop::commands::projects::list_projects,
            plan_cascade_desktop::commands::projects::get_project,
            plan_cascade_desktop::commands::projects::search_projects,
            // Session commands
            plan_cascade_desktop::commands::sessions::list_sessions,
            plan_cascade_desktop::commands::sessions::get_session,
            plan_cascade_desktop::commands::sessions::resume_session,
            plan_cascade_desktop::commands::sessions::search_sessions,
            // MCP commands
            plan_cascade_desktop::commands::mcp::list_mcp_servers,
            plan_cascade_desktop::commands::mcp::add_mcp_server,
            plan_cascade_desktop::commands::mcp::update_mcp_server,
            plan_cascade_desktop::commands::mcp::remove_mcp_server,
            plan_cascade_desktop::commands::mcp::test_mcp_server,
            plan_cascade_desktop::commands::mcp::toggle_mcp_server,
            plan_cascade_desktop::commands::mcp::import_from_claude_desktop,
            plan_cascade_desktop::commands::mcp::connect_mcp_server,
            plan_cascade_desktop::commands::mcp::disconnect_mcp_server,
            plan_cascade_desktop::commands::mcp::list_connected_mcp_servers,
            plan_cascade_desktop::commands::mcp::list_mcp_tools,
            // Claude Code commands
            plan_cascade_desktop::commands::claude_code::start_chat,
            plan_cascade_desktop::commands::claude_code::send_message,
            plan_cascade_desktop::commands::claude_code::cancel_execution,
            plan_cascade_desktop::commands::claude_code::get_session_history,
            plan_cascade_desktop::commands::claude_code::list_active_sessions,
            plan_cascade_desktop::commands::claude_code::remove_session,
            plan_cascade_desktop::commands::claude_code::get_session_info,
            // Timeline commands
            plan_cascade_desktop::commands::timeline::create_checkpoint,
            plan_cascade_desktop::commands::timeline::list_checkpoints,
            plan_cascade_desktop::commands::timeline::get_checkpoint,
            plan_cascade_desktop::commands::timeline::delete_checkpoint,
            plan_cascade_desktop::commands::timeline::get_timeline,
            plan_cascade_desktop::commands::timeline::restore_checkpoint,
            plan_cascade_desktop::commands::timeline::fork_branch,
            plan_cascade_desktop::commands::timeline::list_branches,
            plan_cascade_desktop::commands::timeline::get_branch,
            plan_cascade_desktop::commands::timeline::switch_branch,
            plan_cascade_desktop::commands::timeline::delete_branch,
            plan_cascade_desktop::commands::timeline::rename_branch,
            plan_cascade_desktop::commands::timeline::get_checkpoint_diff,
            plan_cascade_desktop::commands::timeline::get_diff_from_current,
            // Markdown commands
            plan_cascade_desktop::commands::markdown::scan_claude_md,
            plan_cascade_desktop::commands::markdown::read_claude_md,
            plan_cascade_desktop::commands::markdown::save_claude_md,
            plan_cascade_desktop::commands::markdown::create_claude_md,
            plan_cascade_desktop::commands::markdown::get_claude_md_metadata,
            // Analytics commands
            plan_cascade_desktop::commands::analytics::init_analytics,
            plan_cascade_desktop::commands::analytics::track_usage,
            plan_cascade_desktop::commands::analytics::get_tracking_session,
            plan_cascade_desktop::commands::analytics::set_tracking_session,
            plan_cascade_desktop::commands::analytics::get_usage_statistics,
            plan_cascade_desktop::commands::analytics::list_usage_records,
            plan_cascade_desktop::commands::analytics::count_usage_records,
            plan_cascade_desktop::commands::analytics::aggregate_by_model,
            plan_cascade_desktop::commands::analytics::aggregate_by_project,
            plan_cascade_desktop::commands::analytics::get_time_series,
            plan_cascade_desktop::commands::analytics::get_dashboard_summary,
            plan_cascade_desktop::commands::analytics::get_summary_statistics,
            plan_cascade_desktop::commands::analytics::calculate_usage_cost,
            plan_cascade_desktop::commands::analytics::get_model_pricing,
            plan_cascade_desktop::commands::analytics::list_model_pricing,
            plan_cascade_desktop::commands::analytics::set_custom_pricing,
            plan_cascade_desktop::commands::analytics::remove_custom_pricing,
            plan_cascade_desktop::commands::analytics::export_usage,
            plan_cascade_desktop::commands::analytics::export_by_model,
            plan_cascade_desktop::commands::analytics::export_by_project,
            plan_cascade_desktop::commands::analytics::export_time_series,
            plan_cascade_desktop::commands::analytics::export_pricing,
            plan_cascade_desktop::commands::analytics::delete_usage_records,
            plan_cascade_desktop::commands::analytics::check_analytics_health,
            // Agent commands
            plan_cascade_desktop::commands::agents::list_agents,
            plan_cascade_desktop::commands::agents::list_agents_with_stats,
            plan_cascade_desktop::commands::agents::get_agent,
            plan_cascade_desktop::commands::agents::create_agent,
            plan_cascade_desktop::commands::agents::update_agent,
            plan_cascade_desktop::commands::agents::delete_agent,
            plan_cascade_desktop::commands::agents::get_agent_history,
            plan_cascade_desktop::commands::agents::get_agent_stats,
            plan_cascade_desktop::commands::agents::get_agent_run,
            plan_cascade_desktop::commands::agents::prune_agent_runs,
            plan_cascade_desktop::commands::agents::run_agent,
            plan_cascade_desktop::commands::agents::export_agents,
            plan_cascade_desktop::commands::agents::import_agents,
            // Quality Gates commands
            plan_cascade_desktop::commands::quality_gates::init_quality_gates,
            plan_cascade_desktop::commands::quality_gates::detect_project_type_cmd,
            plan_cascade_desktop::commands::quality_gates::get_available_gates,
            plan_cascade_desktop::commands::quality_gates::list_all_gates,
            plan_cascade_desktop::commands::quality_gates::run_quality_gates,
            plan_cascade_desktop::commands::quality_gates::run_specific_gates,
            plan_cascade_desktop::commands::quality_gates::run_custom_gates,
            plan_cascade_desktop::commands::quality_gates::get_gate_results,
            plan_cascade_desktop::commands::quality_gates::get_session_gate_results,
            plan_cascade_desktop::commands::quality_gates::get_gate_result,
            plan_cascade_desktop::commands::quality_gates::cleanup_gate_results,
            plan_cascade_desktop::commands::quality_gates::get_default_gates_for_type,
            plan_cascade_desktop::commands::quality_gates::check_quality_gates_health,
            // Worktree commands
            plan_cascade_desktop::commands::worktree::create_worktree,
            plan_cascade_desktop::commands::worktree::list_worktrees,
            plan_cascade_desktop::commands::worktree::get_worktree,
            plan_cascade_desktop::commands::worktree::get_worktree_status,
            plan_cascade_desktop::commands::worktree::remove_worktree,
            plan_cascade_desktop::commands::worktree::complete_worktree,
            // Standalone LLM commands
            plan_cascade_desktop::commands::standalone::list_providers,
            plan_cascade_desktop::commands::standalone::list_configured_api_key_providers,
            plan_cascade_desktop::commands::standalone::get_provider_api_key,
            plan_cascade_desktop::commands::standalone::configure_provider,
            plan_cascade_desktop::commands::standalone::check_provider_health,
            plan_cascade_desktop::commands::standalone::execute_standalone,
            plan_cascade_desktop::commands::standalone::save_output_export,
            plan_cascade_desktop::commands::standalone::save_binary_export,
            plan_cascade_desktop::commands::standalone::get_usage_stats,
            // Session-based standalone commands
            plan_cascade_desktop::commands::standalone::execute_standalone_with_session,
            plan_cascade_desktop::commands::standalone::cancel_standalone_execution,
            plan_cascade_desktop::commands::standalone::pause_standalone_execution,
            plan_cascade_desktop::commands::standalone::unpause_standalone_execution,
            plan_cascade_desktop::commands::standalone::get_standalone_status,
            plan_cascade_desktop::commands::standalone::get_standalone_progress,
            plan_cascade_desktop::commands::standalone::resume_standalone_execution,
            plan_cascade_desktop::commands::standalone::get_standalone_session,
            plan_cascade_desktop::commands::standalone::list_standalone_sessions,
            plan_cascade_desktop::commands::standalone::delete_standalone_session,
            plan_cascade_desktop::commands::standalone::cleanup_standalone_sessions,
            plan_cascade_desktop::commands::standalone::get_working_directory,
            plan_cascade_desktop::commands::standalone::set_working_directory,
            plan_cascade_desktop::commands::standalone::get_index_status,
            plan_cascade_desktop::commands::standalone::trigger_reindex,
            plan_cascade_desktop::commands::standalone::semantic_search,
            // Strategy commands
            plan_cascade_desktop::commands::strategy::analyze_task_strategy,
            plan_cascade_desktop::commands::strategy::get_strategy_options,
            plan_cascade_desktop::commands::strategy::classify_intent,
            plan_cascade_desktop::commands::strategy::override_task_strategy,
            plan_cascade_desktop::commands::strategy::analyze_task_for_mode,
            plan_cascade_desktop::commands::strategy::enhance_strategy_with_llm,
            // Task Mode commands
            plan_cascade_desktop::commands::task_mode::enter_task_mode,
            plan_cascade_desktop::commands::task_mode::explore_project,
            plan_cascade_desktop::commands::task_mode::generate_task_prd,
            plan_cascade_desktop::commands::task_mode::approve_task_prd,
            plan_cascade_desktop::commands::task_mode::get_task_execution_status,
            plan_cascade_desktop::commands::task_mode::cancel_task_execution,
            plan_cascade_desktop::commands::task_mode::get_task_execution_report,
            plan_cascade_desktop::commands::task_mode::exit_task_mode,
            plan_cascade_desktop::commands::task_mode::prepare_design_doc_for_task,
            plan_cascade_desktop::commands::task_mode::run_requirement_analysis,
            plan_cascade_desktop::commands::task_mode::run_architecture_review,
            // Spec Interview commands
            plan_cascade_desktop::commands::spec_interview::start_spec_interview,
            plan_cascade_desktop::commands::spec_interview::submit_interview_answer,
            plan_cascade_desktop::commands::spec_interview::get_interview_state,
            plan_cascade_desktop::commands::spec_interview::compile_spec,
            // Recovery commands
            plan_cascade_desktop::commands::recovery::detect_incomplete_tasks,
            plan_cascade_desktop::commands::recovery::resume_task,
            plan_cascade_desktop::commands::recovery::discard_task,
            // Design Document commands
            plan_cascade_desktop::commands::design::generate_design_doc,
            plan_cascade_desktop::commands::design::import_design_doc,
            plan_cascade_desktop::commands::design::get_design_doc,
            // File attachment commands
            plan_cascade_desktop::commands::files::read_file_for_attachment,
            plan_cascade_desktop::commands::files::list_workspace_files,
            // Memory commands
            plan_cascade_desktop::commands::memory::search_project_memories,
            plan_cascade_desktop::commands::memory::list_project_memories,
            plan_cascade_desktop::commands::memory::add_project_memory,
            plan_cascade_desktop::commands::memory::update_project_memory,
            plan_cascade_desktop::commands::memory::delete_project_memory,
            plan_cascade_desktop::commands::memory::clear_project_memories,
            plan_cascade_desktop::commands::memory::get_memory_stats,
            plan_cascade_desktop::commands::memory::extract_session_memories,
            // Skill commands
            plan_cascade_desktop::commands::skills::list_skills,
            plan_cascade_desktop::commands::skills::get_skill,
            plan_cascade_desktop::commands::skills::search_skills,
            plan_cascade_desktop::commands::skills::detect_applicable_skills,
            plan_cascade_desktop::commands::skills::toggle_skill,
            plan_cascade_desktop::commands::skills::create_skill_file,
            plan_cascade_desktop::commands::skills::delete_skill,
            plan_cascade_desktop::commands::skills::toggle_generated_skill,
            plan_cascade_desktop::commands::skills::refresh_skill_index,
            plan_cascade_desktop::commands::skills::get_skills_overview,
            // Embedding configuration commands
            plan_cascade_desktop::commands::embedding::get_embedding_config,
            plan_cascade_desktop::commands::embedding::set_embedding_config,
            plan_cascade_desktop::commands::embedding::list_embedding_providers,
            plan_cascade_desktop::commands::embedding::check_embedding_provider_health,
            plan_cascade_desktop::commands::embedding::set_embedding_api_key,
            plan_cascade_desktop::commands::embedding::get_embedding_api_key,
            // Prompt commands
            plan_cascade_desktop::commands::prompts::list_prompts,
            plan_cascade_desktop::commands::prompts::create_prompt,
            plan_cascade_desktop::commands::prompts::update_prompt,
            plan_cascade_desktop::commands::prompts::delete_prompt,
            plan_cascade_desktop::commands::prompts::record_prompt_use,
            plan_cascade_desktop::commands::prompts::toggle_prompt_pin,
            // Plugin commands
            plan_cascade_desktop::commands::plugins::list_plugins,
            plan_cascade_desktop::commands::plugins::toggle_plugin,
            plan_cascade_desktop::commands::plugins::refresh_plugins,
            plan_cascade_desktop::commands::plugins::get_plugin_detail,
            plan_cascade_desktop::commands::plugins::install_plugin,
            plan_cascade_desktop::commands::plugins::fetch_marketplace,
            plan_cascade_desktop::commands::plugins::install_plugin_from_git,
            plan_cascade_desktop::commands::plugins::uninstall_plugin,
            plan_cascade_desktop::commands::plugins::list_marketplaces,
            plan_cascade_desktop::commands::plugins::add_marketplace,
            plan_cascade_desktop::commands::plugins::remove_marketplace,
            plan_cascade_desktop::commands::plugins::toggle_marketplace,
            plan_cascade_desktop::commands::plugins::install_marketplace_plugin,
            // LSP enrichment commands
            plan_cascade_desktop::commands::lsp::detect_lsp_servers,
            plan_cascade_desktop::commands::lsp::get_lsp_status,
            plan_cascade_desktop::commands::lsp::trigger_lsp_enrichment,
            plan_cascade_desktop::commands::lsp::get_enrichment_report,
            // Guardrail commands
            plan_cascade_desktop::commands::guardrails::list_guardrails,
            plan_cascade_desktop::commands::guardrails::toggle_guardrail,
            plan_cascade_desktop::commands::guardrails::add_custom_rule,
            plan_cascade_desktop::commands::guardrails::remove_custom_rule,
            plan_cascade_desktop::commands::guardrails::get_trigger_log,
            plan_cascade_desktop::commands::guardrails::clear_trigger_log,
            // Agent Composer commands
            plan_cascade_desktop::commands::agent_composer::list_agent_pipelines,
            plan_cascade_desktop::commands::agent_composer::get_agent_pipeline,
            plan_cascade_desktop::commands::agent_composer::create_agent_pipeline,
            plan_cascade_desktop::commands::agent_composer::update_agent_pipeline,
            plan_cascade_desktop::commands::agent_composer::delete_agent_pipeline,
            // Graph Workflow commands
            plan_cascade_desktop::commands::graph_workflow::list_graph_workflows,
            plan_cascade_desktop::commands::graph_workflow::get_graph_workflow,
            plan_cascade_desktop::commands::graph_workflow::create_graph_workflow,
            plan_cascade_desktop::commands::graph_workflow::update_graph_workflow,
            plan_cascade_desktop::commands::graph_workflow::delete_graph_workflow,
            plan_cascade_desktop::commands::graph_workflow::export_graph_workflow,
            // Proxy commands
            plan_cascade_desktop::commands::proxy::get_proxy_config,
            plan_cascade_desktop::commands::proxy::set_proxy_config,
            plan_cascade_desktop::commands::proxy::get_provider_proxy_strategy,
            plan_cascade_desktop::commands::proxy::set_provider_proxy_strategy,
            plan_cascade_desktop::commands::proxy::test_proxy,
            // Webhook commands
            plan_cascade_desktop::commands::webhook::list_webhook_channels,
            plan_cascade_desktop::commands::webhook::create_webhook_channel,
            plan_cascade_desktop::commands::webhook::update_webhook_channel,
            plan_cascade_desktop::commands::webhook::delete_webhook_channel,
            plan_cascade_desktop::commands::webhook::test_webhook_channel,
            plan_cascade_desktop::commands::webhook::get_webhook_deliveries,
            plan_cascade_desktop::commands::webhook::retry_webhook_delivery,
            // Remote Control commands
            plan_cascade_desktop::commands::remote::get_remote_gateway_status,
            plan_cascade_desktop::commands::remote::start_remote_gateway,
            plan_cascade_desktop::commands::remote::stop_remote_gateway,
            plan_cascade_desktop::commands::remote::get_remote_config,
            plan_cascade_desktop::commands::remote::update_remote_config,
            plan_cascade_desktop::commands::remote::get_telegram_config,
            plan_cascade_desktop::commands::remote::update_telegram_config,
            plan_cascade_desktop::commands::remote::list_remote_sessions,
            plan_cascade_desktop::commands::remote::disconnect_remote_session,
            plan_cascade_desktop::commands::remote::get_remote_audit_log,
            // Pipeline Execution commands
            plan_cascade_desktop::commands::pipeline_execution::execute_agent_pipeline,
            plan_cascade_desktop::commands::pipeline_execution::execute_graph_workflow_run,
            plan_cascade_desktop::commands::pipeline_execution::get_pipeline_execution_status,
            plan_cascade_desktop::commands::pipeline_execution::cancel_pipeline_execution,
            // Evaluation commands
            plan_cascade_desktop::commands::evaluation::list_evaluators,
            plan_cascade_desktop::commands::evaluation::create_evaluator,
            plan_cascade_desktop::commands::evaluation::delete_evaluator,
            plan_cascade_desktop::commands::evaluation::create_evaluation_run,
            plan_cascade_desktop::commands::evaluation::list_evaluation_runs,
            plan_cascade_desktop::commands::evaluation::get_evaluation_reports,
            plan_cascade_desktop::commands::evaluation::delete_evaluation_run,
            // Knowledge commands
            plan_cascade_desktop::commands::knowledge::rag_ingest_documents,
            plan_cascade_desktop::commands::knowledge::rag_query,
            plan_cascade_desktop::commands::knowledge::rag_list_collections,
            plan_cascade_desktop::commands::knowledge::rag_delete_collection,
            // Git Source Control commands
            plan_cascade_desktop::commands::git::git_full_status,
            plan_cascade_desktop::commands::git::git_stage_files,
            plan_cascade_desktop::commands::git::git_unstage_files,
            plan_cascade_desktop::commands::git::git_stage_hunk,
            plan_cascade_desktop::commands::git::git_commit,
            plan_cascade_desktop::commands::git::git_amend_commit,
            plan_cascade_desktop::commands::git::git_discard_changes,
            plan_cascade_desktop::commands::git::git_diff_staged,
            plan_cascade_desktop::commands::git::git_diff_unstaged,
            plan_cascade_desktop::commands::git::git_diff_file,
            plan_cascade_desktop::commands::git::git_log,
            plan_cascade_desktop::commands::git::git_log_graph,
            plan_cascade_desktop::commands::git::git_list_branches,
            plan_cascade_desktop::commands::git::git_create_branch,
            plan_cascade_desktop::commands::git::git_delete_branch,
            plan_cascade_desktop::commands::git::git_checkout_branch,
            plan_cascade_desktop::commands::git::git_list_stashes,
            plan_cascade_desktop::commands::git::git_stash_save,
            plan_cascade_desktop::commands::git::git_stash_pop,
            plan_cascade_desktop::commands::git::git_stash_drop,
            plan_cascade_desktop::commands::git::git_get_merge_state,
            plan_cascade_desktop::commands::git::git_get_conflict_files,
            plan_cascade_desktop::commands::git::git_resolve_conflict,
            plan_cascade_desktop::commands::git::git_fetch,
            plan_cascade_desktop::commands::git::git_pull,
            plan_cascade_desktop::commands::git::git_push,
            plan_cascade_desktop::commands::git::git_get_remotes,
            plan_cascade_desktop::commands::git::git_merge_branch,
            plan_cascade_desktop::commands::git::git_merge_abort,
            plan_cascade_desktop::commands::git::git_merge_continue,
            plan_cascade_desktop::commands::git::git_rename_branch,
            plan_cascade_desktop::commands::git::git_list_remote_branches,
            plan_cascade_desktop::commands::git::git_read_file_content,
            plan_cascade_desktop::commands::git::git_parse_file_conflicts,
            plan_cascade_desktop::commands::git::git_resolve_file_and_stage,
            plan_cascade_desktop::commands::git::git_generate_commit_message,
            plan_cascade_desktop::commands::git::git_review_diff,
            plan_cascade_desktop::commands::git::git_resolve_conflict_ai,
            plan_cascade_desktop::commands::git::git_summarize_commit,
            plan_cascade_desktop::commands::git::git_check_llm_available,
            plan_cascade_desktop::commands::git::git_configure_llm,
            // Browser Automation commands
            plan_cascade_desktop::commands::browser::execute_browser_action,
            plan_cascade_desktop::commands::browser::get_browser_status,
            // A2A Remote Agent commands
            plan_cascade_desktop::commands::a2a::discover_a2a_agent,
            plan_cascade_desktop::commands::a2a::list_a2a_agents,
            plan_cascade_desktop::commands::a2a::register_a2a_agent,
            plan_cascade_desktop::commands::a2a::remove_a2a_agent,
            // File Change Tracking commands
            plan_cascade_desktop::commands::file_changes::init_file_change_tracker,
            plan_cascade_desktop::commands::file_changes::advance_turn_index,
            plan_cascade_desktop::commands::file_changes::get_file_changes_by_turn,
            plan_cascade_desktop::commands::file_changes::get_file_change_diff,
            plan_cascade_desktop::commands::file_changes::restore_files_to_turn,
            plan_cascade_desktop::commands::file_changes::truncate_changes_from_turn,
            plan_cascade_desktop::commands::file_changes::restore_single_file,
            // Permission commands
            plan_cascade_desktop::commands::permissions::set_session_permission_level,
            plan_cascade_desktop::commands::permissions::get_session_permission_level,
            plan_cascade_desktop::commands::permissions::respond_tool_permission,
            // Artifact commands
            plan_cascade_desktop::commands::artifacts::artifact_save,
            plan_cascade_desktop::commands::artifacts::artifact_load,
            plan_cascade_desktop::commands::artifacts::artifact_list,
            plan_cascade_desktop::commands::artifacts::artifact_versions,
            plan_cascade_desktop::commands::artifacts::artifact_delete,
        ])
        .setup(|app| {
            #[cfg(debug_assertions)]
            {
                let window = app.get_webview_window("main").unwrap();
                window.open_devtools();
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
