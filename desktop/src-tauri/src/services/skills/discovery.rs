//! Skill Discovery
//!
//! Filesystem scanning from 4 sources (builtin, external, user, project-local)
//! plus convention file detection.

use std::path::{Path, PathBuf};

use crate::services::skills::config::{resolve_skill_path, SkillsConfig};
use crate::services::skills::model::{DiscoveredSkill, InjectionPhase, SkillSource};
use crate::utils::error::AppResult;

/// Convention file names to discover in project root and subdirectories
pub const CONVENTION_FILES: &[&str] = &[
    "CLAUDE.md",
    "AGENTS.md",
    "AGENT.md",
    "SKILLS.md",
    "COPILOT.md",
    "GEMINI.md",
    "SOUL.md",
];

/// Directories to skip during recursive walks
const IGNORED_DIRS: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    "target",
    "node_modules",
    ".next",
    "dist",
    "build",
    "coverage",
    "__pycache__",
    ".plan-cascade",
    ".venv",
    "venv",
    ".tox",
];

/// Discover all skills from all 4 sources and merge by priority.
///
/// Sources (in priority order, lowest to highest):
/// 1. BUILTIN: Bundled skills (priority 1-50)
/// 2. EXTERNAL: From external-skills.json sources (priority 51-100)
/// 3. USER: User-defined skills (priority 101-200)
/// 4. PROJECT-LOCAL: Project .skills/ + convention files (priority 201+)
///
/// Same name -> higher priority wins.
pub fn discover_all_skills(
    project_root: &Path,
    config: &SkillsConfig,
    plan_cascade_dir: Option<&Path>,
) -> AppResult<Vec<DiscoveredSkill>> {
    let mut all_skills = Vec::new();

    // 1. BUILTIN: Load from bundled skills directory
    // (Currently no builtin skills directory in desktop app; placeholder for future)

    // 2. EXTERNAL: Load from external-skills.json configured sources
    if let Some(base_dir) = plan_cascade_dir {
        for (skill_name, skill_entry) in &config.skills {
            if let Some(skill_path) = resolve_skill_path(config, skill_entry, base_dir) {
                let skill_files = find_skill_files_in_dir(&skill_path);
                for file_path in skill_files {
                    if let Ok(content) = std::fs::read_to_string(&file_path) {
                        let detect = skill_entry.detect.as_ref().map(|d| {
                            crate::services::skills::model::SkillDetection {
                                files: d.files.clone(),
                                patterns: d.patterns.clone(),
                            }
                        });
                        let inject_into = parse_injection_phases(&skill_entry.inject_into);

                        all_skills.push(DiscoveredSkill {
                            path: file_path,
                            content,
                            source: SkillSource::External {
                                source_name: skill_entry.source.clone(),
                            },
                            priority: skill_entry.priority,
                            detect,
                            inject_into,
                            enabled: true,
                        });
                    }
                }

                // If no SKILL.md found in directory, check if skill_path itself is a file
                if all_skills.iter().all(|s| {
                    if let SkillSource::External { .. } = &s.source {
                        // Check if this skill name was already discovered
                        false
                    } else {
                        true
                    }
                }) {
                    // Try with .md extension or SKILL.md inside directory
                    let skill_md = skill_path.join("SKILL.md");
                    if skill_md.exists() {
                        if let Ok(content) = std::fs::read_to_string(&skill_md) {
                            let detect = skill_entry.detect.as_ref().map(|d| {
                                crate::services::skills::model::SkillDetection {
                                    files: d.files.clone(),
                                    patterns: d.patterns.clone(),
                                }
                            });
                            let inject_into = parse_injection_phases(&skill_entry.inject_into);

                            all_skills.push(DiscoveredSkill {
                                path: skill_md,
                                content,
                                source: SkillSource::External {
                                    source_name: skill_entry.source.clone(),
                                },
                                priority: skill_entry.priority,
                                detect,
                                inject_into,
                                enabled: true,
                            });
                        }
                    }
                }
            }
            // If source path can't be resolved, skip silently
            let _ = skill_name; // avoid unused warning
        }
    }

    // 3. USER: Not implemented yet (would scan ~/.plan-cascade/skills.json)

    // 4. PROJECT-LOCAL: Scan project .skills/ directory + convention files
    let project_skills = discover_project_skills(project_root)?;
    for file_path in project_skills {
        if let Ok(content) = std::fs::read_to_string(&file_path) {
            all_skills.push(DiscoveredSkill {
                path: file_path,
                content,
                source: SkillSource::ProjectLocal,
                priority: 201,
                detect: None,
                inject_into: vec![InjectionPhase::Always],
                enabled: true,
            });
        }
    }

    // Sort by priority (ascending - lower priority number is processed first)
    all_skills.sort_by(|a, b| a.priority.cmp(&b.priority));

    Ok(all_skills)
}

