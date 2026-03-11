//! MCP Installer Orchestrator
//!
//! Implements discover-install lifecycle:
//! precheck -> runtime repair -> package prepare -> write config ->
//! verify protocol -> commit metadata -> rollback on failure.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::Utc;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use crate::models::{
    CreateMcpServerRequest, McpCatalogItem, McpInstallPhase, McpInstallPreview, McpInstallRecord,
    McpInstallRequest, McpInstallResult, McpInstallStatus, McpInstallStrategy,
    McpInstallStrategyKind, McpRuntimeKind, McpServerType,
};
use crate::services::mcp::McpService;
use crate::services::mcp_catalog::McpCatalogService;
use crate::services::mcp_runtime_manager::McpRuntimeManager;
use crate::storage::database::Database;
use crate::utils::error::{AppError, AppResult};
use crate::utils::configure_background_process;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct McpInstallProgressEvent {
    pub job_id: String,
    pub phase: McpInstallPhase,
    pub progress: f64,
    pub status: String,
    pub message: String,
    pub server_id: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct McpInstallLogEvent {
    pub job_id: String,
    pub phase: McpInstallPhase,
    pub level: String,
    pub message: String,
}

/// Installer orchestrator service.
pub struct McpInstallerService {
    db: Database,
    catalog: McpCatalogService,
    runtime: McpRuntimeManager,
    mcp: McpService,
}

impl McpInstallerService {
    /// Create service with default dependencies.
    pub fn new() -> AppResult<Self> {
        let db = Database::new()?;
        Ok(Self {
            db: db.clone(),
            catalog: McpCatalogService::with_database(db.clone()),
            runtime: McpRuntimeManager::with_database(db.clone()),
            mcp: McpService::with_database(db),
        })
    }

    /// Preview installation for a catalog item.
    pub fn preview_install(
        &self,
        item_id: &str,
        preferred_strategy: Option<&str>,
    ) -> AppResult<McpInstallPreview> {
        let item = self.catalog.get_item(item_id)?;
        let strategy = select_strategy(&item, preferred_strategy)?;
        let inventory = self.runtime.refresh_inventory()?;
        let mut missing = Vec::new();
        let mut install_commands = Vec::new();

        for req in &strategy.requirements {
            let matched = inventory
                .iter()
                .find(|runtime| runtime.runtime == req.runtime);
            if matched.map(|runtime| runtime.healthy).unwrap_or(false) {
                continue;
            }
            if !req.optional {
                missing.push(req.runtime.clone());
                install_commands.extend(self.runtime.install_commands_for(&req.runtime));
            }
        }

        let risk_flags = build_install_risk_flags(&item, strategy, !install_commands.is_empty());

        Ok(McpInstallPreview {
            item_id: item.id.clone(),
            selected_strategy: strategy.id.clone(),
            missing_runtimes: missing,
            install_commands,
            required_secrets: item.secrets_schema.clone(),
            risk_flags,
        })
    }

    /// Perform installation flow and return result.
    pub async fn install_catalog_item(
        &self,
        request: McpInstallRequest,
        app: Option<&AppHandle>,
    ) -> AppResult<McpInstallResult> {
        let job_id = Uuid::new_v4().to_string();
        self.install_catalog_item_with_job(job_id, request, app)
            .await
    }

    /// Retry install job from saved request snapshot.
    pub async fn retry_install(
        &self,
        job_id: &str,
        app: Option<&AppHandle>,
    ) -> AppResult<McpInstallResult> {
        let job = self
            .db
            .get_mcp_install_job(job_id)?
            .ok_or_else(|| AppError::not_found(format!("Install job not found: {}", job_id)))?;
        let logs_json = job.logs_json.ok_or_else(|| {
            AppError::validation("Install job has no request snapshot".to_string())
        })?;
        let payload: serde_json::Value = serde_json::from_str(&logs_json)?;
        let req_val = payload.get("request").cloned().ok_or_else(|| {
            AppError::validation("Install job snapshot missing request".to_string())
        })?;
        let request: McpInstallRequest = serde_json::from_value(req_val)?;
        self.install_catalog_item_with_job(Uuid::new_v4().to_string(), request, app)
            .await
    }

    /// Get managed install record by server id.
    pub fn get_install_record(&self, server_id: &str) -> AppResult<Option<McpInstallRecord>> {
        self.db.get_mcp_install_record(server_id)
    }

    async fn install_catalog_item_with_job(
        &self,
        job_id: String,
        request: McpInstallRequest,
        app: Option<&AppHandle>,
    ) -> AppResult<McpInstallResult> {
        let mut phase = McpInstallPhase::Precheck;
        let mut created_server_id: Option<String> = None;
        let mut created_wrapper: Option<PathBuf> = None;
        let mut event_logs: Vec<String> = vec!["install_started".to_string()];

        let job_snapshot = serde_json::json!({ "request": request.clone() });
        self.persist_job(
            &job_id,
            &request.item_id,
            None,
            &phase,
            0.05,
            McpInstallStatus::Running,
            None,
            None,
            Some(&job_snapshot),
        )?;
        emit_progress(
            app,
            McpInstallProgressEvent {
                job_id: job_id.clone(),
                phase: phase.clone(),
                progress: 0.05,
                status: "running".to_string(),
                message: "Running preflight checks".to_string(),
                server_id: None,
            },
        );

        let item = self.catalog.get_item(&request.item_id)?;
        let strategy = select_strategy(&item, request.selected_strategy.as_deref())?;
        let preview = self.preview_install(&item.id, Some(&strategy.id))?;
        if let Err(e) = validate_required_secrets(&item, &request) {
            return self
                .finalize_failure(
                    &job_id,
                    &request.item_id,
                    phase.clone(),
                    e.to_string(),
                    None,
                    &created_wrapper,
                    &request,
                )
                .await;
        }
        event_logs.push(format!("selected_strategy:{}", strategy.id));

        phase = McpInstallPhase::InstallRuntime;
        self.persist_job(
            &job_id,
            &request.item_id,
            None,
            &phase,
            0.20,
            McpInstallStatus::Running,
            None,
            None,
            Some(&serde_json::json!({ "request": request.clone(), "events": event_logs.clone() })),
        )?;
        emit_progress(
            app,
            McpInstallProgressEvent {
                job_id: job_id.clone(),
                phase: phase.clone(),
                progress: 0.20,
                status: "running".to_string(),
                message: "Checking and repairing runtimes".to_string(),
                server_id: None,
            },
        );

        for runtime in preview.missing_runtimes {
            let result = self.runtime.repair_runtime(runtime.clone()).await?;
            event_logs.push(format!(
                "runtime_{}:{}",
                runtime_name(&runtime),
                result.status
            ));
            emit_log(
                app,
                McpInstallLogEvent {
                    job_id: job_id.clone(),
                    phase: phase.clone(),
                    level: if result.status == "repaired" || result.status == "already_healthy" {
                        "info".to_string()
                    } else {
                        "warn".to_string()
                    },
                    message: format!("{}: {}", runtime_name(&runtime), result.message),
                },
            );
            if result.status == "failed" || result.status == "runtime_unavailable" {
                return self
                    .finalize_failure(
                        &job_id,
                        &request.item_id,
                        phase.clone(),
                        format!("runtime: {}", result.message),
                        created_server_id.as_deref(),
                        &created_wrapper,
                        &request,
                    )
                    .await;
            }
        }

        let runtime_snapshot = self.runtime.refresh_inventory()?;

        phase = McpInstallPhase::InstallPackage;
        self.persist_job(
            &job_id,
            &request.item_id,
            None,
            &phase,
            0.35,
            McpInstallStatus::Running,
            None,
            None,
            Some(&serde_json::json!({ "request": request.clone(), "events": event_logs.clone() })),
        )?;
        emit_progress(
            app,
            McpInstallProgressEvent {
                job_id: job_id.clone(),
                phase: phase.clone(),
                progress: 0.35,
                status: "running".to_string(),
                message: "Preparing launcher".to_string(),
                server_id: None,
            },
        );

        let launcher = match prepare_launcher(&item, &strategy, &job_id, app).await {
            Ok(config) => config,
            Err(e) => {
                return self
                    .finalize_failure(
                        &job_id,
                        &request.item_id,
                        phase,
                        format!(
                            "install_package:{}: {}",
                            classify_install_error(&e.to_string()),
                            e
                        ),
                        created_server_id.as_deref(),
                        &created_wrapper,
                        &request,
                    )
                    .await;
            }
        };
        created_wrapper = launcher.wrapper_path.clone();

        phase = McpInstallPhase::WriteConfig;
        self.persist_job(
            &job_id,
            &request.item_id,
            None,
            &phase,
            0.50,
            McpInstallStatus::Running,
            None,
            None,
            Some(&serde_json::json!({ "request": request.clone(), "events": event_logs.clone() })),
        )?;

        let create_request = build_server_request(&item, &strategy, &request, &launcher)?;
        let mut server = match self.mcp.add_server(create_request) {
            Ok(server) => server,
            Err(e) => {
                return self
                    .finalize_failure(
                        &job_id,
                        &request.item_id,
                        phase,
                        e.to_string(),
                        None,
                        &created_wrapper,
                        &request,
                    )
                    .await;
            }
        };
        created_server_id = Some(server.id.clone());

        // Mark managed metadata on the stored server.
        server.managed_install = true;
        server.catalog_item_id = Some(item.id.clone());
        server.trust_level = Some(item.trust_level.clone());
        self.db.update_mcp_server(&server)?;

        phase = McpInstallPhase::VerifyProtocol;
        self.persist_job(
            &job_id,
            &request.item_id,
            created_server_id.as_deref(),
            &phase,
            0.70,
            McpInstallStatus::Running,
            None,
            None,
            Some(&serde_json::json!({ "request": request.clone(), "events": event_logs.clone() })),
        )?;
        emit_progress(
            app,
            McpInstallProgressEvent {
                job_id: job_id.clone(),
                phase: phase.clone(),
                progress: 0.70,
                status: "running".to_string(),
                message: "Verifying MCP protocol handshake".to_string(),
                server_id: created_server_id.clone(),
            },
        );

        if let Err(e) = self.mcp.test_server(&server.id).await {
            return self
                .finalize_failure(
                    &job_id,
                    &request.item_id,
                    phase,
                    e.to_string(),
                    created_server_id.as_deref(),
                    &created_wrapper,
                    &request,
                )
                .await;
        }

        phase = McpInstallPhase::Commit;
        let runtime_snapshot_json = serde_json::to_value(&runtime_snapshot).ok();
        let package_lock_json = launcher.package_lock_json.clone().or_else(|| {
            Some(serde_json::json!({
                "strategy": strategy.id,
                "launcher": launcher.command,
                "args": launcher.args,
                "generated_at": Utc::now().to_rfc3339(),
            }))
        });
        self.db.upsert_mcp_install_record(&McpInstallRecord {
            server_id: server.id.clone(),
            catalog_item_id: item.id.clone(),
            catalog_version: Some("builtin-v1".to_string()),
            strategy_id: strategy.id.clone(),
            trust_level: item.trust_level.clone(),
            package_lock_json,
            runtime_snapshot_json,
            installed_at: None,
            updated_at: None,
        })?;

        self.persist_job(
            &job_id,
            &request.item_id,
            Some(&server.id),
            &phase,
            1.0,
            McpInstallStatus::Success,
            None,
            None,
            Some(&serde_json::json!({ "request": request.clone(), "events": event_logs.clone() })),
        )?;
        emit_progress(
            app,
            McpInstallProgressEvent {
                job_id: job_id.clone(),
                phase: phase.clone(),
                progress: 1.0,
                status: "success".to_string(),
                message: "MCP server installed".to_string(),
                server_id: Some(server.id.clone()),
            },
        );

        Ok(McpInstallResult {
            job_id,
            server_id: Some(server.id),
            phase,
            status: McpInstallStatus::Success,
            diagnostics: None,
        })
    }

    async fn finalize_failure(
        &self,
        job_id: &str,
        item_id: &str,
        failed_phase: McpInstallPhase,
        error: String,
        server_id: Option<&str>,
        wrapper_path: &Option<PathBuf>,
        request: &McpInstallRequest,
    ) -> AppResult<McpInstallResult> {
        if let Some(server_id) = server_id {
            let _ = self.db.delete_mcp_install_record(server_id);
            let _ = self.mcp.remove_server(server_id);
        }
        if let Some(wrapper) = wrapper_path {
            let _ = std::fs::remove_file(wrapper);
        }

        self.persist_job(
            job_id,
            item_id,
            server_id,
            &McpInstallPhase::Rollback,
            1.0,
            McpInstallStatus::Failed,
            Some(classify_error_class(&error)),
            Some(&error),
            Some(&serde_json::json!({ "request": request.clone() })),
        )?;

        Ok(McpInstallResult {
            job_id: job_id.to_string(),
            server_id: None,
            phase: failed_phase,
            status: McpInstallStatus::Failed,
            diagnostics: Some(error),
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn persist_job(
        &self,
        job_id: &str,
        item_id: &str,
        server_id: Option<&str>,
        phase: &McpInstallPhase,
        progress: f64,
        status: McpInstallStatus,
        error_class: Option<&str>,
        error_message: Option<&str>,
        logs: Option<&serde_json::Value>,
    ) -> AppResult<()> {
        let status_str = match status {
            McpInstallStatus::Running => "running",
            McpInstallStatus::Success => "success",
            McpInstallStatus::Failed => "failed",
        };
        let logs_json = logs.map(|value| value.to_string());
        self.db.upsert_mcp_install_job(
            job_id,
            item_id,
            server_id,
            phase_name(phase),
            progress,
            status_str,
            error_class,
            error_message,
            logs_json.as_deref(),
        )
    }
}

#[derive(Debug, Clone)]
struct LauncherConfig {
    command: String,
    args: Vec<String>,
    env: HashMap<String, String>,
    wrapper_path: Option<PathBuf>,
    package_lock_json: Option<serde_json::Value>,
}

fn select_strategy<'a>(
    item: &'a McpCatalogItem,
    preferred_strategy: Option<&str>,
) -> AppResult<&'a McpInstallStrategy> {
    if item.strategies.is_empty() {
        return Err(AppError::validation(format!(
            "Catalog item '{}' has no install strategies",
            item.id
        )));
    }

    if let Some(preferred) = preferred_strategy {
        if let Some(strategy) = item.strategies.iter().find(|s| s.id == preferred) {
            return Ok(strategy);
        }
    }

    item.strategies
        .iter()
        .min_by_key(|strategy| strategy.priority)
        .ok_or_else(|| AppError::validation("No strategy available".to_string()))
}

fn build_server_request(
    item: &McpCatalogItem,
    strategy: &McpInstallStrategy,
    request: &McpInstallRequest,
    launcher: &LauncherConfig,
) -> AppResult<CreateMcpServerRequest> {
    let recipe_type = strategy
        .recipe
        .get("server_type")
        .and_then(|v| v.as_str())
        .unwrap_or("stdio");

    if recipe_type == "stream_http" {
        let raw_url = strategy
            .recipe
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::validation("stream_http strategy missing url".to_string()))?;
        let url = apply_placeholders(raw_url, &request.secrets);

        let mut headers = HashMap::new();
        if let Some(recipe_headers) = strategy.recipe.get("headers").and_then(|v| v.as_object()) {
            for (key, value) in recipe_headers {
                if let Some(value) = value.as_str() {
                    headers.insert(key.to_string(), apply_placeholders(value, &request.secrets));
                }
            }
        }

        return Ok(CreateMcpServerRequest {
            name: request.server_alias.clone(),
            server_type: McpServerType::StreamHttp,
            command: None,
            args: Some(Vec::new()),
            env: Some(HashMap::new()),
            url: Some(url),
            headers: Some(headers),
            auto_connect: Some(request.auto_connect.unwrap_or(true)),
        });
    }

    let mut env = launcher.env.clone();
    for secret in &item.secrets_schema {
        if let Some(value) = request.secrets.get(&secret.key) {
            env.insert(secret.key.clone(), value.clone());
        }
    }
    if let Some(mode) = request.oauth_mode.as_ref() {
        env.insert("PLAN_CASCADE_OAUTH_MODE".to_string(), mode.clone());
    }

    Ok(CreateMcpServerRequest {
        name: request.server_alias.clone(),
        server_type: McpServerType::Stdio,
        command: Some(launcher.command.clone()),
        args: Some(launcher.args.clone()),
        env: Some(env),
        url: None,
        headers: Some(HashMap::new()),
        auto_connect: Some(request.auto_connect.unwrap_or(true)),
    })
}

async fn prepare_launcher(
    item: &McpCatalogItem,
    strategy: &McpInstallStrategy,
    job_id: &str,
    app: Option<&AppHandle>,
) -> AppResult<LauncherConfig> {
    let mut env = HashMap::new();
    match strategy.kind {
        McpInstallStrategyKind::StreamHttpApiKey
        | McpInstallStrategyKind::StreamHttpApiKeyOptional => Ok(LauncherConfig {
            command: String::new(),
            args: Vec::new(),
            env,
            wrapper_path: None,
            package_lock_json: None,
        }),
        McpInstallStrategyKind::UvTool => {
            let package = strategy
                .recipe
                .get("package")
                .and_then(|v| v.as_str())
                .unwrap_or(&item.id);
            let mut launch_args = Vec::new();
            if let Some(extra_args) = strategy.recipe.get("args").and_then(|v| v.as_array()) {
                launch_args.extend(
                    extra_args
                        .iter()
                        .filter_map(|v| v.as_str().map(ToString::to_string)),
                );
            }
            prepare_uv_tool(item, job_id, package, launch_args, app).await
        }
        McpInstallStrategyKind::PythonVenv => {
            let package = strategy
                .recipe
                .get("package")
                .and_then(|v| v.as_str())
                .unwrap_or(&item.id);
            let module = strategy
                .recipe
                .get("module")
                .and_then(|v| v.as_str())
                .map(ToString::to_string)
                .unwrap_or_else(|| package_to_python_module(package));
            let mut launch_args = Vec::new();
            if let Some(extra_args) = strategy.recipe.get("args").and_then(|v| v.as_array()) {
                launch_args.extend(
                    extra_args
                        .iter()
                        .filter_map(|v| v.as_str().map(ToString::to_string)),
                );
            }
            prepare_python_venv(item, job_id, package, &module, launch_args, app).await
        }
        McpInstallStrategyKind::NodeManagedPkg => {
            let package = strategy
                .recipe
                .get("package")
                .and_then(|v| v.as_str())
                .unwrap_or(&item.id);
            let mut launch_args = Vec::new();
            if let Some(extra_args) = strategy.recipe.get("args").and_then(|v| v.as_array()) {
                launch_args.extend(
                    extra_args
                        .iter()
                        .filter_map(|v| v.as_str().map(ToString::to_string)),
                );
            }
            prepare_node_managed_pkg(item, job_id, package, None, launch_args, app).await
        }
        McpInstallStrategyKind::Docker => {
            let image = strategy
                .recipe
                .get("image")
                .and_then(|v| v.as_str())
                .ok_or_else(|| AppError::validation("Docker strategy missing image".to_string()))?;
            Ok(LauncherConfig {
                command: "docker".to_string(),
                args: vec![
                    "run".to_string(),
                    "--rm".to_string(),
                    "-i".to_string(),
                    image.to_string(),
                ],
                env,
                wrapper_path: None,
                package_lock_json: None,
            })
        }
        McpInstallStrategyKind::OauthBridgeMcpRemote => {
            let target_url = strategy
                .recipe
                .get("target_url")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    AppError::validation("OAuth bridge strategy missing target_url".to_string())
                })?;
            env.insert("MCP_REMOTE_TARGET".to_string(), target_url.to_string());
            emit_oauth_state(
                app,
                job_id,
                "pending_authorization",
                Some("OAuth bridge is prepared. Authorization will continue in provider flow."),
            );
            let bridge_package = strategy
                .recipe
                .get("bridge_package")
                .and_then(|v| v.as_str())
                .unwrap_or("mcp-remote");
            let mut launcher = prepare_node_managed_pkg(
                item,
                job_id,
                bridge_package,
                Some("mcp-remote"),
                vec![target_url.to_string()],
                app,
            )
            .await?;
            launcher.env.extend(env);
            Ok(launcher)
        }
        McpInstallStrategyKind::GoBinary => {
            let binary = strategy
                .recipe
                .get("binary")
                .and_then(|v| v.as_str())
                .unwrap_or("mcp-server");
            Ok(LauncherConfig {
                command: binary.to_string(),
                args: Vec::new(),
                env,
                wrapper_path: None,
                package_lock_json: None,
            })
        }
    }
}

