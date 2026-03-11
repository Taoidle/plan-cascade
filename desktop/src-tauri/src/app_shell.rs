use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;
use std::sync::Mutex;

use tauri::menu::MenuEvent;
use tauri::menu::MenuBuilder;
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, RunEvent, Runtime, State, WindowEvent};

use crate::models::settings::AppConfig;
use crate::storage::ConfigService;

pub const MAIN_WINDOW_LABEL: &str = "main";
const TRAY_ID: &str = "main-tray";
const TRAY_SHOW_ID: &str = "show_main";
const TRAY_HIDE_ID: &str = "hide_main";
const TRAY_QUIT_ID: &str = "quit_app";

pub struct AppShellState {
    close_to_background_enabled: AtomicBool,
    is_quitting: AtomicBool,
    tray_locale: Mutex<String>,
}

impl AppShellState {
    pub fn new(close_to_background_enabled: bool) -> Self {
        Self::with_tray_locale(close_to_background_enabled, "en".to_string())
    }

    fn with_tray_locale(close_to_background_enabled: bool, tray_locale: String) -> Self {
        Self {
            close_to_background_enabled: AtomicBool::new(close_to_background_enabled),
            is_quitting: AtomicBool::new(false),
            tray_locale: Mutex::new(tray_locale),
        }
    }

    pub fn from_disk() -> Self {
        let (enabled, locale) = ConfigService::new()
            .map(|service| {
                let config = service.get_config();
                (config.close_to_background_enabled, config.language.clone())
            })
            .unwrap_or_else(|_| (true, "en".to_string()));
        Self::with_tray_locale(enabled, locale)
    }

    pub fn close_to_background_enabled(&self) -> bool {
        self.close_to_background_enabled.load(Ordering::Relaxed)
    }

    pub fn set_close_to_background_enabled(&self, enabled: bool) {
        self.close_to_background_enabled
            .store(enabled, Ordering::Relaxed);
    }

    pub fn is_quitting(&self) -> bool {
        self.is_quitting.load(Ordering::Relaxed)
    }

    pub fn mark_quitting(&self) {
        self.is_quitting.store(true, Ordering::Relaxed);
    }

    fn should_refresh_tray<R: Runtime>(&self, app: &AppHandle<R>, locale: &str) -> bool {
        let current_locale = self.tray_locale.lock().unwrap();
        should_refresh_tray(&current_locale, locale, app.tray_by_id(TRAY_ID).is_some())
    }

    fn set_tray_locale(&self, locale: &str) {
        *self.tray_locale.lock().unwrap() = locale.to_string();
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BackgroundAction {
    AllowClose,
    Hide,
    Minimize,
}

fn background_action(close_to_background_enabled: bool) -> BackgroundAction {
    if !close_to_background_enabled {
        return BackgroundAction::AllowClose;
    }

    if cfg!(target_os = "linux") {
        BackgroundAction::Minimize
    } else {
        BackgroundAction::Hide
    }
}

#[derive(Clone, Copy)]
struct TrayLabels {
    tooltip: &'static str,
    show: &'static str,
    hide: &'static str,
    quit: &'static str,
}

fn tray_labels(locale: &str) -> TrayLabels {
    let normalized = locale.trim().to_ascii_lowercase();
    if normalized.starts_with("zh") {
        TrayLabels {
            tooltip: "Plan Cascade",
            show: "显示窗口",
            hide: "隐藏到后台",
            quit: "退出应用",
        }
    } else if normalized.starts_with("ja") {
        TrayLabels {
            tooltip: "Plan Cascade",
            show: "ウィンドウを表示",
            hide: "バックグラウンドへ隠す",
            quit: "アプリを終了",
        }
    } else {
        TrayLabels {
            tooltip: "Plan Cascade",
            show: "Show Window",
            hide: "Hide to Background",
            quit: "Quit Application",
        }
    }
}

fn create_tray<R: Runtime>(app: &AppHandle<R>, locale: &str) -> tauri::Result<()> {
    let labels = tray_labels(locale);
    let menu = MenuBuilder::new(app)
        .text(TRAY_SHOW_ID, labels.show)
        .text(TRAY_HIDE_ID, labels.hide)
        .separator()
        .text(TRAY_QUIT_ID, labels.quit)
        .build()?;

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .tooltip(labels.tooltip)
        .show_menu_on_left_click(false)
        .on_menu_event(|app: &AppHandle<R>, event: MenuEvent| match event.id().as_ref() {
            TRAY_SHOW_ID => {
                let _ = show_main_window(app);
            }
            TRAY_HIDE_ID => {
                let _ = hide_main_window_to_background(app);
            }
            TRAY_QUIT_ID => {
                let shell_state: State<'_, AppShellState> = app.state();
                quit_application(app, shell_state.inner());
            }
            _ => {}
        })
        .on_tray_icon_event(|tray: &TrayIcon<R>, event: TrayIconEvent| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                let _ = show_main_window(&app);
            }
        });

