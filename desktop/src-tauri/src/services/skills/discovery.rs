//! Skill Discovery
//!
//! Filesystem scanning from 4 sources (builtin, external, user, project-local)
//! plus convention file detection.

use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::services::skills::config::{
    resolve_source_path_for_project, SkillsConfig,
};
use crate::services::skills::model::{
    DiscoveredSkill, InjectionPhase, SkillDetection, SkillSource,
};
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
    all_skills.extend(discover_builtin_skills());

    // 2. EXTERNAL: Load from configured source roots and explicit external-skills.json entries
    for (source_name, source_def) in &config.sources {
        if !source_def.enabled {
            continue;
        }
        if let Some(source_root) =
            resolve_source_path_for_project(source_def, project_root, plan_cascade_dir)
        {
            let skill_files = find_skill_files_in_source_root(&source_root);
            for file_path in skill_files {
                if let Ok(content) = std::fs::read_to_string(&file_path) {
                    all_skills.push(DiscoveredSkill {
                        path: file_path,
                        content,
                        source: SkillSource::External {
                            source_name: source_name.clone(),
                        },
                        priority: config.priority_ranges.submodule.min.saturating_add(10),
                        detect: None,
                        inject_into: vec![InjectionPhase::Always],
                        enabled: true,
                    });
                }
            }
        }
    }

    if let Some(base_dir) = plan_cascade_dir {
        for (skill_name, skill_entry) in &config.skills {
            if config
                .sources
                .get(&skill_entry.source)
                .map(|source_def| !source_def.enabled)
                .unwrap_or(false)
            {
                continue;
            }
            let Some(source_def) = config.sources.get(&skill_entry.source) else {
                let _ = skill_name;
                continue;
            };
            if let Some(source_root) =
                resolve_source_path_for_project(source_def, project_root, Some(base_dir))
            {
                let skill_path = source_root.join(&skill_entry.skill_path);
                let skill_files = find_skill_files_in_source_root(&skill_path);
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
            }
            // If source path can't be resolved, skip silently
            let _ = skill_name; // avoid unused warning
        }
    }

    // 3. USER: Load from .plan-cascade/skills.json (project) and ~/.plan-cascade/skills.json (user)
    all_skills.extend(discover_user_skills(project_root, plan_cascade_dir));

    // 4. PROJECT-LOCAL: Scan project .skills/ directory + convention files
    let project_skills = discover_project_skills(project_root)?;
    for file_path in project_skills {
        if let Ok(content) = std::fs::read_to_string(&file_path) {
            let depth = file_path
                .strip_prefix(project_root)
                .ok()
                .map(path_depth)
                .unwrap_or(0) as u32;
            let is_convention = file_path
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| CONVENTION_FILES.contains(&name))
                .unwrap_or(false);
            all_skills.push(DiscoveredSkill {
                path: file_path,
                content,
                source: SkillSource::ProjectLocal,
                priority: if is_convention {
                    260 + depth
                } else {
                    220 + depth
                },
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

#[derive(Debug, Clone, Deserialize)]
struct UserSkillsConfig {
    #[serde(default)]
    skills: Vec<UserSkillEntry>,
}

#[derive(Debug, Clone, Deserialize)]
struct UserSkillEntry {
    name: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    detect: Option<UserSkillDetect>,
    #[serde(default)]
    priority: Option<u32>,
    #[serde(default)]
    inject_into: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct UserSkillDetect {
    #[serde(default)]
    files: Vec<String>,
    #[serde(default)]
    patterns: Vec<String>,
}

#[derive(Debug, Clone)]
struct UserSkillCandidate {
    entry: UserSkillEntry,
    base_dir: PathBuf,
}

fn load_user_skills_config(config_path: &Path) -> Option<UserSkillsConfig> {
    let content = std::fs::read_to_string(config_path).ok()?;
    serde_json::from_str::<UserSkillsConfig>(&content).ok()
}

fn discover_user_skills(
    project_root: &Path,
    plan_cascade_dir: Option<&Path>,
) -> Vec<DiscoveredSkill> {
    let mut merged: HashMap<String, UserSkillCandidate> = HashMap::new();

    if let Some(plan_dir) = plan_cascade_dir {
        let user_config_path = plan_dir.join("skills.json");
        if let Some(user_cfg) = load_user_skills_config(&user_config_path) {
            let user_base = user_config_path
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| plan_dir.to_path_buf());
            for entry in user_cfg.skills {
                merged.insert(
                    entry.name.clone(),
                    UserSkillCandidate {
                        entry,
                        base_dir: user_base.clone(),
                    },
                );
            }
        }
    }

    let project_config_path = project_root.join(".plan-cascade").join("skills.json");
    if let Some(project_cfg) = load_user_skills_config(&project_config_path) {
        let project_base = project_config_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| project_root.to_path_buf());
        for entry in project_cfg.skills {
            merged.insert(
                entry.name.clone(),
                UserSkillCandidate {
                    entry,
                    base_dir: project_base.clone(),
                },
            );
        }
    }

    let mut discovered = Vec::new();
    for (_, candidate) in merged {
        if candidate.entry.path.is_none() {
            // URL-backed user skills are intentionally skipped in desktop for now.
            if candidate.entry.url.is_some() {
                tracing::warn!(
                    "Skipping user skill '{}' from URL source (not yet supported in desktop)",
                    candidate.entry.name
                );
            }
            continue;
        }

        let relative_path = candidate.entry.path.as_deref().unwrap_or_default();
        let resolved = candidate.base_dir.join(relative_path);
        let files = if resolved.is_dir() {
            find_skill_files_in_dir(&resolved)
        } else if resolved.is_file() && is_skill_file(&resolved) {
            vec![resolved]
        } else {
            Vec::new()
        };

        if files.is_empty() {
            continue;
        }

        let detect = candidate.entry.detect.as_ref().map(|d| SkillDetection {
            files: d.files.clone(),
            patterns: d.patterns.clone(),
        });
        let inject_into = if candidate.entry.inject_into.is_empty() {
            vec![InjectionPhase::Implementation, InjectionPhase::Retry]
        } else {
            parse_injection_phases(&candidate.entry.inject_into)
        };
        let priority = candidate.entry.priority.unwrap_or(150).clamp(101, 200);

        for file_path in files {
            if let Ok(content) = std::fs::read_to_string(&file_path) {
                discovered.push(DiscoveredSkill {
                    path: file_path,
                    content,
                    source: SkillSource::User,
                    priority,
                    detect: detect.clone(),
                    inject_into: inject_into.clone(),
                    enabled: true,
                });
            }
        }
    }

    discovered
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

    // Discover convention files recursively so nearest directory rules can win.
    walk_project_convention_dirs(project_root, &mut files);

    Ok(files)
}

fn discover_builtin_skills() -> Vec<DiscoveredSkill> {
    builtin_skill_specs()
        .into_iter()
        .map(|spec| DiscoveredSkill {
            path: PathBuf::from(format!("builtin://{}", spec.slug)),
            content: spec.content.to_string(),
            source: SkillSource::Builtin,
            priority: spec.priority,
            detect: spec.detect,
            inject_into: spec.inject_into,
            enabled: true,
        })
        .collect()
}

fn path_depth(path: &Path) -> usize {
    path.components().count().saturating_sub(1)
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

fn find_skill_files_in_source_root(root: &Path) -> Vec<PathBuf> {
    if root.is_file() {
        return if is_skill_file(root) {
            vec![root.to_path_buf()]
        } else {
            Vec::new()
        };
    }

    if !root.is_dir() {
        return Vec::new();
    }

    let mut files = Vec::new();
    walk_skill_source_directory(root, &mut files);
    if files.is_empty() {
        return find_skill_files_in_dir(root);
    }
    files.sort();
    files.dedup();
    files
}

fn walk_skill_source_directory(dir: &Path, files: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !IGNORED_DIRS.contains(&dir_name) {
                walk_skill_source_directory(&path, files);
            }
            continue;
        }

        let is_named_skill_md = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.eq_ignore_ascii_case("SKILL.md"))
            .unwrap_or(false);
        if is_named_skill_md {
            files.push(path);
        }
    }
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

fn walk_project_convention_dirs(dir: &Path, files: &mut Vec<PathBuf>) {
    discover_convention_files_in_dir(dir, files);

    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if IGNORED_DIRS.contains(&dir_name) {
            continue;
        }
        walk_project_convention_dirs(&path, files);
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

struct BuiltinSkillSpec {
    slug: &'static str,
    priority: u32,
    detect: Option<SkillDetection>,
    inject_into: Vec<InjectionPhase>,
    content: &'static str,
}

fn builtin_skill_specs() -> Vec<BuiltinSkillSpec> {
    vec![
        BuiltinSkillSpec {
            slug: "react-next",
            priority: 20,
            detect: Some(SkillDetection {
                files: vec!["package.json".to_string()],
                patterns: vec!["\"react\"".to_string(), "\"next\"".to_string()],
            }),
            inject_into: vec![InjectionPhase::Always],
            content: r#"---
name: react-next-platform
description: Apply React and Next.js implementation, routing, and rendering conventions.
tags: [react, nextjs, frontend, typescript]
tool-policy-mode: advisory
---

# React and Next.js

- Prefer server-first data fetching and keep client components narrowly scoped.
- Preserve route segment boundaries, loading states, and error boundaries.
- Keep mutations typed and colocate validation with the boundary that receives input.
- Favor incremental edits over rewrites so App Router files and shared layouts stay stable.
"#,
        },
        BuiltinSkillSpec {
            slug: "vue-nuxt",
            priority: 21,
            detect: Some(SkillDetection {
                files: vec!["package.json".to_string()],
                patterns: vec!["\"vue\"".to_string(), "\"nuxt\"".to_string()],
            }),
            inject_into: vec![InjectionPhase::Always],
            content: r#"---
name: vue-nuxt-platform
description: Apply Vue and Nuxt composition, routing, and SSR conventions.
tags: [vue, nuxt, frontend, typescript]
tool-policy-mode: advisory
---

# Vue and Nuxt

- Prefer Composition API patterns and keep composables reusable and side-effect light.
- Respect Nuxt server/client boundaries and keep data fetching aligned with route lifecycle.
- Keep state explicit and typed; avoid implicit global mutations in components.
- Preserve module and plugin registration order when making framework-level changes.
"#,
        },
        BuiltinSkillSpec {
            slug: "rust-tauri",
            priority: 22,
            detect: Some(SkillDetection {
                files: vec!["Cargo.toml".to_string(), "src-tauri/Cargo.toml".to_string()],
                patterns: vec!["tauri".to_string()],
            }),
            inject_into: vec![InjectionPhase::Always],
            content: r#"---
name: rust-tauri-platform
description: Apply Rust and Tauri conventions for commands, async boundaries, and desktop safety.
tags: [rust, tauri, desktop]
tool-policy-mode: advisory
---

# Rust and Tauri

- Keep command signatures stable and preserve serde payload compatibility.
- Prefer explicit error propagation and avoid hidden panics in command handlers.
- Maintain async boundaries cleanly: UI-facing commands return structured errors, services keep core logic.
- When modifying desktop behavior, account for both Tauri IPC shape and frontend store expectations.
"#,
        },
        BuiltinSkillSpec {
            slug: "typescript-node",
            priority: 23,
            detect: Some(SkillDetection {
                files: vec!["package.json".to_string(), "tsconfig.json".to_string()],
                patterns: vec!["typescript".to_string()],
            }),
            inject_into: vec![InjectionPhase::Always],
            content: r#"---
name: typescript-node-platform
description: Apply TypeScript and Node service conventions with strong runtime contracts.
tags: [typescript, node, backend]
tool-policy-mode: advisory
---

# TypeScript and Node

- Preserve runtime validation and do not trust compile-time types alone across boundaries.
- Keep side effects explicit and isolate environment-dependent behavior behind adapters.
- Prefer narrow, typed return shapes for IPC, RPC, and store-facing APIs.
- Update tests when changing public command payloads or store contracts.
"#,
        },
        BuiltinSkillSpec {
            slug: "python",
            priority: 24,
            detect: Some(SkillDetection {
                files: vec!["pyproject.toml".to_string(), "requirements.txt".to_string()],
                patterns: vec![],
            }),
            inject_into: vec![InjectionPhase::Always],
            content: r#"---
name: python-platform
description: Apply Python packaging, tooling, and readability conventions.
tags: [python, backend, scripting]
tool-policy-mode: advisory
---

# Python

- Keep modules import-safe and avoid work at import time.
- Prefer explicit data models and small pure helpers around IO boundaries.
- Preserve virtualenv and packaging conventions already present in the repo.
- Favor focused tests and deterministic command invocations for automation.
"#,
        },
        BuiltinSkillSpec {
            slug: "go",
            priority: 25,
            detect: Some(SkillDetection {
                files: vec!["go.mod".to_string()],
                patterns: vec![],
            }),
            inject_into: vec![InjectionPhase::Always],
            content: r#"---
name: go-platform
description: Apply Go package, interface, and concurrency conventions.
tags: [go, backend]
tool-policy-mode: advisory
---

# Go

- Keep package boundaries clear and avoid interface abstractions without a concrete need.
- Return wrapped errors with context and keep goroutine ownership explicit.
- Preserve module layout and avoid hidden global state.
- Prefer table-driven tests for behavior changes.
"#,
        },
        BuiltinSkillSpec {
            slug: "testing-workflow",
            priority: 26,
            detect: None,
            inject_into: vec![InjectionPhase::Implementation, InjectionPhase::Retry],
            content: r#"---
name: testing-workflow
description: Use targeted validation, regression checks, and failure triage during implementation.
tags: [testing, validation, workflow]
tool-policy-mode: advisory
---

# Testing Workflow

- Start with the narrowest check that validates the changed behavior.
- When a failure appears, isolate whether it is an existing issue, an environment issue, or a regression.
- Record unrun or failing checks explicitly in the final handoff.
- Do not broaden test scope blindly when a targeted check can prove the change.
"#,
        },
        BuiltinSkillSpec {
            slug: "refactor-workflow",
            priority: 27,
            detect: None,
            inject_into: vec![InjectionPhase::Implementation],
            content: r#"---
name: refactor-workflow
description: Use disciplined refactoring steps that preserve behavior while improving structure.
tags: [refactor, workflow]
tool-policy-mode: advisory
---

# Refactor Workflow

- Separate structural cleanup from behavior change whenever practical.
- Keep interface changes narrow and update all impacted call sites in the same pass.
- Prefer extraction and simplification over framework churn.
- Run the smallest meaningful regression check after each risky step.
"#,
        },
        BuiltinSkillSpec {
            slug: "release-workflow",
            priority: 28,
            detect: None,
            inject_into: vec![InjectionPhase::Planning, InjectionPhase::Implementation],
            content: r#"---
name: release-workflow
description: Apply release-oriented discipline for migrations, compatibility, and rollout safety.
tags: [release, migration, workflow]
tool-policy-mode: advisory
---

# Release Workflow

- Identify compatibility edges, migration needs, and rollback constraints before changing public behavior.
- Keep operator-facing diagnostics and failure modes explicit.
- Favor additive compatibility shims before removing legacy paths.
- Document rollout risks, hidden prerequisites, and validation steps.
"#,
        },
    ]
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