async fn prepare_node_managed_pkg(
    item: &McpCatalogItem,
    job_id: &str,
    package_spec: &str,
    forced_bin: Option<&str>,
    launch_args: Vec<String>,
    app: Option<&AppHandle>,
) -> AppResult<LauncherConfig> {
    let tool_dir = managed_node_tool_dir(item, package_spec, job_id);
    std::fs::create_dir_all(&tool_dir)?;

    let package_json = tool_dir.join("package.json");
    if !package_json.exists() {
        let args = vec!["init".to_string(), "-y".to_string()];
        let _ = run_command("npm", &args, Some(&tool_dir), None, 60).await?;
    }

    emit_log(
        app,
        McpInstallLogEvent {
            job_id: job_id.to_string(),
            phase: McpInstallPhase::InstallPackage,
            level: "info".to_string(),
            message: format!("Installing npm package {}", package_spec),
        },
    );

    let install_args = vec![
        "install".to_string(),
        "--no-audit".to_string(),
        "--no-fund".to_string(),
        "--save-exact".to_string(),
        package_spec.to_string(),
    ];
    let _ = run_command("npm", &install_args, Some(&tool_dir), None, 600).await?;

    let package_name = package_spec_to_name(package_spec);
    let inferred_bin = forced_bin
        .map(ToString::to_string)
        .or_else(|| npm_infer_bin_name(&tool_dir, &package_name))
        .unwrap_or_else(|| package_name_to_bin_name(&package_name));

    let bin_path = npm_bin_path(&tool_dir, &inferred_bin);
    let (command, args) = if bin_path.exists() {
        let mut args = Vec::new();
        args.extend(launch_args);
        (bin_path.to_string_lossy().to_string(), args)
    } else {
        let mut args = vec![
            "exec".to_string(),
            "--prefix".to_string(),
            tool_dir.to_string_lossy().to_string(),
            "--".to_string(),
            inferred_bin.clone(),
        ];
        args.extend(launch_args);
        ("npm".to_string(), args)
    };

    let wrapper_path = Some(write_wrapper_script(
        job_id,
        &command,
        &args,
        &HashMap::new(),
    )?);
    let launcher_command = command.clone();
    let launcher_args = args.clone();
    let lock_json = Some(serde_json::json!({
        "kind": "node_managed_pkg",
        "package_spec": package_spec,
        "package_name": package_name,
        "tool_dir": tool_dir.to_string_lossy().to_string(),
        "wrapper_path": wrapper_path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string()),
        "launcher_command": launcher_command,
        "launcher_args": launcher_args,
        "package_lock": read_json_file(tool_dir.join("package-lock.json")),
    }));

    Ok(LauncherConfig {
        command: wrapper_path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or(command),
        args: Vec::new(),
        env: HashMap::new(),
        wrapper_path,
        package_lock_json: lock_json,
    })
}

