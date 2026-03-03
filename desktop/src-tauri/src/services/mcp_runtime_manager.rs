//! MCP Runtime Manager
//!
//! Detects runtime availability (Node/uv/Python/Docker) and provides
//! cross-platform repair/install command plans.

use std::process::Command;
use std::time::Duration;

use chrono::Utc;

use crate::models::{McpRuntimeInfo, McpRuntimeKind, McpRuntimeRepairResult};
use crate::storage::database::Database;
use crate::utils::error::AppResult;

/// Runtime manager with persistence for inventory snapshots.
#[derive(Clone)]
pub struct McpRuntimeManager {
    db: Database,
}

impl McpRuntimeManager {
    /// Create with default database.
    pub fn new() -> AppResult<Self> {
        Ok(Self {
            db: Database::new()?,
        })
    }

    /// Create with injected database.
    pub fn with_database(db: Database) -> Self {
        Self { db }
    }

    /// Detect and persist all supported runtimes.
    pub fn refresh_inventory(&self) -> AppResult<Vec<McpRuntimeInfo>> {
        let kinds = [
            McpRuntimeKind::Node,
            McpRuntimeKind::Uv,
            McpRuntimeKind::Python,
            McpRuntimeKind::Docker,
        ];

        for kind in kinds {
            let info = self.detect_runtime(kind.clone());
            let key = runtime_key(&kind);
            self.db.upsert_mcp_runtime_inventory(&key, &info)?;
        }

        self.db.list_mcp_runtime_inventory()
    }

    /// List runtime inventory from DB; if empty, auto-refresh first.
    pub fn list_inventory(&self) -> AppResult<Vec<McpRuntimeInfo>> {
        let current = self.db.list_mcp_runtime_inventory()?;
        if current.is_empty() {
            return self.refresh_inventory();
        }
        Ok(current)
    }

    /// Detect single runtime availability.
    pub fn detect_runtime(&self, runtime: McpRuntimeKind) -> McpRuntimeInfo {
        let now = Utc::now().to_rfc3339();
        let (binary, version_args) = match runtime {
            McpRuntimeKind::Node => ("node", vec!["--version"]),
            McpRuntimeKind::Uv => ("uv", vec!["--version"]),
            McpRuntimeKind::Python => ("python3", vec!["--version"]),
            // docker info verifies daemon reachability, not just CLI presence.
            McpRuntimeKind::Docker => ("docker", vec!["info", "--format", "{{.ServerVersion}}"]),
        };

        let found = find_binary(binary).or_else(|| {
            if runtime == McpRuntimeKind::Python {
                find_binary("python")
            } else {
                None
            }
        });

        if found.is_none() {
            return McpRuntimeInfo {
                runtime,
                version: None,
                path: None,
                source: None,
                managed: false,
                healthy: false,
                last_error: Some("runtime_not_found".to_string()),
                last_checked: Some(now),
            };
        }

        let path = found.unwrap_or_default();
        let mut cmd = Command::new(&path);
        cmd.args(&version_args);
        let mut last_error = None;
        let mut version = None;
        let healthy = match cmd.output() {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
                let value = if stdout.is_empty() { stderr } else { stdout };
                version = extract_version(&value).or_else(|| Some("unknown".to_string()));
                match (&runtime, version.as_deref()) {
                    (McpRuntimeKind::Node, Some(v)) => version_at_least(v, "20.0.0"),
                    (McpRuntimeKind::Uv, Some(v)) => version_at_least(v, "0.4.0"),
                    (McpRuntimeKind::Python, Some(v)) => version_at_least(v, "3.10.0"),
                    (McpRuntimeKind::Docker, Some(_)) => true,
                    _ => false,
                }
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
                let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
                let message = if stderr.is_empty() { stdout } else { stderr };
                if !message.is_empty() {
                    last_error = Some(message);
                } else {
                    last_error = Some("runtime_probe_failed".to_string());
                }
                false
            }
            Err(e) => {
                last_error = Some(format!("runtime_probe_failed: {}", e));
                false
            }
        };

        if !healthy && last_error.is_none() {
            if let Some(min) = runtime_min_version(&runtime) {
                if let Some(actual) = version.as_deref() {
                    last_error = Some(format!(
                        "version_too_low: need >= {}, found {}",
                        min, actual
                    ));
                }
            }
        }

