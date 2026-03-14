use tauri::{AppHandle, State};

use crate::app_shell::{
    hide_main_window_to_background, quit_application, show_main_window, AppShellState,
};

#[tauri::command]
pub async fn app_quit(app: AppHandle, shell_state: State<'_, AppShellState>) -> Result<(), String> {
    quit_application(&app, shell_state.inner());
    Ok(())
}

#[tauri::command]
pub async fn app_show_main_window(app: AppHandle) -> Result<(), String> {
    show_main_window(&app).map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn app_hide_main_window_to_background(app: AppHandle) -> Result<(), String> {
    hide_main_window_to_background(&app).map_err(|error| error.to_string())
}