async fn prepare_python_venv(
    item: &McpCatalogItem,
    job_id: &str,
    package_spec: &str,
    module: &str,
    launch_args: Vec<String>,
    app: Option<&AppHandle>,
) -> AppResult<LauncherConfig> {
    let tool_dir = managed_python_tool_dir(item, package_spec, job_id);
    std::fs::create_dir_all(&tool_dir)?;
    let venv_dir = tool_dir.join("venv");

    if !python_exe(&venv_dir).exists() {
        let python = if cfg!(target_os = "windows") {
            "python"
        } else {
            "python3"
        };
        let args = vec![
            "-m".to_string(),
            "venv".to_string(),
            venv_dir.to_string_lossy().to_string(),
        ];
        let _ = run_command(python, &args, None, None, 180).await?;
    }

    let pip = pip_exe(&venv_dir).to_string_lossy().to_string();
    emit_log(
        app,
        McpInstallLogEvent {
            job_id: job_id.to_string(),
            phase: McpInstallPhase::InstallPackage,
            level: "info".to_string(),
            message: format!("Installing Python package {}", package_spec),
        },
    );
    let install_args = vec!["install".to_string(), package_spec.to_string()];
    let _ = run_command(&pip, &install_args, None, None, 600).await?;

    let freeze_args = vec!["freeze".to_string()];
    let freeze_output = run_command(&pip, &freeze_args, None, None, 60).await?;
    let python_cmd = python_exe(&venv_dir).to_string_lossy().to_string();
    let mut args = vec!["-m".to_string(), module.to_string()];
    args.extend(launch_args);
    let wrapper_path = Some(write_wrapper_script(
        job_id,
        &python_cmd,
        &args,
        &HashMap::new(),
    )?);
    let launcher_command = python_cmd.clone();
    let launcher_args = args.clone();
    let lock_json = Some(serde_json::json!({
        "kind": "python_venv",
        "package_spec": package_spec,
        "module": module,
        "tool_dir": tool_dir.to_string_lossy().to_string(),
        "venv_dir": venv_dir.to_string_lossy().to_string(),
        "wrapper_path": wrapper_path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string()),
        "launcher_command": launcher_command,
        "launcher_args": launcher_args,
        "freeze": freeze_output.stdout.lines().collect::<Vec<_>>(),
    }));

    Ok(LauncherConfig {
        command: wrapper_path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or(python_cmd),
        args: Vec::new(),
        env: HashMap::new(),
        wrapper_path,
        package_lock_json: lock_json,
    })
}

