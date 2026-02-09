//! Project Models
//!
//! Data structures for Claude Code projects.

use serde::{Deserialize, Serialize};

/// A Claude Code project discovered in ~/.claude/projects/
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    /// Unique project identifier (directory name hash or path-based)
    pub id: String,
    /// Project display name (derived from path or CLAUDE.md)
    pub name: String,
    /// Full path to the project directory
    pub path: String,
    /// Last activity timestamp (ISO 8601)
    pub last_activity: Option<String>,
    /// Number of sessions in this project
    pub session_count: u32,
    /// Total message count across all sessions
    pub message_count: u32,
}

impl Project {
    /// Create a new project with minimal info
    pub fn new(id: impl Into<String>, name: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            path: path.into(),
            last_activity: None,
            session_count: 0,
            message_count: 0,
        }
    }
}

/// Sort options for project listing
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProjectSortBy {
    /// Sort by most recent activity (default)
    #[default]
    RecentActivity,
    /// Sort alphabetically by name
    Name,
    /// Sort by session count (descending)
    SessionCount,
}

impl ProjectSortBy {
    /// Parse from string, defaulting to RecentActivity
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "name" | "alphabetical" => Self::Name,
            "session_count" | "sessions" => Self::SessionCount,
            _ => Self::RecentActivity,
        }
    }
}

/// Request for listing projects
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ListProjectsRequest {
    /// Sort order
    #[serde(default)]
    pub sort_by: ProjectSortBy,
    /// Maximum number of projects to return
    #[serde(default = "default_limit")]
    pub limit: u32,
    /// Offset for pagination
    #[serde(default)]
    pub offset: u32,
}

fn default_limit() -> u32 {
    50
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_creation() {
        let project = Project::new("proj1", "My Project", "/path/to/project");
        assert_eq!(project.id, "proj1");
        assert_eq!(project.name, "My Project");
        assert_eq!(project.session_count, 0);
    }

    #[test]
    fn test_sort_by_parsing() {
        assert!(matches!(
            ProjectSortBy::from_str("name"),
            ProjectSortBy::Name
        ));
        assert!(matches!(
            ProjectSortBy::from_str("sessions"),
            ProjectSortBy::SessionCount
        ));
        assert!(matches!(
            ProjectSortBy::from_str("recent"),
            ProjectSortBy::RecentActivity
        ));
        assert!(matches!(
            ProjectSortBy::from_str("unknown"),
            ProjectSortBy::RecentActivity
        ));
    }

    #[test]
    fn test_project_serialization() {
        let project = Project::new("proj1", "Test", "/test");
        let json = serde_json::to_string(&project).unwrap();
        assert!(json.contains("\"id\":\"proj1\""));

        let parsed: Project = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "proj1");
    }
}
