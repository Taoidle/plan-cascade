// Plan Cascade Desktop - Tauri Application Entry Point
// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use plan_cascade_desktop::state::AppState;

use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(AppState::new())
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
            // Markdown commands
            plan_cascade_desktop::commands::markdown::scan_claude_md,
            plan_cascade_desktop::commands::markdown::read_claude_md,
            plan_cascade_desktop::commands::markdown::save_claude_md,
            plan_cascade_desktop::commands::markdown::create_claude_md,
            plan_cascade_desktop::commands::markdown::get_claude_md_metadata,
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