async fn prepare_uv_tool(
    item: &McpCatalogItem,
    job_id: &str,
    package_spec: &str,
    launch_args: Vec<String>,
    app: Option<&AppHandle>,
) -> AppResult<LauncherConfig> {
    let tool_dir = managed_uv_tool_dir(item, package_spec, job_id);
    std::fs::create_dir_all(&tool_dir)?;
    emit_log(
        app,
        McpInstallLogEvent {
            job_id: job_id.to_string(),
            phase: McpInstallPhase::InstallPackage,
            level: "info".to_string(),
            message: format!("Installing uv tool {}", package_spec),
        },
    );
    let args = vec![
        "tool".to_string(),
        "install".to_string(),
        "--tool-dir".to_string(),
        tool_dir.to_string_lossy().to_string(),
        "--force".to_string(),
        package_spec.to_string(),
    ];
    let result = run_command("uv", &args, None, None, 600).await;

    let package_name = package_spec_to_name(package_spec);
    let bin_name = package_name_to_bin_name(&package_name);
    let uv_bin = uv_tool_bin_path(&tool_dir, &bin_name);

    let (command, args) = if result.is_ok() && uv_bin.exists() {
        let mut args = Vec::new();
        args.extend(launch_args);
        (uv_bin.to_string_lossy().to_string(), args)
    } else {
        let mut args = vec![package_spec.to_string()];
        args.extend(launch_args);
        ("uvx".to_string(), args)
    };

    let wrapper_path = Some(write_wrapper_script(
        job_id,
        &command,
        &args,
        &HashMap::new(),
    )?);
    let lock_json = Some(serde_json::json!({
        "kind": "uv_tool",
        "package_spec": package_spec,
        "tool_dir": tool_dir.to_string_lossy().to_string(),
        "wrapper_path": wrapper_path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string()),
        "launcher_command": command.clone(),
        "launcher_args": args.clone(),
        "installed_with_uv_tool": result.is_ok(),
    }));

    Ok(LauncherConfig {
        command: wrapper_path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or(command),
        args: Vec::new(),
        env: HashMap::new(),
        wrapper_path,
        package_lock_json: lock_json,
    })
}

