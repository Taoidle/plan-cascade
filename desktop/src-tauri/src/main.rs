// Plan Cascade Desktop - Tauri Application Entry Point
// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use plan_cascade_desktop::state::AppState;
use plan_cascade_desktop::commands::claude_code::ClaudeCodeState;
use plan_cascade_desktop::commands::analytics::AnalyticsState;
use plan_cascade_desktop::commands::quality_gates::QualityGatesState;
use plan_cascade_desktop::commands::worktree::WorktreeState;
use plan_cascade_desktop::commands::standalone::StandaloneState;
use plan_cascade_desktop::commands::spec_interview::SpecInterviewState;

use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(AppState::new())
        .manage(ClaudeCodeState::new())
        .manage(AnalyticsState::new())
        .manage(QualityGatesState::new())
        .manage(WorktreeState::new())
        .manage(StandaloneState::new())
        .manage(SpecInterviewState::new())
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
            plan_cascade_desktop::commands::standalone::configure_provider,
            plan_cascade_desktop::commands::standalone::check_provider_health,
            plan_cascade_desktop::commands::standalone::execute_standalone,
            plan_cascade_desktop::commands::standalone::get_usage_stats,
            // Session-based standalone commands
            plan_cascade_desktop::commands::standalone::execute_standalone_with_session,
            plan_cascade_desktop::commands::standalone::cancel_standalone_execution,
            plan_cascade_desktop::commands::standalone::get_standalone_status,
            plan_cascade_desktop::commands::standalone::get_standalone_progress,
            plan_cascade_desktop::commands::standalone::resume_standalone_execution,
            plan_cascade_desktop::commands::standalone::get_standalone_session,
            plan_cascade_desktop::commands::standalone::list_standalone_sessions,
            plan_cascade_desktop::commands::standalone::delete_standalone_session,
            plan_cascade_desktop::commands::standalone::cleanup_standalone_sessions,
            // Strategy commands
            plan_cascade_desktop::commands::strategy::analyze_task_strategy,
            plan_cascade_desktop::commands::strategy::get_strategy_options,
            plan_cascade_desktop::commands::strategy::classify_intent,
            plan_cascade_desktop::commands::strategy::override_task_strategy,
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
