//! Skill Configuration
//!
//! Loads and merges external-skills.json + user config with 4-source hierarchy.
//! Compatible with plan-cascade CLI external-skills.json format.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::services::skills::model::SkillDetection;
use crate::utils::error::{AppError, AppResult};

/// Priority ranges for each source tier
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriorityRange {
    pub min: u32,
    pub max: u32,
}

/// Priority ranges configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriorityRanges {
    pub builtin: PriorityRange,
    pub submodule: PriorityRange,
    pub user: PriorityRange,
}

impl Default for PriorityRanges {
    fn default() -> Self {
        Self {
            builtin: PriorityRange { min: 1, max: 50 },
            submodule: PriorityRange { min: 51, max: 100 },
            user: PriorityRange { min: 101, max: 200 },
        }
    }
}

/// A skill source definition (e.g., external submodule)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceDefinition {
    /// Source type: "submodule", "local", "remote"
    #[serde(rename = "type")]
    pub source_type: String,
    /// Local path (relative to plan-cascade install dir)
    pub path: Option<String>,
    /// Git repository URL
    pub repository: Option<String>,
}

/// Detection rules for a skill in the config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillDetectConfig {
    /// Files to check for existence
    pub files: Vec<String>,
    /// Content patterns to search for in those files
    #[serde(default)]
    pub patterns: Vec<String>,
}

impl From<SkillDetectConfig> for SkillDetection {
    fn from(config: SkillDetectConfig) -> Self {
        SkillDetection {
            files: config.files,
            patterns: config.patterns,
        }
    }
}

/// A single skill entry in external-skills.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillEntry {
    pub source: String,
    pub skill_path: String,
    #[serde(default)]
    pub detect: Option<SkillDetectConfig>,
    #[serde(default)]
    pub inject_into: Vec<String>,
    #[serde(default = "default_priority")]
    pub priority: u32,
}

fn default_priority() -> u32 {
    100
}

/// Settings from external-skills.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillsSettings {
    #[serde(default = "default_max_skills")]
    pub max_skills_per_story: usize,
    #[serde(default = "default_max_lines")]
    pub max_content_lines: usize,
    #[serde(default)]
    pub cache_enabled: bool,
}

fn default_max_skills() -> usize {
    3
}

fn default_max_lines() -> usize {
    200
}

impl Default for SkillsSettings {
    fn default() -> Self {
        Self {
            max_skills_per_story: 3,
            max_content_lines: 200,
            cache_enabled: true,
        }
    }
}

/// The full external-skills.json configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillsConfig {
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default)]
    pub priority_ranges: PriorityRanges,
    #[serde(default)]
    pub sources: HashMap<String, SourceDefinition>,
    #[serde(default)]
    pub skills: HashMap<String, SkillEntry>,
    #[serde(default)]
    pub settings: SkillsSettings,
}

fn default_version() -> String {
    "1.1.0".to_string()
}

impl Default for SkillsConfig {
    fn default() -> Self {
        Self {
            version: default_version(),
            priority_ranges: PriorityRanges::default(),
            sources: HashMap::new(),
            skills: HashMap::new(),
            settings: SkillsSettings::default(),
        }
    }
}

/// Load skills configuration from a JSON file path.
/// Returns default config if file doesn't exist.
pub fn load_skills_config(path: &Path) -> AppResult<SkillsConfig> {
    if !path.exists() {
        return Ok(SkillsConfig::default());
    }

    let content = std::fs::read_to_string(path).map_err(|e| {
        AppError::config(format!(
            "Failed to read skills config {}: {}",
            path.display(),
            e
        ))
    })?;

    serde_json::from_str(&content).map_err(|e| {
        AppError::config(format!(
            "Failed to parse skills config {}: {}",
            path.display(),
            e
        ))
    })
}

/// Merge two skills configs. `override_config` takes precedence.
pub fn merge_configs(base: &SkillsConfig, override_config: &SkillsConfig) -> SkillsConfig {
    let mut merged = base.clone();

    // Override sources
    for (name, source) in &override_config.sources {
        merged.sources.insert(name.clone(), source.clone());
    }

    // Override skills
    for (name, skill) in &override_config.skills {
        merged.skills.insert(name.clone(), skill.clone());
    }

    // Override settings if non-default
    if override_config.settings.max_skills_per_story != 3 {
        merged.settings.max_skills_per_story = override_config.settings.max_skills_per_story;
    }
    if override_config.settings.max_content_lines != 200 {
        merged.settings.max_content_lines = override_config.settings.max_content_lines;
    }

    merged
}