#[derive(Debug)]
struct CommandExecOutput {
    stdout: String,
}

async fn run_command(
    program: &str,
    args: &[String],
    cwd: Option<&Path>,
    env: Option<&HashMap<String, String>>,
    timeout_secs: u64,
) -> AppResult<CommandExecOutput> {
    let mut command = tokio::process::Command::new(program);
    command.args(args);
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    if let Some(env) = env {
        for (key, value) in env {
            command.env(key, value);
        }
    }
    configure_background_process(&mut command);

    let output = tokio::time::timeout(Duration::from_secs(timeout_secs), command.output())
        .await
        .map_err(|_| {
            AppError::command(format!("Command timed out: {} {}", program, args.join(" ")))
        })?
        .map_err(|e| AppError::command(format!("Failed to execute {}: {}", program, e)))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        let message = if stderr.trim().is_empty() {
            stdout.trim().to_string()
        } else {
            stderr.trim().to_string()
        };
        return Err(AppError::command(format!(
            "{} {} failed: {}",
            program,
            args.join(" "),
            message
        )));
    }

    Ok(CommandExecOutput { stdout })
}

fn managed_root() -> AppResult<PathBuf> {
    let home = dirs::home_dir()
        .ok_or_else(|| AppError::config("Cannot determine home directory for MCP managed tools"))?;
    Ok(home.join(".plan-cascade"))
}