    if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder.icon(icon);
    }

    builder.build(app)?;
    Ok(())
}

pub fn init_tray<R: Runtime>(app: &AppHandle<R>, locale: &str) -> tauri::Result<()> {
    if app.tray_by_id(TRAY_ID).is_some() {
        return Ok(());
    }
    create_tray(app, locale)
}

fn refresh_tray<R: Runtime>(app: &AppHandle<R>, locale: &str) -> tauri::Result<()> {
    let _ = app.remove_tray_by_id(TRAY_ID);
    create_tray(app, locale)
}

fn refresh_tray_on_main_thread<R: Runtime>(app: &AppHandle<R>, locale: &str) -> tauri::Result<()> {
    let app_handle = app.clone();
    let locale = locale.to_string();
    let (tx, rx) = channel();

    app.run_on_main_thread(move || {
        let _ = tx.send(refresh_tray(&app_handle, &locale));
    })?;

    rx.recv().map_err(|_| tauri::Error::FailedToReceiveMessage)?
}

fn should_refresh_tray(current_locale: &str, next_locale: &str, tray_exists: bool) -> bool {
    !tray_exists || current_locale != next_locale
}

pub fn show_main_window<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        if window.is_minimized().unwrap_or(false) {
            let _ = window.unminimize();
        }
        window.show()?;
        window.set_focus()?;
    }
    Ok(())
}

pub fn hide_main_window<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        window.hide()?;
    }
    Ok(())
}

pub fn minimize_main_window<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        window.minimize()?;
    }
    Ok(())
}

pub fn hide_main_window_to_background<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    match background_action(true) {
        BackgroundAction::Hide => hide_main_window(app),
        BackgroundAction::Minimize => minimize_main_window(app),
        BackgroundAction::AllowClose => Ok(()),
    }
}

pub fn quit_application<R: Runtime>(app: &AppHandle<R>, shell_state: &AppShellState) {
    shell_state.mark_quitting();
    app.exit(0);
}

pub fn apply_runtime_preferences<R: Runtime>(
    app: &AppHandle<R>,
    shell_state: &AppShellState,
    config: &AppConfig,
) -> tauri::Result<()> {
    shell_state.set_close_to_background_enabled(config.close_to_background_enabled);
    if shell_state.should_refresh_tray(app, &config.language) {
        refresh_tray_on_main_thread(app, &config.language)?;
        shell_state.set_tray_locale(&config.language);
    }
    Ok(())
}

pub fn handle_window_event<R: Runtime>(window: &tauri::Window<R>, event: &WindowEvent) {
    if window.label() != MAIN_WINDOW_LABEL {
        return;
    }

    if let WindowEvent::CloseRequested { api, .. } = event {
        let app = window.app_handle();
        let shell_state: State<'_, AppShellState> = app.state();
        if shell_state.is_quitting() {
            return;
        }

        match background_action(shell_state.close_to_background_enabled()) {
            BackgroundAction::AllowClose => {}
            BackgroundAction::Hide => {
                api.prevent_close();
                let _ = hide_main_window(&app);
            }
            BackgroundAction::Minimize => {
                api.prevent_close();
                let _ = minimize_main_window(&app);
            }
        }
    }
}

pub fn handle_run_event<R: Runtime>(app: &AppHandle<R>, event: &RunEvent) {
    let shell_state: State<'_, AppShellState> = app.state();

    match event {
        RunEvent::ExitRequested { .. } => {
            shell_state.mark_quitting();
        }
        #[cfg(target_os = "macos")]
        RunEvent::Reopen {
            has_visible_windows,
            ..
        } => {
            if !has_visible_windows {
                let _ = show_main_window(app);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::{background_action, should_refresh_tray, BackgroundAction};

    #[test]
    fn background_action_allows_close_when_disabled() {
        assert_eq!(
            background_action(false),
            BackgroundAction::AllowClose
        );
    }

    #[test]
    fn background_action_uses_platform_default_when_enabled() {
        let expected = if cfg!(target_os = "linux") {
            BackgroundAction::Minimize
        } else {
            BackgroundAction::Hide
        };
        assert_eq!(background_action(true), expected);
    }

    #[test]
    fn tray_refresh_is_skipped_when_locale_unchanged_and_tray_exists() {
        assert!(!should_refresh_tray("en", "en", true));
    }

    #[test]
    fn tray_refresh_runs_when_locale_changes() {
        assert!(should_refresh_tray("en", "zh", true));
    }

    #[test]
    fn tray_refresh_runs_when_tray_is_missing() {
        assert!(should_refresh_tray("en", "en", false));
    }
}