/// Discover project-local skills only (fast path for session start).
///
/// Scans:
/// 1. `<project_root>/.skills/` directory recursively for .md files
/// 2. Convention files (CLAUDE.md, AGENTS.md, etc.) in project root
pub fn discover_project_skills(project_root: &Path) -> AppResult<Vec<PathBuf>> {
    let mut files = Vec::new();

    // Scan .skills/ directory
    let skills_dir = project_root.join(".skills");
    if skills_dir.is_dir() {
        walk_skill_directory(&skills_dir, &mut files);
    }

    // Discover convention files in root
    discover_convention_files_in_dir(project_root, &mut files);

    Ok(files)
}

/// Find SKILL.md files in a directory (non-recursive, looks for SKILL.md or *.md).
fn find_skill_files_in_dir(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();

    if !dir.is_dir() {
        // Maybe it's a file path directly
        if dir.is_file() && is_skill_file(dir) {
            files.push(dir.to_path_buf());
        }
        return files;
    }

    // First check for SKILL.md
    let skill_md = dir.join("SKILL.md");
    if skill_md.exists() {
        files.push(skill_md);
        return files;
    }

    // Then check for any .md files
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && is_skill_file(&path) {
                files.push(path);
            }
        }
    }

    files
}

/// Recursively walk a .skills/ directory and collect all .md files.
fn walk_skill_directory(dir: &Path, files: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !IGNORED_DIRS.contains(&dir_name) {
                walk_skill_directory(&path, files);
            }
        } else if is_skill_file(&path) {
            files.push(path);
        }
    }
}

/// Discover convention files in a specific directory.
fn discover_convention_files_in_dir(dir: &Path, files: &mut Vec<PathBuf>) {
    for name in CONVENTION_FILES {
        let path = dir.join(name);
        if path.is_file() {
            files.push(path);
        }
    }
}

/// Check if a path is a skill file (must have .md extension).
fn is_skill_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("md"))
        .unwrap_or(false)
}