fn managed_node_tool_dir(item: &McpCatalogItem, package_spec: &str, install_id: &str) -> PathBuf {
    let package_name = package_spec_to_name(package_spec).replace('/', "__");
    managed_root()
        .unwrap_or_else(|_| PathBuf::from(".plan-cascade"))
        .join("mcp-tools")
        .join("node")
        .join(format!("{}__{}__{}", item.id, package_name, install_id))
}

fn managed_python_tool_dir(item: &McpCatalogItem, package_spec: &str, install_id: &str) -> PathBuf {
    let package_name = package_spec_to_name(package_spec).replace('/', "__");
    managed_root()
        .unwrap_or_else(|_| PathBuf::from(".plan-cascade"))
        .join("mcp-tools")
        .join("python")
        .join(format!("{}__{}__{}", item.id, package_name, install_id))
}

fn managed_uv_tool_dir(item: &McpCatalogItem, package_spec: &str, install_id: &str) -> PathBuf {
    managed_python_tool_dir(item, package_spec, install_id).join("uv-tools")
}

fn launcher_dir() -> AppResult<PathBuf> {
    Ok(managed_root()?.join("mcp-launchers"))
}

fn write_wrapper_script(
    job_id: &str,
    command: &str,
    args: &[String],
    env: &HashMap<String, String>,
) -> AppResult<PathBuf> {
    let dir = launcher_dir()?;
    std::fs::create_dir_all(&dir)?;
    let is_windows = cfg!(target_os = "windows");
    let extension = if is_windows { "cmd" } else { "sh" };
    let path = dir.join(format!("mcp-{}.{}", job_id, extension));

    let mut content = String::new();
    if is_windows {
        content.push_str("@echo off\n");
        for (key, value) in env {
            content.push_str(&format!("set {}={}\n", key, value.replace('\n', " ")));
        }
        content.push_str(&format!("\"{}\"", command));
        for arg in args {
            content.push(' ');
            content.push_str(&windows_quote(arg));
        }
        content.push_str(" %*\n");
    } else {
        content.push_str("#!/usr/bin/env sh\n");
        content.push_str("set -e\n");
        for (key, value) in env {
            content.push_str(&format!(
                "export {}='{}'\n",
                key,
                value.replace('\'', "'\"'\"'")
            ));
        }
        content.push_str(&shell_quote(command));
        for arg in args {
            content.push(' ');
            content.push_str(&shell_quote(arg));
        }
        content.push_str(" \"$@\"\n");
    }

    std::fs::write(&path, content)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms)?;
    }
    Ok(path)
}

fn package_spec_to_name(spec: &str) -> String {
    if spec.starts_with('@') {
        let mut parts = spec.splitn(3, '/');
        let scope = parts.next().unwrap_or_default();
        let rest = parts.next().unwrap_or_default();
        let name = rest.split('@').next().unwrap_or(rest);
        if scope.is_empty() || name.is_empty() {
            spec.to_string()
        } else {
            format!("{}/{}", scope, name)
        }
    } else {
        spec.split('@').next().unwrap_or(spec).to_string()
    }
}

fn package_name_to_bin_name(package_name: &str) -> String {
    package_name
        .split('/')
        .next_back()
        .unwrap_or(package_name)
        .to_string()
}

fn npm_infer_bin_name(tool_dir: &Path, package_name: &str) -> Option<String> {
    let package_json_path = tool_dir
        .join("node_modules")
        .join(package_name)
        .join("package.json");
    let value = read_json_file(package_json_path)?;
    let bin = value.get("bin")?;
    if bin.as_str().is_some() {
        return Some(package_name_to_bin_name(package_name));
    }
    if let Some(map) = bin.as_object() {
        if let Some((key, _)) = map.iter().next() {
            return Some(key.to_string());
        }
    }
    None
}

