use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::UpdaterExt;
use url::Url;

use crate::models::response::CommandResponse;

const UPDATE_PROGRESS_EVENT: &str = "app-update-progress";
const DEFAULT_RESTART_REQUIRED: bool = true;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UpdateChannel {
    Stable,
    Beta,
    Alpha,
}

impl UpdateChannel {
    fn pointer_release_tag(self) -> &'static str {
        match self {
            Self::Stable => "desktop-stable",
            Self::Beta => "desktop-beta",
            Self::Alpha => "desktop-alpha",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppUpdateInfo {
    pub current_version: String,
    pub available: bool,
    pub target_version: Option<String>,
    pub channel: UpdateChannel,
    pub published_at: Option<i64>,
    pub notes: Option<String>,
    pub manifest_url: String,
    pub download_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppUpdateInstallResult {
    pub installed: bool,
    pub version: Option<String>,
    pub restart_required: bool,
    pub channel: UpdateChannel,
    pub manifest_url: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AppUpdateProgressStage {
    Started,
    Downloading,
    Verifying,
    Finished,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppUpdateProgressEvent {
    pub channel: UpdateChannel,
    pub stage: AppUpdateProgressStage,
    pub version: Option<String>,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub message: Option<String>,
}

fn updater_pubkey() -> Result<String, String> {
    option_env!("PLAN_CASCADE_UPDATER_PUBKEY")
        .or(option_env!("TAURI_UPDATER_PUBKEY"))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| "Updater public key is not configured".to_string())
}

fn updater_base_url() -> Result<String, String> {
    if let Some(override_url) = option_env!("PLAN_CASCADE_UPDATER_BASE_URL")
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Ok(override_url.trim_end_matches('/').to_string());
    }

    let repository = env!("CARGO_PKG_REPOSITORY").trim_end_matches('/');
    let slug = repository
        .strip_prefix("https://github.com/")
        .or_else(|| repository.strip_prefix("http://github.com/"))
        .ok_or_else(|| format!("Unsupported repository URL for updater: {repository}"))?;
    Ok(format!("https://github.com/{slug}/releases/download"))
}

fn manifest_url(channel: UpdateChannel) -> Result<String, String> {
    Ok(format!(
        "{}/{}/latest.json",
        updater_base_url()?,
        channel.pointer_release_tag()
    ))
}

fn emit_update_progress(
    app: &AppHandle,
    channel: UpdateChannel,
    version: Option<&str>,
    stage: AppUpdateProgressStage,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
    message: Option<String>,
) {
    if let Err(error) = app.emit(
        UPDATE_PROGRESS_EVENT,
        AppUpdateProgressEvent {
            channel,
            stage,
            version: version.map(str::to_string),
            downloaded_bytes,
            total_bytes,
            message,
        },
    ) {
        tracing::debug!(%error, "Failed to emit app update progress event");
    }
}

async fn query_update(
    app: &AppHandle,
    channel: UpdateChannel,
) -> Result<(tauri_plugin_updater::Update, String), String> {
    let manifest_url = manifest_url(channel)?;
    let pubkey = updater_pubkey()?;
    let endpoint = Url::parse(&manifest_url).map_err(|error| error.to_string())?;
    let updater = app
        .updater_builder()
        .pubkey(pubkey)
        .endpoints(vec![endpoint])
        .map_err(|error| error.to_string())?
        .build()
        .map_err(|error| error.to_string())?;

    let update = updater
        .check()
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "No update available".to_string())?;

    Ok((update, manifest_url))
}

#[tauri::command]
pub async fn check_app_update(
    app: AppHandle,
    channel: UpdateChannel,
) -> Result<CommandResponse<AppUpdateInfo>, String> {
    let current_version = app.package_info().version.to_string();
    let manifest_url = manifest_url(channel)?;
    let pubkey = updater_pubkey()?;
    let endpoint = Url::parse(&manifest_url).map_err(|error| error.to_string())?;
    let updater = app
        .updater_builder()
        .pubkey(pubkey)
        .endpoints(vec![endpoint])
        .map_err(|error| error.to_string())?
        .build()
        .map_err(|error| error.to_string())?;

    match updater.check().await.map_err(|error| error.to_string())? {
        Some(update) => Ok(CommandResponse::ok(AppUpdateInfo {
            current_version,
            available: true,
            target_version: Some(update.version.clone()),
            channel,
            published_at: update.date.map(|date| date.unix_timestamp()),
            notes: update.body.clone(),
            manifest_url,
            download_url: Some(update.download_url.to_string()),
        })),
        None => Ok(CommandResponse::ok(AppUpdateInfo {
            current_version,
            available: false,
            target_version: None,
            channel,
            published_at: None,
            notes: None,
            manifest_url,
            download_url: None,
        })),
    }
}

#[tauri::command]
pub async fn download_and_install_app_update(
    app: AppHandle,
    channel: UpdateChannel,
    expected_version: Option<String>,
) -> Result<CommandResponse<AppUpdateInstallResult>, String> {
    let (update, manifest_url) = query_update(&app, channel).await?;
    if let Some(expected_version) = expected_version.as_deref() {
        if update.version != expected_version {
            return Ok(CommandResponse::err(format!(
                "Expected update version {expected_version}, but latest available version is {}",
                update.version
            )));
        }
    }

    let version = update.version.clone();
    emit_update_progress(
        &app,
        channel,
        Some(&version),
        AppUpdateProgressStage::Started,
        0,
        None,
        Some("Starting update download".to_string()),
    );

    let downloaded_bytes = Arc::new(AtomicU64::new(0));
    let download_progress = Arc::clone(&downloaded_bytes);
    let verify_progress = Arc::clone(&downloaded_bytes);
    let install_result = update
        .download_and_install(
            |chunk_length, total| {
                let current = download_progress.fetch_add(chunk_length as u64, Ordering::Relaxed)
                    + chunk_length as u64;
                emit_update_progress(
                    &app,
                    channel,
                    Some(&version),
                    AppUpdateProgressStage::Downloading,
                    current,
                    total,
                    None,
                );
            },
            || {
                let current = verify_progress.load(Ordering::Relaxed);
                emit_update_progress(
                    &app,
                    channel,
                    Some(&version),
                    AppUpdateProgressStage::Verifying,
                    current,
                    None,
                    Some("Verifying and installing update".to_string()),
                );
            },
        )
        .await;

    match install_result {
        Ok(()) => {
            emit_update_progress(
                &app,
                channel,
                Some(&version),
                AppUpdateProgressStage::Finished,
                downloaded_bytes.load(Ordering::Relaxed),
                None,
                Some("Update downloaded and installed".to_string()),
            );
            Ok(CommandResponse::ok(AppUpdateInstallResult {
                installed: true,
                version: Some(version),
                restart_required: DEFAULT_RESTART_REQUIRED,
                channel,
                manifest_url,
            }))
        }
        Err(error) => {
            emit_update_progress(
                &app,
                channel,
                Some(&version),
                AppUpdateProgressStage::Failed,
                downloaded_bytes.load(Ordering::Relaxed),
                None,
                Some(error.to_string()),
            );
            Ok(CommandResponse::err(error.to_string()))
        }
    }
}

#[tauri::command]
pub fn restart_app_for_update(app: AppHandle) -> CommandResponse<bool> {
    app.request_restart();
    CommandResponse::ok(true)
}