/// Parse injection phase strings into InjectionPhase enum values.
fn parse_injection_phases(phases: &[String]) -> Vec<InjectionPhase> {
    phases
        .iter()
        .filter_map(|p| match p.to_lowercase().as_str() {
            "planning" => Some(InjectionPhase::Planning),
            "implementation" => Some(InjectionPhase::Implementation),
            "retry" => Some(InjectionPhase::Retry),
            "always" => Some(InjectionPhase::Always),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_discover_project_skills_empty_dir() {
        let dir = TempDir::new().unwrap();
        let result = discover_project_skills(dir.path()).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_discover_project_skills_with_skills_dir() {
        let dir = TempDir::new().unwrap();
        let skills_dir = dir.path().join(".skills");
        fs::create_dir(&skills_dir).unwrap();

        // Create a skill file
        fs::write(
            skills_dir.join("SKILL.md"),
            "---\nname: test\ndescription: test skill\n---\n# Test",
        )
        .unwrap();

        let result = discover_project_skills(dir.path()).unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].to_str().unwrap().contains("SKILL.md"));
    }

    #[test]
    fn test_discover_project_skills_with_nested_skills() {
        let dir = TempDir::new().unwrap();
        let skills_dir = dir.path().join(".skills");
        let nested_dir = skills_dir.join("sub-skill");
        fs::create_dir_all(&nested_dir).unwrap();

        fs::write(skills_dir.join("root.md"), "# Root skill").unwrap();
        fs::write(nested_dir.join("nested.md"), "# Nested skill").unwrap();

        let result = discover_project_skills(dir.path()).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_discover_convention_files() {
        let dir = TempDir::new().unwrap();

        fs::write(dir.path().join("CLAUDE.md"), "# CLAUDE.md\nGuidance").unwrap();
        fs::write(dir.path().join("AGENTS.md"), "# AGENTS.md\nConfig").unwrap();
        fs::write(
            dir.path().join("README.md"),
            "# README\nNot a convention file",
        )
        .unwrap();

        let result = discover_project_skills(dir.path()).unwrap();
        assert_eq!(result.len(), 2); // CLAUDE.md and AGENTS.md, not README.md
    }

    #[test]
    fn test_convention_file_names() {
        assert!(CONVENTION_FILES.contains(&"CLAUDE.md"));
        assert!(CONVENTION_FILES.contains(&"AGENTS.md"));
        assert!(CONVENTION_FILES.contains(&"AGENT.md"));
        assert!(CONVENTION_FILES.contains(&"SKILLS.md"));
        assert!(CONVENTION_FILES.contains(&"COPILOT.md"));
        assert!(CONVENTION_FILES.contains(&"GEMINI.md"));
        assert!(CONVENTION_FILES.contains(&"SOUL.md"));
    }

    #[test]
    fn test_ignored_dirs_respected() {
        let dir = TempDir::new().unwrap();
        let skills_dir = dir.path().join(".skills");
        let node_modules = skills_dir.join("node_modules");
        fs::create_dir_all(&node_modules).unwrap();

        fs::write(skills_dir.join("good.md"), "# Good skill").unwrap();
        fs::write(node_modules.join("bad.md"), "# Should be ignored").unwrap();

        let result = discover_project_skills(dir.path()).unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].to_str().unwrap().contains("good.md"));
    }

    #[test]
    fn test_is_skill_file() {
        assert!(is_skill_file(Path::new("SKILL.md")));
        assert!(is_skill_file(Path::new("test.MD")));
        assert!(!is_skill_file(Path::new("test.txt")));
        assert!(!is_skill_file(Path::new("test.rs")));
        assert!(!is_skill_file(Path::new("noext")));
    }

    #[test]
    fn test_parse_injection_phases() {
        let phases = vec![
            "planning".to_string(),
            "implementation".to_string(),
            "retry".to_string(),
        ];
        let result = parse_injection_phases(&phases);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], InjectionPhase::Planning);
        assert_eq!(result[1], InjectionPhase::Implementation);
        assert_eq!(result[2], InjectionPhase::Retry);
    }

    #[test]
    fn test_parse_injection_phases_unknown() {
        let phases = vec!["unknown".to_string(), "always".to_string()];
        let result = parse_injection_phases(&phases);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], InjectionPhase::Always);
    }

    #[test]
    fn test_discover_all_skills_empty() {
        let dir = TempDir::new().unwrap();
        let config = SkillsConfig::default();
        let result = discover_all_skills(dir.path(), &config, None).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_discover_all_skills_project_local() {
        let dir = TempDir::new().unwrap();
        let skills_dir = dir.path().join(".skills");
        fs::create_dir(&skills_dir).unwrap();
        fs::write(
            skills_dir.join("SKILL.md"),
            "---\nname: local\ndescription: local skill\n---\n# Local",
        )
        .unwrap();

        let config = SkillsConfig::default();
        let result = discover_all_skills(dir.path(), &config, None).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].priority, 201);
        assert!(matches!(result[0].source, SkillSource::ProjectLocal));
    }

    #[test]
    fn test_discover_all_skills_with_convention_and_skills_dir() {
        let dir = TempDir::new().unwrap();

        // Create .skills/
        let skills_dir = dir.path().join(".skills");
        fs::create_dir(&skills_dir).unwrap();
        fs::write(skills_dir.join("custom.md"), "# Custom skill").unwrap();

        // Create convention file
        fs::write(dir.path().join("CLAUDE.md"), "# CLAUDE\nProject guidance").unwrap();

        let config = SkillsConfig::default();
        let result = discover_all_skills(dir.path(), &config, None).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_non_md_files_ignored_in_skills_dir() {
        let dir = TempDir::new().unwrap();
        let skills_dir = dir.path().join(".skills");
        fs::create_dir(&skills_dir).unwrap();

        fs::write(skills_dir.join("good.md"), "# Skill").unwrap();
        fs::write(skills_dir.join("bad.txt"), "Not a skill").unwrap();
        fs::write(skills_dir.join("bad.json"), "{}").unwrap();

        let result = discover_project_skills(dir.path()).unwrap();
        assert_eq!(result.len(), 1);
    }
}