fn read_json_file(path: PathBuf) -> Option<serde_json::Value> {
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn npm_bin_path(tool_dir: &Path, bin_name: &str) -> PathBuf {
    if cfg!(target_os = "windows") {
        tool_dir
            .join("node_modules")
            .join(".bin")
            .join(format!("{}.cmd", bin_name))
    } else {
        tool_dir.join("node_modules").join(".bin").join(bin_name)
    }
}

fn uv_tool_bin_path(tool_dir: &Path, bin_name: &str) -> PathBuf {
    if cfg!(target_os = "windows") {
        tool_dir.join("Scripts").join(format!("{}.exe", bin_name))
    } else {
        tool_dir.join("bin").join(bin_name)
    }
}

fn python_exe(venv_dir: &Path) -> PathBuf {
    if cfg!(target_os = "windows") {
        venv_dir.join("Scripts").join("python.exe")
    } else {
        venv_dir.join("bin").join("python")
    }
}

fn pip_exe(venv_dir: &Path) -> PathBuf {
    if cfg!(target_os = "windows") {
        venv_dir.join("Scripts").join("pip.exe")
    } else {
        venv_dir.join("bin").join("pip")
    }
}

fn package_to_python_module(package_spec: &str) -> String {
    package_spec_to_name(package_spec)
        .split('/')
        .next_back()
        .unwrap_or(package_spec)
        .replace('-', "_")
}

fn windows_quote(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\\\""))
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn phase_name(phase: &McpInstallPhase) -> &'static str {
    match phase {
        McpInstallPhase::Precheck => "PRECHECK",
        McpInstallPhase::Elevate => "ELEVATE",
        McpInstallPhase::InstallRuntime => "INSTALL_RUNTIME",
        McpInstallPhase::InstallPackage => "INSTALL_PACKAGE",
        McpInstallPhase::WriteConfig => "WRITE_CONFIG",
        McpInstallPhase::VerifyProtocol => "VERIFY_PROTOCOL",
        McpInstallPhase::AutoConnect => "AUTO_CONNECT",
        McpInstallPhase::Commit => "COMMIT",
        McpInstallPhase::Rollback => "ROLLBACK",
    }
}

fn classify_error_class(error: &str) -> &'static str {
    let lower = error.to_lowercase();
    if lower.contains("auth") || lower.contains("401") || lower.contains("403") {
        "auth"
    } else if lower.contains("protocol")
        || lower.contains("initialize")
        || lower.contains("tools/list")
    {
        "protocol"
    } else if lower.contains("schema") || lower.contains("validation") {
        "schema"
    } else {
        "transport"
    }
}

fn classify_install_error(error: &str) -> &'static str {
    let lower = error.to_lowercase();
    if lower.contains("unauthorized")
        || lower.contains("forbidden")
        || lower.contains("401")
        || lower.contains("403")
        || lower.contains("auth")
    {
        "auth"
    } else if lower.contains("timeout")
        || lower.contains("network")
        || lower.contains("econn")
        || lower.contains("dns")
    {
        "network"
    } else if lower.contains("version")
        || lower.contains("not found")
        || lower.contains("no matching")
    {
        "version"
    } else if lower.contains("python")
        || lower.contains("node")
        || lower.contains("uv")
        || lower.contains("npm")
        || lower.contains("pip")
    {
        "toolchain"
    } else {
        "install"
    }
}

fn apply_placeholders(input: &str, values: &HashMap<String, String>) -> String {
    let mut output = input.to_string();
    for (key, value) in values {
        output = output.replace(&format!("{{{{{}}}}}", key), value);
    }
    output
}

fn build_install_risk_flags(
    item: &McpCatalogItem,
    strategy: &McpInstallStrategy,
    runtime_install_commands_present: bool,
) -> Vec<String> {
    let mut flags = Vec::new();
    match item.trust_level {
        crate::models::McpCatalogTrustLevel::Official => {}
        crate::models::McpCatalogTrustLevel::Verified => {
            flags.push("review_commands".to_string());
        }
        crate::models::McpCatalogTrustLevel::Community => {
            flags.push("community_caution".to_string());
            flags.push("community_item_confirmation_required".to_string());
            flags.push("review_commands".to_string());
        }
    }

    if strategy_contains_unpinned_artifact(strategy) {
        flags.push("unpinned_artifact".to_string());
    }
    if runtime_install_commands_present || strategy_requires_command_review(strategy) {
        flags.push("review_commands".to_string());
    }

    flags.sort();
    flags.dedup();
    flags
}

fn strategy_requires_command_review(strategy: &McpInstallStrategy) -> bool {
    matches!(
        strategy.kind,
        McpInstallStrategyKind::UvTool
            | McpInstallStrategyKind::PythonVenv
            | McpInstallStrategyKind::NodeManagedPkg
            | McpInstallStrategyKind::Docker
            | McpInstallStrategyKind::GoBinary
            | McpInstallStrategyKind::OauthBridgeMcpRemote
    )
}

fn strategy_contains_unpinned_artifact(strategy: &McpInstallStrategy) -> bool {
    if let Some(image) = strategy.recipe.get("image").and_then(|v| v.as_str()) {
        if docker_image_unpinned(image) {
            return true;
        }
    }
    if let Some(package) = strategy.recipe.get("package").and_then(|v| v.as_str()) {
        if package_spec_unpinned(package) {
            return true;
        }
    }
    if let Some(package) = strategy
        .recipe
        .get("bridge_package")
        .and_then(|v| v.as_str())
    {
        if package_spec_unpinned(package) {
            return true;
        }
    }
    false
}

