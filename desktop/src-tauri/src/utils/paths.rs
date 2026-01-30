//! Cross-Platform Path Utilities
//!
//! Functions for resolving application directories across platforms.
//! Handles ~/.claude/projects/, ~/.plan-cascade/, etc.

use std::path::PathBuf;

use crate::utils::error::{AppError, AppResult};

/// Get the user's home directory
pub fn home_dir() -> AppResult<PathBuf> {
    dirs::home_dir().ok_or_else(|| AppError::config("Could not determine home directory"))
}

/// Get the Claude projects directory (~/.claude/projects/)
pub fn claude_projects_dir() -> AppResult<PathBuf> {
    Ok(home_dir()?.join(".claude").join("projects"))
}

/// Get the Plan Cascade directory (~/.plan-cascade/)
pub fn plan_cascade_dir() -> AppResult<PathBuf> {
    Ok(home_dir()?.join(".plan-cascade"))
}

/// Get the config file path (~/.plan-cascade/config.json)
pub fn config_path() -> AppResult<PathBuf> {
    Ok(plan_cascade_dir()?.join("config.json"))
}

/// Get the database file path (~/.plan-cascade/data.db)
pub fn database_path() -> AppResult<PathBuf> {
    Ok(plan_cascade_dir()?.join("data.db"))
}

/// Get the agents directory (~/.plan-cascade/agents/)
pub fn agents_dir() -> AppResult<PathBuf> {
    Ok(plan_cascade_dir()?.join("agents"))
}

/// Ensure a directory exists, creating it if necessary
pub fn ensure_dir(path: &PathBuf) -> AppResult<()> {
    if !path.exists() {
        std::fs::create_dir_all(path)?;
    }
    Ok(())
}

/// Get the Claude projects directory, creating if it doesn't exist
pub fn ensure_claude_projects_dir() -> AppResult<PathBuf> {
    let path = claude_projects_dir()?;
    ensure_dir(&path)?;
    Ok(path)
}

/// Get the Plan Cascade directory, creating if it doesn't exist
pub fn ensure_plan_cascade_dir() -> AppResult<PathBuf> {
    let path = plan_cascade_dir()?;
    ensure_dir(&path)?;
    Ok(path)
}

/// Get the agents directory, creating if it doesn't exist
pub fn ensure_agents_dir() -> AppResult<PathBuf> {
    let path = agents_dir()?;
    ensure_dir(&path)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_home_dir() {
        let home = home_dir();
        assert!(home.is_ok());
        assert!(home.unwrap().exists());
    }

    #[test]
    fn test_plan_cascade_dir() {
        let dir = plan_cascade_dir();
        assert!(dir.is_ok());
        let path = dir.unwrap();
        assert!(path.to_string_lossy().contains(".plan-cascade"));
    }

    #[test]
    fn test_config_path() {
        let path = config_path();
        assert!(path.is_ok());
        assert!(path.unwrap().to_string_lossy().contains("config.json"));
    }

    #[test]
    fn test_database_path() {
        let path = database_path();
        assert!(path.is_ok());
        assert!(path.unwrap().to_string_lossy().contains("data.db"));
    }
}
