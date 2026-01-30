//! Project Commands
//!
//! Tauri commands for project management.

use crate::models::project::{Project, ProjectSortBy};
use crate::models::response::CommandResponse;
use crate::services::project::ProjectService;

/// List all projects with sorting and pagination
#[tauri::command]
pub fn list_projects(
    sort_by: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<CommandResponse<Vec<Project>>, String> {
    let service = ProjectService::new();

    let sort = sort_by
        .map(|s| ProjectSortBy::from_str(&s))
        .unwrap_or_default();
    let limit = limit.unwrap_or(50);
    let offset = offset.unwrap_or(0);

    match service.list_projects(sort, limit, offset) {
        Ok(projects) => Ok(CommandResponse::ok(projects)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Get a single project by ID
#[tauri::command]
pub fn get_project(project_id: String) -> Result<CommandResponse<Project>, String> {
    let service = ProjectService::new();

    match service.get_project(&project_id) {
        Ok(project) => Ok(CommandResponse::ok(project)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Search projects by name or path
#[tauri::command]
pub fn search_projects(query: String) -> Result<CommandResponse<Vec<Project>>, String> {
    let service = ProjectService::new();

    match service.search_projects(&query) {
        Ok(projects) => Ok(CommandResponse::ok(projects)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_projects_command() {
        // This will return empty list if ~/.claude/projects doesn't exist
        let result = list_projects(None, None, None).unwrap();
        assert!(result.success);
    }
}