fn docker_image_unpinned(image: &str) -> bool {
    let trimmed = image.trim();
    if trimmed.is_empty() {
        return true;
    }
    if trimmed.contains("@sha256:") {
        return false;
    }

    let last_segment = trimmed.rsplit('/').next().unwrap_or(trimmed);
    if let Some((_, tag)) = last_segment.split_once(':') {
        let normalized = tag.trim();
        return normalized.is_empty() || normalized.eq_ignore_ascii_case("latest");
    }
    true
}

fn package_spec_unpinned(spec: &str) -> bool {
    let trimmed = spec.trim();
    if trimmed.is_empty() {
        return true;
    }
    if trimmed.starts_with('@') {
        let scoped_part = &trimmed[1..];
        if let Some(relative_idx) = scoped_part.rfind('@') {
            let version = &scoped_part[(relative_idx + 1)..];
            return version.trim().is_empty() || version.eq_ignore_ascii_case("latest");
        }
        return true;
    }
    if let Some((_, version)) = trimmed.split_once('@') {
        return version.trim().is_empty() || version.eq_ignore_ascii_case("latest");
    }
    true
}

fn validate_required_secrets(item: &McpCatalogItem, request: &McpInstallRequest) -> AppResult<()> {
    let missing: Vec<String> = item
        .secrets_schema
        .iter()
        .filter(|field| field.required)
        .filter_map(|field| {
            let value = request
                .secrets
                .get(&field.key)
                .map(|v| v.trim())
                .unwrap_or("");
            if value.is_empty() {
                Some(field.key.clone())
            } else {
                None
            }
        })
        .collect();

    if missing.is_empty() {
        return Ok(());
    }

    Err(AppError::validation(format!(
        "required_secrets_missing: {}",
        missing.join(", ")
    )))
}

fn emit_progress(app: Option<&AppHandle>, event: McpInstallProgressEvent) {
    if let Some(app) = app {
        let _ = app.emit("mcp:install-progress", event);
    }
}

fn emit_log(app: Option<&AppHandle>, event: McpInstallLogEvent) {
    if let Some(app) = app {
        let _ = app.emit("mcp:install-log", event);
    }
}

fn emit_oauth_state(app: Option<&AppHandle>, job_id: &str, state: &str, message: Option<&str>) {
    if let Some(app) = app {
        let payload = serde_json::json!({
            "job_id": job_id,
            "state": state,
            "message": message,
        });
        let _ = app.emit("mcp:oauth-state", payload);
    }
}

fn runtime_name(kind: &McpRuntimeKind) -> &'static str {
    match kind {
        McpRuntimeKind::Node => "node",
        McpRuntimeKind::Uv => "uv",
        McpRuntimeKind::Python => "python",
        McpRuntimeKind::Docker => "docker",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        McpCatalogTrustLevel, McpInstallVerification, McpSecretSchemaField, RuntimeRequirement,
    };

    fn strategy_with_recipe(
        kind: McpInstallStrategyKind,
        recipe: serde_json::Value,
    ) -> McpInstallStrategy {
        McpInstallStrategy {
            id: "test".to_string(),
            kind,
            priority: 1,
            requirements: Vec::<RuntimeRequirement>::new(),
            recipe,
            verification: McpInstallVerification {
                require_initialize: true,
                require_tools_list: true,
            },
        }
    }

    #[test]
    fn detects_unpinned_artifacts_in_recipe() {
        let docker = strategy_with_recipe(
            McpInstallStrategyKind::Docker,
            serde_json::json!({ "image": "ghcr.io/example/tool:latest" }),
        );
        assert!(strategy_contains_unpinned_artifact(&docker));

        let pinned_docker = strategy_with_recipe(
            McpInstallStrategyKind::Docker,
            serde_json::json!({ "image": "ghcr.io/example/tool@sha256:abc123" }),
        );
        assert!(!strategy_contains_unpinned_artifact(&pinned_docker));

        let npm = strategy_with_recipe(
            McpInstallStrategyKind::NodeManagedPkg,
            serde_json::json!({ "package": "@scope/tool" }),
        );
        assert!(strategy_contains_unpinned_artifact(&npm));

        let pinned_npm = strategy_with_recipe(
            McpInstallStrategyKind::NodeManagedPkg,
            serde_json::json!({ "package": "@scope/tool@1.2.3" }),
        );
        assert!(!strategy_contains_unpinned_artifact(&pinned_npm));
    }

    #[test]
    fn required_secret_validation_reports_fields() {
        let item = McpCatalogItem {
            id: "item".to_string(),
            name: "Item".to_string(),
            vendor: "Vendor".to_string(),
            trust_level: McpCatalogTrustLevel::Official,
            tags: vec![],
            docs_url: None,
            maintained_by: None,
            os_support: vec![],
            strategies: vec![],
            secrets_schema: vec![
                McpSecretSchemaField {
                    key: "REQ_ONE".to_string(),
                    label: "Req One".to_string(),
                    required: true,
                    secret_type: None,
                },
                McpSecretSchemaField {
                    key: "OPTIONAL".to_string(),
                    label: "Optional".to_string(),
                    required: false,
                    secret_type: None,
                },
            ],
        };

        let request = McpInstallRequest {
            item_id: "item".to_string(),
            server_alias: "alias".to_string(),
            selected_strategy: None,
            secrets: HashMap::new(),
            oauth_mode: None,
            auto_connect: Some(true),
        };

        let err = validate_required_secrets(&item, &request).unwrap_err();
        assert!(err.to_string().contains("REQ_ONE"));
    }
}