/// Resolve the source path for a skill entry.
/// Combines source definition path with skill entry skill_path.
pub fn resolve_skill_path(
    config: &SkillsConfig,
    skill_entry: &SkillEntry,
    base_dir: &Path,
) -> Option<std::path::PathBuf> {
    let source_def = config.sources.get(&skill_entry.source)?;
    let source_path = source_def.path.as_ref()?;
    Some(base_dir.join(source_path).join(&skill_entry.skill_path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_default_config() {
        let config = SkillsConfig::default();
        assert_eq!(config.version, "1.1.0");
        assert_eq!(config.priority_ranges.builtin.min, 1);
        assert_eq!(config.priority_ranges.builtin.max, 50);
        assert_eq!(config.priority_ranges.submodule.min, 51);
        assert_eq!(config.priority_ranges.submodule.max, 100);
        assert_eq!(config.priority_ranges.user.min, 101);
        assert_eq!(config.priority_ranges.user.max, 200);
        assert_eq!(config.settings.max_skills_per_story, 3);
        assert_eq!(config.settings.max_content_lines, 200);
    }

    #[test]
    fn test_load_nonexistent_returns_default() {
        let config = load_skills_config(Path::new("/nonexistent/path.json")).unwrap();
        assert_eq!(config.version, "1.1.0");
        assert!(config.sources.is_empty());
        assert!(config.skills.is_empty());
    }

    #[test]
    fn test_load_valid_config() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("external-skills.json");
        let content = r#"{
            "version": "1.1.0",
            "priority_ranges": {
                "builtin": { "min": 1, "max": 50 },
                "submodule": { "min": 51, "max": 100 },
                "user": { "min": 101, "max": 200 }
            },
            "sources": {
                "vercel": {
                    "type": "submodule",
                    "path": "external-skills/vercel",
                    "repository": "https://github.com/vercel-labs/agent-skills"
                }
            },
            "skills": {
                "react-best-practices": {
                    "source": "vercel",
                    "skill_path": "skills/react-best-practices",
                    "detect": {
                        "files": ["package.json"],
                        "patterns": ["\"react\"", "\"next\""]
                    },
                    "inject_into": ["planning", "implementation"],
                    "priority": 100
                }
            },
            "settings": {
                "max_skills_per_story": 3,
                "max_content_lines": 200,
                "cache_enabled": true
            }
        }"#;
        fs::write(&config_path, content).unwrap();

        let config = load_skills_config(&config_path).unwrap();
        assert_eq!(config.version, "1.1.0");
        assert_eq!(config.sources.len(), 1);
        assert!(config.sources.contains_key("vercel"));
        assert_eq!(config.skills.len(), 1);

        let skill = config.skills.get("react-best-practices").unwrap();
        assert_eq!(skill.source, "vercel");
        assert_eq!(skill.priority, 100);
        assert!(skill.detect.is_some());
        let detect = skill.detect.as_ref().unwrap();
        assert_eq!(detect.files, vec!["package.json"]);
        assert_eq!(detect.patterns, vec!["\"react\"", "\"next\""]);
    }

    #[test]
    fn test_load_invalid_json() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("bad.json");
        fs::write(&config_path, "not valid json").unwrap();

        let result = load_skills_config(&config_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_merge_configs() {
        let base = SkillsConfig {
            version: "1.0.0".to_string(),
            sources: {
                let mut m = HashMap::new();
                m.insert(
                    "vercel".to_string(),
                    SourceDefinition {
                        source_type: "submodule".to_string(),
                        path: Some("external-skills/vercel".to_string()),
                        repository: None,
                    },
                );
                m
            },
            skills: HashMap::new(),
            ..Default::default()
        };

        let override_config = SkillsConfig {
            sources: {
                let mut m = HashMap::new();
                m.insert(
                    "custom".to_string(),
                    SourceDefinition {
                        source_type: "local".to_string(),
                        path: Some("/custom/skills".to_string()),
                        repository: None,
                    },
                );
                m
            },
            ..Default::default()
        };

        let merged = merge_configs(&base, &override_config);
        assert_eq!(merged.sources.len(), 2);
        assert!(merged.sources.contains_key("vercel"));
        assert!(merged.sources.contains_key("custom"));
    }

    #[test]
    fn test_resolve_skill_path() {
        let config = SkillsConfig {
            sources: {
                let mut m = HashMap::new();
                m.insert(
                    "vercel".to_string(),
                    SourceDefinition {
                        source_type: "submodule".to_string(),
                        path: Some("external-skills/vercel".to_string()),
                        repository: None,
                    },
                );
                m
            },
            ..Default::default()
        };

        let entry = SkillEntry {
            source: "vercel".to_string(),
            skill_path: "skills/react-best-practices".to_string(),
            detect: None,
            inject_into: vec![],
            priority: 100,
        };

        let base_dir = Path::new("/home/user/plan-cascade");
        let resolved = resolve_skill_path(&config, &entry, base_dir);
        assert!(resolved.is_some());
        let path = resolved.unwrap();
        assert_eq!(
            path.to_str().unwrap(),
            "/home/user/plan-cascade/external-skills/vercel/skills/react-best-practices"
        );
    }

    #[test]
    fn test_resolve_unknown_source() {
        let config = SkillsConfig::default();
        let entry = SkillEntry {
            source: "nonexistent".to_string(),
            skill_path: "skills/test".to_string(),
            detect: None,
            inject_into: vec![],
            priority: 100,
        };

        let resolved = resolve_skill_path(&config, &entry, Path::new("/base"));
        assert!(resolved.is_none());
    }

    #[test]
    fn test_skill_detect_config_to_detection() {
        let config = SkillDetectConfig {
            files: vec!["package.json".to_string()],
            patterns: vec!["\"react\"".to_string()],
        };
        let detection: SkillDetection = config.into();
        assert_eq!(detection.files, vec!["package.json"]);
        assert_eq!(detection.patterns, vec!["\"react\""]);
    }
}
