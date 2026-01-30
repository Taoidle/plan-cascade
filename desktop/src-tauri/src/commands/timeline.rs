//! Timeline Commands
//!
//! Tauri commands for checkpoint and timeline management.

use crate::models::checkpoint::{
    Checkpoint, CheckpointBranch, CheckpointDiff, RestoreResult, TimelineMetadata,
};
use crate::models::response::CommandResponse;
use crate::services::timeline::TimelineService;

/// Create a new checkpoint
#[tauri::command]
pub fn create_checkpoint(
    project_path: String,
    session_id: String,
    label: String,
    tracked_files: Vec<String>,
) -> Result<CommandResponse<Checkpoint>, String> {
    let service = TimelineService::new();

    match service.create_checkpoint(&project_path, &session_id, &label, &tracked_files) {
        Ok(checkpoint) => Ok(CommandResponse::ok(checkpoint)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// List all checkpoints for a session
#[tauri::command]
pub fn list_checkpoints(
    project_path: String,
    session_id: String,
    branch_id: Option<String>,
) -> Result<CommandResponse<Vec<Checkpoint>>, String> {
    let service = TimelineService::new();

    match service.list_checkpoints(&project_path, &session_id, branch_id.as_deref()) {
        Ok(checkpoints) => Ok(CommandResponse::ok(checkpoints)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Get a single checkpoint by ID
#[tauri::command]
pub fn get_checkpoint(
    project_path: String,
    session_id: String,
    checkpoint_id: String,
) -> Result<CommandResponse<Checkpoint>, String> {
    let service = TimelineService::new();

    match service.get_checkpoint(&project_path, &session_id, &checkpoint_id) {
        Ok(checkpoint) => Ok(CommandResponse::ok(checkpoint)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Delete a checkpoint
#[tauri::command]
pub fn delete_checkpoint(
    project_path: String,
    session_id: String,
    checkpoint_id: String,
) -> Result<CommandResponse<()>, String> {
    let service = TimelineService::new();

    match service.delete_checkpoint(&project_path, &session_id, &checkpoint_id) {
        Ok(()) => Ok(CommandResponse::ok(())),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Get the full timeline metadata for a session
#[tauri::command]
pub fn get_timeline(
    project_path: String,
    session_id: String,
) -> Result<CommandResponse<TimelineMetadata>, String> {
    let service = TimelineService::new();

    match service.get_timeline(&project_path, &session_id) {
        Ok(metadata) => Ok(CommandResponse::ok(metadata)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Restore to a checkpoint
#[tauri::command]
pub fn restore_checkpoint(
    project_path: String,
    session_id: String,
    checkpoint_id: String,
    create_backup: bool,
    current_tracked_files: Vec<String>,
) -> Result<CommandResponse<RestoreResult>, String> {
    let service = TimelineService::new();

    match service.restore_checkpoint(
        &project_path,
        &session_id,
        &checkpoint_id,
        create_backup,
        &current_tracked_files,
    ) {
        Ok(result) => Ok(CommandResponse::ok(result)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Fork a new branch from a checkpoint
#[tauri::command]
pub fn fork_branch(
    project_path: String,
    session_id: String,
    checkpoint_id: String,
    branch_name: String,
) -> Result<CommandResponse<CheckpointBranch>, String> {
    let service = TimelineService::new();

    match service.fork_branch(&project_path, &session_id, &checkpoint_id, &branch_name) {
        Ok(branch) => Ok(CommandResponse::ok(branch)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// List all branches for a session
#[tauri::command]
pub fn list_branches(
    project_path: String,
    session_id: String,
) -> Result<CommandResponse<Vec<CheckpointBranch>>, String> {
    let service = TimelineService::new();

    match service.list_branches(&project_path, &session_id) {
        Ok(branches) => Ok(CommandResponse::ok(branches)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Get a single branch by ID
#[tauri::command]
pub fn get_branch(
    project_path: String,
    session_id: String,
    branch_id: String,
) -> Result<CommandResponse<CheckpointBranch>, String> {
    let service = TimelineService::new();

    match service.get_branch(&project_path, &session_id, &branch_id) {
        Ok(branch) => Ok(CommandResponse::ok(branch)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Switch to a different branch
#[tauri::command]
pub fn switch_branch(
    project_path: String,
    session_id: String,
    branch_id: String,
) -> Result<CommandResponse<CheckpointBranch>, String> {
    let service = TimelineService::new();

    match service.switch_branch(&project_path, &session_id, &branch_id) {
        Ok(branch) => Ok(CommandResponse::ok(branch)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Delete a branch
#[tauri::command]
pub fn delete_branch(
    project_path: String,
    session_id: String,
    branch_id: String,
) -> Result<CommandResponse<()>, String> {
    let service = TimelineService::new();

    match service.delete_branch(&project_path, &session_id, &branch_id) {
        Ok(()) => Ok(CommandResponse::ok(())),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Rename a branch
#[tauri::command]
pub fn rename_branch(
    project_path: String,
    session_id: String,
    branch_id: String,
    new_name: String,
) -> Result<CommandResponse<CheckpointBranch>, String> {
    let service = TimelineService::new();

    match service.rename_branch(&project_path, &session_id, &branch_id, &new_name) {
        Ok(branch) => Ok(CommandResponse::ok(branch)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Calculate diff between two checkpoints
#[tauri::command]
pub fn get_checkpoint_diff(
    project_path: String,
    session_id: String,
    from_checkpoint_id: String,
    to_checkpoint_id: String,
) -> Result<CommandResponse<CheckpointDiff>, String> {
    let service = TimelineService::new();

    match service.calculate_diff(&project_path, &session_id, &from_checkpoint_id, &to_checkpoint_id) {
        Ok(diff) => Ok(CommandResponse::ok(diff)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Get diff from a checkpoint to current state
#[tauri::command]
pub fn get_diff_from_current(
    project_path: String,
    session_id: String,
    checkpoint_id: String,
    tracked_files: Vec<String>,
) -> Result<CommandResponse<CheckpointDiff>, String> {
    let service = TimelineService::new();

    match service.get_diff_from_current(&project_path, &session_id, &checkpoint_id, &tracked_files) {
        Ok(diff) => Ok(CommandResponse::ok(diff)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;
    use uuid::Uuid;

    fn create_temp_project() -> String {
        let temp_dir = env::temp_dir().join(format!("timeline_cmd_test_{}", Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).unwrap();
        temp_dir.to_string_lossy().to_string()
    }

    fn cleanup_temp_project(path: &str) {
        let _ = fs::remove_dir_all(path);
    }

    #[test]
    fn test_create_checkpoint_command() {
        let project_path = create_temp_project();

        let result = create_checkpoint(
            project_path.clone(),
            "sess1".to_string(),
            "Test checkpoint".to_string(),
            vec![],
        )
        .unwrap();

        assert!(result.success);
        assert!(result.data.is_some());

        let checkpoint = result.data.unwrap();
        assert_eq!(checkpoint.label, "Test checkpoint");
        assert_eq!(checkpoint.session_id, "sess1");

        cleanup_temp_project(&project_path);
    }

    #[test]
    fn test_list_checkpoints_command() {
        let project_path = create_temp_project();

        // Create some checkpoints
        create_checkpoint(
            project_path.clone(),
            "sess1".to_string(),
            "First".to_string(),
            vec![],
        )
        .unwrap();

        create_checkpoint(
            project_path.clone(),
            "sess1".to_string(),
            "Second".to_string(),
            vec![],
        )
        .unwrap();

        let result = list_checkpoints(project_path.clone(), "sess1".to_string(), None).unwrap();

        assert!(result.success);
        assert_eq!(result.data.unwrap().len(), 2);

        cleanup_temp_project(&project_path);
    }

    #[test]
    fn test_get_checkpoint_command() {
        let project_path = create_temp_project();

        let created = create_checkpoint(
            project_path.clone(),
            "sess1".to_string(),
            "Test".to_string(),
            vec![],
        )
        .unwrap()
        .data
        .unwrap();

        let result = get_checkpoint(
            project_path.clone(),
            "sess1".to_string(),
            created.id.clone(),
        )
        .unwrap();

        assert!(result.success);
        assert_eq!(result.data.unwrap().id, created.id);

        cleanup_temp_project(&project_path);
    }

    #[test]
    fn test_fork_branch_command() {
        let project_path = create_temp_project();

        let cp = create_checkpoint(
            project_path.clone(),
            "sess1".to_string(),
            "Initial".to_string(),
            vec![],
        )
        .unwrap()
        .data
        .unwrap();

        let result = fork_branch(
            project_path.clone(),
            "sess1".to_string(),
            cp.id,
            "feature-branch".to_string(),
        )
        .unwrap();

        assert!(result.success);
        let branch = result.data.unwrap();
        assert_eq!(branch.name, "feature-branch");

        cleanup_temp_project(&project_path);
    }

    #[test]
    fn test_list_branches_command() {
        let project_path = create_temp_project();

        create_checkpoint(
            project_path.clone(),
            "sess1".to_string(),
            "Initial".to_string(),
            vec![],
        )
        .unwrap();

        let result = list_branches(project_path.clone(), "sess1".to_string()).unwrap();

        assert!(result.success);
        // Should have at least the main branch
        assert!(!result.data.unwrap().is_empty());

        cleanup_temp_project(&project_path);
    }

    #[test]
    fn test_get_timeline_command() {
        let project_path = create_temp_project();

        create_checkpoint(
            project_path.clone(),
            "sess1".to_string(),
            "Test".to_string(),
            vec![],
        )
        .unwrap();

        let result = get_timeline(project_path.clone(), "sess1".to_string()).unwrap();

        assert!(result.success);
        let metadata = result.data.unwrap();
        assert_eq!(metadata.session_id, "sess1");
        assert!(!metadata.checkpoints.is_empty());

        cleanup_temp_project(&project_path);
    }

    #[test]
    fn test_restore_checkpoint_command() {
        let project_path = create_temp_project();

        let cp = create_checkpoint(
            project_path.clone(),
            "sess1".to_string(),
            "To restore".to_string(),
            vec![],
        )
        .unwrap()
        .data
        .unwrap();

        let result = restore_checkpoint(
            project_path.clone(),
            "sess1".to_string(),
            cp.id.clone(),
            false,
            vec![],
        )
        .unwrap();

        assert!(result.success);
        let restore_result = result.data.unwrap();
        assert!(restore_result.success);
        assert_eq!(restore_result.restored_checkpoint_id, cp.id);

        cleanup_temp_project(&project_path);
    }

    #[test]
    fn test_restore_with_backup() {
        let project_path = create_temp_project();

        let cp = create_checkpoint(
            project_path.clone(),
            "sess1".to_string(),
            "Original".to_string(),
            vec![],
        )
        .unwrap()
        .data
        .unwrap();

        let result = restore_checkpoint(
            project_path.clone(),
            "sess1".to_string(),
            cp.id.clone(),
            true, // Create backup
            vec![],
        )
        .unwrap();

        assert!(result.success);
        let restore_result = result.data.unwrap();
        assert!(restore_result.backup_checkpoint_id.is_some());

        cleanup_temp_project(&project_path);
    }

    #[test]
    fn test_get_checkpoint_diff_command() {
        let project_path = create_temp_project();

        // Create file and first checkpoint
        let test_file = std::path::Path::new(&project_path).join("test.txt");
        fs::write(&test_file, "Initial content").unwrap();

        let cp1 = create_checkpoint(
            project_path.clone(),
            "sess1".to_string(),
            "V1".to_string(),
            vec!["test.txt".to_string()],
        )
        .unwrap()
        .data
        .unwrap();

        // Modify file and create second checkpoint
        fs::write(&test_file, "Modified content").unwrap();

        let cp2 = create_checkpoint(
            project_path.clone(),
            "sess1".to_string(),
            "V2".to_string(),
            vec!["test.txt".to_string()],
        )
        .unwrap()
        .data
        .unwrap();

        let result = get_checkpoint_diff(
            project_path.clone(),
            "sess1".to_string(),
            cp1.id,
            cp2.id,
        )
        .unwrap();

        assert!(result.success);
        let diff = result.data.unwrap();
        assert_eq!(diff.modified_files.len(), 1);

        cleanup_temp_project(&project_path);
    }
}
