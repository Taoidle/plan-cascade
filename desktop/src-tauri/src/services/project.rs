//! Project Service
//!
//! Scans and manages Claude Code projects from ~/.claude/projects/

use std::fs;
use std::path::PathBuf;

use crate::models::project::{Project, ProjectSortBy};
use crate::utils::error::{AppError, AppResult};
use crate::utils::paths::claude_projects_dir;

/// Service for managing Claude Code projects
#[derive(Debug, Default)]
pub struct ProjectService;

impl ProjectService {
    /// Create a new project service
    pub fn new() -> Self {
        Self
    }

    /// Scan the Claude projects directory and return all discovered projects
    pub fn scan_projects(&self) -> AppResult<Vec<Project>> {
        let projects_dir = claude_projects_dir()?;

        if !projects_dir.exists() {
            return Ok(vec![]);
        }

        let mut projects = Vec::new();

        let entries = fs::read_dir(&projects_dir).map_err(|e| AppError::Io(e))?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(project) = self.parse_project_dir(&path) {
                    projects.push(project);
                }
            }
        }

        Ok(projects)
    }

    /// Parse a project directory and extract metadata
    fn parse_project_dir(&self, path: &PathBuf) -> Option<Project> {
        let dir_name = path.file_name()?.to_string_lossy().to_string();

        // Generate project ID from directory name
        let id = self.generate_project_id(&dir_name);

        // Try to get a nice name from the path
        let name = self.extract_project_name(path, &dir_name);

        // Get last modified time
        let last_activity = fs::metadata(path)
            .ok()
            .and_then(|m| m.modified().ok())
            .map(|t| {
                let datetime: chrono::DateTime<chrono::Utc> = t.into();
                datetime.to_rfc3339()
            });

        // Count sessions
        let sessions_dir = path.join("sessions");
        let session_count = if sessions_dir.exists() {
            fs::read_dir(&sessions_dir)
                .map(|entries| entries.filter_map(|e| e.ok()).count() as u32)
                .unwrap_or(0)
        } else {
            0
        };

        Some(Project {
            id,
            name,
            path: path.to_string_lossy().to_string(),
            last_activity,
            session_count,
            message_count: 0, // Would require parsing all sessions
        })
    }

    /// Generate a project ID from directory name
    fn generate_project_id(&self, dir_name: &str) -> String {
        // Use the directory name as-is for now (it's usually a hash already)
        dir_name.to_string()
    }

    /// Extract a nice project name from path or use directory name
    fn extract_project_name(&self, path: &PathBuf, dir_name: &str) -> String {
        // Check for CLAUDE.md to extract project info
        let claude_md = path.join("CLAUDE.md");
        if claude_md.exists() {
            if let Ok(content) = fs::read_to_string(&claude_md) {
                // Try to extract title from first heading
                for line in content.lines().take(5) {
                    let trimmed = line.trim();
                    if trimmed.starts_with("# ") {
                        return trimmed[2..].trim().to_string();
                    }
                }
            }
        }

        // Fall back to directory name (decode if it's a path hash)
        // The format is often like "d41d8cd98f00b204e9800998ecf8427e-projectname"
        if dir_name.contains('-') {
            let parts: Vec<&str> = dir_name.splitn(2, '-').collect();
            if parts.len() == 2 {
                return parts[1].to_string();
            }
        }

        dir_name.to_string()
    }

    /// List projects with sorting and pagination
    pub fn list_projects(
        &self,
        sort_by: ProjectSortBy,
        limit: u32,
        offset: u32,
    ) -> AppResult<Vec<Project>> {
        let mut projects = self.scan_projects()?;

        // Sort
        match sort_by {
            ProjectSortBy::RecentActivity => {
                projects.sort_by(|a, b| b.last_activity.cmp(&a.last_activity));
            }
            ProjectSortBy::Name => {
                projects.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
            }
            ProjectSortBy::SessionCount => {
                projects.sort_by(|a, b| b.session_count.cmp(&a.session_count));
            }
        }

        // Paginate
        let start = offset as usize;
        let end = (offset + limit) as usize;

        Ok(projects.into_iter().skip(start).take(end - start).collect())
    }

    /// Get a single project by ID
    pub fn get_project(&self, project_id: &str) -> AppResult<Project> {
        let projects = self.scan_projects()?;

        projects
            .into_iter()
            .find(|p| p.id == project_id)
            .ok_or_else(|| AppError::not_found(format!("Project not found: {}", project_id)))
    }

    /// Search projects by name
    pub fn search_projects(&self, query: &str) -> AppResult<Vec<Project>> {
        let projects = self.scan_projects()?;
        let query_lower = query.to_lowercase();

        Ok(projects
            .into_iter()
            .filter(|p| {
                p.name.to_lowercase().contains(&query_lower)
                    || p.path.to_lowercase().contains(&query_lower)
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_creation() {
        let service = ProjectService::new();
        // Just verify it creates without panic
        let _ = service;
    }

    #[test]
    fn test_generate_project_id() {
        let service = ProjectService::new();
        let id = service.generate_project_id("abc123-myproject");
        assert_eq!(id, "abc123-myproject");
    }

    #[test]
    fn test_extract_project_name() {
        let service = ProjectService::new();

        // Test with hash-prefixed name
        let path = PathBuf::from("/tmp/abc123-myproject");
        let name = service.extract_project_name(&path, "abc123-myproject");
        assert_eq!(name, "myproject");

        // Test with simple name
        let path = PathBuf::from("/tmp/simple");
        let name = service.extract_project_name(&path, "simple");
        assert_eq!(name, "simple");
    }
}