        McpRuntimeInfo {
            runtime,
            version,
            path: Some(path),
            source: Some("system".to_string()),
            managed: false,
            healthy,
            last_error,
            last_checked: Some(now),
        }
    }

    /// Suggest package-manager install commands.
    pub fn install_commands_for(&self, runtime: &McpRuntimeKind) -> Vec<String> {
        if cfg!(target_os = "macos") {
            if find_binary("brew").is_none() {
                return Vec::new();
            }
            let pkg = match runtime {
                McpRuntimeKind::Node => "node",
                McpRuntimeKind::Uv => "uv",
                McpRuntimeKind::Python => "python@3.11",
                McpRuntimeKind::Docker => "docker",
            };
            return vec![format!("brew install {}", pkg)];
        }

        if cfg!(target_os = "windows") {
            let winget_pkg = match runtime {
                McpRuntimeKind::Node => "OpenJS.NodeJS.LTS",
                McpRuntimeKind::Uv => "astral-sh.uv",
                McpRuntimeKind::Python => "Python.Python.3.11",
                McpRuntimeKind::Docker => "Docker.DockerDesktop",
            };
            let choco_pkg = match runtime {
                McpRuntimeKind::Node => "nodejs-lts",
                McpRuntimeKind::Uv => "uv",
                McpRuntimeKind::Python => "python",
                McpRuntimeKind::Docker => "docker-desktop",
            };
            let mut plan = Vec::new();
            if find_binary("winget").is_some() {
                plan.push(format!(
                    "winget install --id {} --accept-package-agreements --accept-source-agreements",
                    winget_pkg
                ));
            }
            if find_binary("choco").is_some() {
                plan.push(format!("choco install {} -y", choco_pkg));
            }
            return plan;
        }

        let mut plan = Vec::new();
        if find_binary("apt-get").is_some() {
            let pkg = match runtime {
                McpRuntimeKind::Docker => "docker.io",
                _ => linux_package(runtime),
            };
            plan.push(format!("apt-get update && apt-get install -y {}", pkg));
        }
        if find_binary("dnf").is_some() {
            plan.push(format!("dnf install -y {}", linux_package(runtime)));
        }
        if find_binary("yum").is_some() {
            plan.push(format!("yum install -y {}", linux_package(runtime)));
        }
        if find_binary("pacman").is_some() {
            plan.push(format!("pacman -S --noconfirm {}", linux_package(runtime)));
        }
        if find_binary("zypper").is_some() {
            plan.push(format!(
                "zypper --non-interactive install {}",
                linux_package(runtime)
            ));
        }
        plan
    }

    /// Build an elevated command wrapper for current OS.
    pub fn elevated_wrapper(&self, raw_cmd: &str) -> (String, Vec<String>) {
        if cfg!(target_os = "windows") {
            return (
                "powershell".to_string(),
                vec![
                    "-NoProfile".to_string(),
                    "-Command".to_string(),
                    format!(
                        "Start-Process PowerShell -Verb RunAs -ArgumentList '-NoProfile -Command \"{}\"'",
                        raw_cmd.replace('"', "\\\"")
                    ),
                ],
            );
        }

        if cfg!(target_os = "macos") {
            return (
                "osascript".to_string(),
                vec![
                    "-e".to_string(),
                    format!(
                        "do shell script \"{}\" with administrator privileges",
                        raw_cmd.replace('"', "\\\"")
                    ),
                ],
            );
        }

        (
            "pkexec".to_string(),
            vec!["sh".to_string(), "-lc".to_string(), raw_cmd.to_string()],
        )
    }

    /// Best-effort runtime repair (executes install command with elevation).
    pub async fn repair_runtime(
        &self,
        runtime: McpRuntimeKind,
    ) -> AppResult<McpRuntimeRepairResult> {
        let detected = self.detect_runtime(runtime.clone());
        if detected.healthy {
            let key = runtime_key(&runtime);
            self.db.upsert_mcp_runtime_inventory(&key, &detected)?;
            return Ok(McpRuntimeRepairResult {
                runtime,
                status: "already_healthy".to_string(),
                message: "Runtime already available".to_string(),
            });
        }

        let plan = self.install_commands_for(&runtime);
        if plan.is_empty() {
            let guidance = if cfg!(target_os = "windows") {
                "No supported package manager found (expected winget or choco)"
            } else if cfg!(target_os = "macos") {
                "Homebrew is required to install missing runtimes"
            } else {
                "No supported package manager found (expected apt/dnf/yum/pacman/zypper)"
            };
            return Ok(McpRuntimeRepairResult {
                runtime,
                status: "runtime_unavailable".to_string(),
                message: guidance.to_string(),
            });
        }

        if cfg!(target_os = "linux") && find_binary("pkexec").is_none() {
            return Ok(McpRuntimeRepairResult {
                runtime,
                status: "runtime_unavailable".to_string(),
                message: "pkexec is required for elevated runtime install on Linux".to_string(),
            });
        }

        let mut failures = Vec::new();
        for raw_cmd in plan {
            let (program, args) = self.elevated_wrapper(&raw_cmd);
            let attempt = tokio::time::timeout(
                Duration::from_secs(180),
                tokio::process::Command::new(&program).args(&args).output(),
            )
            .await;
            match attempt {
                Ok(Ok(output)) => {
                    if !output.status.success() {
                        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                        failures.push(if stderr.is_empty() {
                            format!("{} => {}", raw_cmd, stdout)
                        } else {
                            format!("{} => {}", raw_cmd, stderr)
                        });
                        continue;
                    }
                }
                Ok(Err(e)) => {
                    failures.push(format!("{} => launch_failed: {}", raw_cmd, e));
                    continue;
                }
                Err(_) => {
                    failures.push(format!("{} => timeout", raw_cmd));
                    continue;
                }
            }

            let refreshed = self.detect_runtime(runtime.clone());
            let key = runtime_key(&runtime);
            self.db.upsert_mcp_runtime_inventory(&key, &refreshed)?;
            if refreshed.healthy {
                return Ok(McpRuntimeRepairResult {
                    runtime,
                    status: "repaired".to_string(),
                    message: "Runtime installed successfully".to_string(),
                });
            }
        }

        let refreshed = self.detect_runtime(runtime.clone());
        let key = runtime_key(&runtime);
        self.db.upsert_mcp_runtime_inventory(&key, &refreshed)?;
        Ok(McpRuntimeRepairResult {
            runtime,
            status: "failed".to_string(),
            message: if failures.is_empty() {
                "Runtime installation failed after install attempts".to_string()
            } else {
                format!("Runtime installation failed: {}", failures.join(" | "))
            },
        })
    }
}

