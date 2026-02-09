//! Planning Config
//!
//! Handles reading and writing of .planning-config.json files in worktrees.

use std::fs;
use std::path::{Path, PathBuf};

use crate::models::worktree::{PlanningConfig, PlanningPhase};
use crate::utils::error::{AppError, AppResult};

/// Name of the planning config file
pub const PLANNING_CONFIG_FILE: &str = ".planning-config.json";

/// Files to exclude from commits (planning artifacts)
pub const PLANNING_FILES: &[&str] = &[
    ".planning-config.json",
    "prd.json",
    "progress.txt",
    "findings.md",
    "task_plan.md",
];

/// Service for managing planning configuration files
#[derive(Debug, Default)]
pub struct PlanningConfigService;

impl PlanningConfigService {
    /// Create a new config service
    pub fn new() -> Self {
        Self
    }

    /// Get the path to the planning config file in a worktree
    pub fn config_path(&self, worktree_path: &Path) -> PathBuf {
        worktree_path.join(PLANNING_CONFIG_FILE)
    }

    /// Check if a planning config exists in the given directory
    pub fn exists(&self, worktree_path: &Path) -> bool {
        self.config_path(worktree_path).exists()
    }

    /// Read the planning config from a worktree
    pub fn read(&self, worktree_path: &Path) -> AppResult<PlanningConfig> {
        let config_path = self.config_path(worktree_path);

        if !config_path.exists() {
            return Err(AppError::not_found(format!(
                "Planning config not found at {}",
                config_path.display()
            )));
        }

        let content = fs::read_to_string(&config_path)?;
        let config: PlanningConfig = serde_json::from_str(&content)?;

        Ok(config)
    }

    /// Write a planning config to a worktree
    pub fn write(&self, worktree_path: &Path, config: &PlanningConfig) -> AppResult<()> {
        let config_path = self.config_path(worktree_path);
        let content = serde_json::to_string_pretty(config)?;

        fs::write(&config_path, content)?;

        Ok(())
    }

    /// Create a new planning config in a worktree
    pub fn create(
        &self,
        worktree_path: &Path,
        task_name: &str,
        target_branch: &str,
        prd_path: Option<&str>,
        execution_mode: &str,
    ) -> AppResult<PlanningConfig> {
        let mut config = PlanningConfig::new(task_name, target_branch);
        config.prd_path = prd_path.map(|s| s.to_string());
        config.execution_mode = execution_mode.to_string();

        self.write(worktree_path, &config)?;

        Ok(config)
    }

    /// Update the phase in a planning config
    pub fn update_phase(
        &self,
        worktree_path: &Path,
        phase: PlanningPhase,
    ) -> AppResult<PlanningConfig> {
        let mut config = self.read(worktree_path)?;
        config.set_phase(phase);
        self.write(worktree_path, &config)?;
        Ok(config)
    }

    /// Mark a story as complete in the planning config
    pub fn complete_story(
        &self,
        worktree_path: &Path,
        story_id: &str,
    ) -> AppResult<PlanningConfig> {
        let mut config = self.read(worktree_path)?;
        config.complete_story(story_id);
        self.write(worktree_path, &config)?;
        Ok(config)
    }

    /// Delete the planning config from a worktree
    pub fn delete(&self, worktree_path: &Path) -> AppResult<()> {
        let config_path = self.config_path(worktree_path);

        if config_path.exists() {
            fs::remove_file(&config_path)?;
        }

        Ok(())
    }

    /// Get list of planning files that should be excluded from commits
    pub fn get_planning_files(&self, worktree_path: &Path) -> Vec<PathBuf> {
        PLANNING_FILES
            .iter()
            .map(|f| worktree_path.join(f))
            .filter(|p| p.exists())
            .collect()
    }

    /// Check if a file path is a planning file
    pub fn is_planning_file(&self, path: &Path) -> bool {
        if let Some(filename) = path.file_name() {
            let filename_str = filename.to_string_lossy();
            PLANNING_FILES.iter().any(|&pf| filename_str == pf)
        } else {
            false
        }
    }

    /// Get files that should be committed (excluding planning files)
    pub fn get_committable_files(&self, worktree_path: &Path, files: &[String]) -> Vec<String> {
        files
            .iter()
            .filter(|f| {
                let path = worktree_path.join(f);
                !self.is_planning_file(&path)
            })
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_config_path() {
        let service = PlanningConfigService::new();
        let path = Path::new("/path/to/worktree");
        let config_path = service.config_path(path);
        assert!(config_path
            .to_string_lossy()
            .contains(".planning-config.json"));
    }

    #[test]
    fn test_is_planning_file() {
        let service = PlanningConfigService::new();

        assert!(service.is_planning_file(Path::new(".planning-config.json")));
        assert!(service.is_planning_file(Path::new("/some/path/prd.json")));
        assert!(service.is_planning_file(Path::new("progress.txt")));

        assert!(!service.is_planning_file(Path::new("main.rs")));
        assert!(!service.is_planning_file(Path::new("Cargo.toml")));
    }

    #[test]
    fn test_get_committable_files() {
        let service = PlanningConfigService::new();
        let worktree_path = Path::new("/worktree");

        let files = vec![
            "src/main.rs".to_string(),
            "prd.json".to_string(),
            "Cargo.toml".to_string(),
            "progress.txt".to_string(),
        ];

        let committable = service.get_committable_files(worktree_path, &files);

        assert_eq!(committable.len(), 2);
        assert!(committable.contains(&"src/main.rs".to_string()));
        assert!(committable.contains(&"Cargo.toml".to_string()));
        assert!(!committable.contains(&"prd.json".to_string()));
        assert!(!committable.contains(&"progress.txt".to_string()));
    }

    #[test]
    fn test_create_and_read_config() {
        let dir = tempdir().unwrap();
        let service = PlanningConfigService::new();

        let config = service
            .create(dir.path(), "test-task", "main", Some("prd.json"), "auto")
            .unwrap();

        assert_eq!(config.task_name, "test-task");
        assert_eq!(config.target_branch, "main");
        assert_eq!(config.prd_path, Some("prd.json".to_string()));

        let read_config = service.read(dir.path()).unwrap();
        assert_eq!(read_config.task_name, "test-task");
    }

    #[test]
    fn test_update_phase() {
        let dir = tempdir().unwrap();
        let service = PlanningConfigService::new();

        service
            .create(dir.path(), "test-task", "main", None, "auto")
            .unwrap();

        let updated = service
            .update_phase(dir.path(), PlanningPhase::Executing)
            .unwrap();

        assert!(matches!(updated.phase, PlanningPhase::Executing));
    }

    #[test]
    fn test_complete_story() {
        let dir = tempdir().unwrap();
        let service = PlanningConfigService::new();

        service
            .create(dir.path(), "test-task", "main", None, "auto")
            .unwrap();

        let updated = service.complete_story(dir.path(), "story-001").unwrap();

        assert!(updated.completed_stories.contains(&"story-001".to_string()));
    }
}