fn runtime_key(runtime: &McpRuntimeKind) -> String {
    match runtime {
        McpRuntimeKind::Node => "node".to_string(),
        McpRuntimeKind::Uv => "uv".to_string(),
        McpRuntimeKind::Python => "python".to_string(),
        McpRuntimeKind::Docker => "docker".to_string(),
    }
}

fn find_binary(binary: &str) -> Option<String> {
    let checker = if cfg!(target_os = "windows") {
        "where"
    } else {
        "which"
    };
    Command::new(checker)
        .arg(binary)
        .output()
        .ok()
        .and_then(|out| {
            if out.status.success() {
                String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .next()
                    .map(|s| s.trim().to_string())
            } else {
                None
            }
        })
}

fn extract_version(raw: &str) -> Option<String> {
    let mut started = false;
    let mut chars = String::new();
    for ch in raw.chars() {
        if ch.is_ascii_digit() {
            started = true;
            chars.push(ch);
            continue;
        }
        if started && (ch == '.' || ch == '-') {
            chars.push(ch);
            continue;
        }
        if started {
            break;
        }
    }
    if chars.is_empty() {
        None
    } else {
        Some(chars)
    }
}

fn version_at_least(actual: &str, minimum: &str) -> bool {
    let actual_parts = version_parts(actual);
    let min_parts = version_parts(minimum);
    let width = std::cmp::max(actual_parts.len(), min_parts.len());
    for idx in 0..width {
        let a = *actual_parts.get(idx).unwrap_or(&0);
        let b = *min_parts.get(idx).unwrap_or(&0);
        if a > b {
            return true;
        }
        if a < b {
            return false;
        }
    }
    true
}

fn version_parts(raw: &str) -> Vec<u64> {
    let mut current = String::new();
    let mut out = Vec::new();
    for ch in raw.chars() {
        if ch.is_ascii_digit() {
            current.push(ch);
            continue;
        }
        if !current.is_empty() {
            if let Ok(value) = current.parse::<u64>() {
                out.push(value);
            }
            current.clear();
        }
    }
    if !current.is_empty() {
        if let Ok(value) = current.parse::<u64>() {
            out.push(value);
        }
    }
    out
}

fn runtime_min_version(runtime: &McpRuntimeKind) -> Option<&'static str> {
    match runtime {
        McpRuntimeKind::Node => Some("20.0.0"),
        McpRuntimeKind::Uv => Some("0.4.0"),
        McpRuntimeKind::Python => Some("3.10.0"),
        McpRuntimeKind::Docker => None,
    }
}

fn linux_package(runtime: &McpRuntimeKind) -> &'static str {
    match runtime {
        McpRuntimeKind::Node => "nodejs",
        McpRuntimeKind::Uv => "uv",
        McpRuntimeKind::Python => "python3",
        McpRuntimeKind::Docker => "docker",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_at_least() {
        assert!(version_at_least("20.1.0", "20.0.0"));
        assert!(version_at_least("0.4.0", "0.4.0"));
        assert!(version_at_least("3.11.2", "3.10.0"));
        assert!(!version_at_least("19.9.1", "20.0.0"));
        assert!(!version_at_least("0.3.9", "0.4.0"));
        assert!(!version_at_least("3.9.18", "3.10.0"));
    }

    #[test]
    fn test_extract_version() {
        assert_eq!(extract_version("v20.18.0"), Some("20.18.0".to_string()));
        assert_eq!(extract_version("Python 3.11.7"), Some("3.11.7".to_string()));
        assert_eq!(
            extract_version("uv 0.5.13 (Homebrew)"),
            Some("0.5.13".to_string())
        );
        assert_eq!(extract_version("no version"), None);
    }
}
