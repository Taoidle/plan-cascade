//! Spec Compiler
//!
//! Generates spec.json and spec.md from completed interview data,
//! and compiles to PRD format compatible with Plan Cascade execution.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;

use crate::utils::error::AppResult;

/// Options for compiling spec into PRD
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompileOptions {
    /// Override description for PRD metadata
    #[serde(default)]
    pub description: String,
    /// Flow level: "quick", "standard", "full"
    #[serde(default)]
    pub flow_level: Option<String>,
    /// TDD mode: "off", "on", "auto"
    #[serde(default)]
    pub tdd_mode: Option<String>,
    /// Whether batch confirmation is required
    #[serde(default)]
    pub confirm: bool,
    /// Whether to skip batch confirmation
    #[serde(default)]
    pub no_confirm: bool,
}

impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            description: String::new(),
            flow_level: None,
            tdd_mode: None,
            confirm: false,
            no_confirm: false,
        }
    }
}

/// Compiled spec output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledSpec {
    /// The spec.json content
    pub spec_json: Value,
    /// The spec.md content (rendered markdown)
    pub spec_md: String,
    /// The compiled PRD (prd.json content)
    pub prd_json: Value,
}

/// The spec compiler service
pub struct SpecCompiler;

impl SpecCompiler {
    /// Compile interview spec data into spec.json, spec.md, and prd.json
    pub fn compile(spec_data: &Value, options: &CompileOptions) -> AppResult<CompiledSpec> {
        // Build the full spec.json structure
        let spec_json = Self::build_spec_json(spec_data)?;

        // Render spec.md from the spec
        let spec_md = Self::render_spec_md(&spec_json);

        // Compile spec.json into prd.json
        let prd_json = Self::compile_to_prd(&spec_json, options)?;

        Ok(CompiledSpec {
            spec_json,
            spec_md,
            prd_json,
        })
    }

    /// Build the canonical spec.json from interview data
    fn build_spec_json(spec_data: &Value) -> AppResult<Value> {
        let now = Utc::now().to_rfc3339();

        let overview = spec_data.get("overview").cloned().unwrap_or(json!({}));
        let scope = spec_data.get("scope").cloned().unwrap_or(json!({}));
        let requirements = spec_data.get("requirements").cloned().unwrap_or(json!({}));
        let interfaces = spec_data.get("interfaces").cloned().unwrap_or(json!({}));
        let stories = spec_data.get("stories").cloned().unwrap_or(json!([]));
        let open_questions = spec_data.get("open_questions").cloned().unwrap_or(json!([]));

        let spec = json!({
            "metadata": {
                "schema_version": "spec-0.1",
                "source": "spec-interview",
                "created_at": now,
                "updated_at": now,
            },
            "overview": overview,
            "scope": scope,
            "requirements": requirements,
            "interfaces": interfaces,
            "stories": stories,
            "phases": [],
            "decision_log": [],
            "open_questions": open_questions,
        });

        Ok(spec)
    }

    /// Render spec.json into human-readable markdown
    fn render_spec_md(spec: &Value) -> String {
        let mut md = Vec::new();

        let overview = spec.get("overview").cloned().unwrap_or(json!({}));
        let scope = spec.get("scope").cloned().unwrap_or(json!({}));
        let reqs = spec.get("requirements").cloned().unwrap_or(json!({}));
        let nfr = reqs.get("non_functional").cloned().unwrap_or(json!({}));
        let interfaces = spec.get("interfaces").cloned().unwrap_or(json!({}));
        let metadata = spec.get("metadata").cloned().unwrap_or(json!({}));

        let title = overview.get("title").and_then(|v| v.as_str()).unwrap_or("Specification");
        let goal = overview.get("goal").and_then(|v| v.as_str()).unwrap_or("");
        let problem = overview.get("problem").and_then(|v| v.as_str()).unwrap_or("");

        // Header
        md.push(format!("# Spec: {}", title));
        md.push(String::new());
        md.push(format!(
            "**Schema:** `{}`",
            metadata.get("schema_version").and_then(|v| v.as_str()).unwrap_or("")
        ));
        md.push(format!(
            "**Created:** `{}`",
            metadata.get("created_at").and_then(|v| v.as_str()).unwrap_or("")
        ));
        md.push(format!(
            "**Updated:** `{}`",
            metadata.get("updated_at").and_then(|v| v.as_str()).unwrap_or("")
        ));
        md.push(String::new());

        // Goal
        if !goal.is_empty() {
            md.push("## Goal".to_string());
            md.push(String::new());
            md.push(goal.to_string());
            md.push(String::new());
        }

        // Problem
        if !problem.is_empty() {
            md.push("## Problem".to_string());
            md.push(String::new());
            md.push(problem.to_string());
            md.push(String::new());
        }

        // Success Metrics
        md.push("## Success Metrics".to_string());
        md.push(String::new());
        Self::render_list(&mut md, &overview, "success_metrics");

        // Non-goals
        md.push("## Non-goals".to_string());
        md.push(String::new());
        Self::render_list(&mut md, &overview, "non_goals");

        // Scope
        md.push("## Scope".to_string());
        md.push(String::new());
        md.push("### In scope".to_string());
        md.push(String::new());
        Self::render_list(&mut md, &scope, "in_scope");
        md.push("### Out of scope".to_string());
        md.push(String::new());
        Self::render_list(&mut md, &scope, "out_of_scope");
        md.push("### Do not touch".to_string());
        md.push(String::new());
        Self::render_list(&mut md, &scope, "do_not_touch");
        md.push("### Assumptions".to_string());
        md.push(String::new());
        Self::render_list(&mut md, &scope, "assumptions");

        // Requirements
        md.push("## Requirements".to_string());
        md.push(String::new());
        md.push("### Functional".to_string());
        md.push(String::new());
        Self::render_list(&mut md, &reqs, "functional");

        md.push("### Non-functional".to_string());
        md.push(String::new());
        for section in &["performance_targets", "security", "reliability", "scalability", "accessibility"] {
            let label = section.replace('_', " ");
            let label = format!("{}{}",
                label.chars().next().unwrap().to_uppercase(),
                &label[1..]
            );
            md.push(format!("#### {}", label));
            md.push(String::new());
            Self::render_list(&mut md, &nfr, section);
        }

        // Interfaces
        md.push("## Interfaces".to_string());
        md.push(String::new());
        md.push("### API".to_string());
        md.push(String::new());
        if let Some(api) = interfaces.get("api").and_then(|v| v.as_array()) {
            if api.is_empty() {
                md.push("_(none)_".to_string());
            } else {
                for item in api {
                    if let Some(obj) = item.as_object() {
                        let name = obj.get("name").and_then(|v| v.as_str()).unwrap_or("");
                        let notes = obj.get("notes").and_then(|v| v.as_str()).unwrap_or("");
                        if notes.is_empty() {
                            md.push(format!("- `{}`", name));
                        } else {
                            md.push(format!("- `{}` -- {}", name, notes));
                        }
                    } else if let Some(s) = item.as_str() {
                        md.push(format!("- {}", s));
                    }
                }
            }
        } else {
            md.push("_(none)_".to_string());
        }
        md.push(String::new());

        md.push("### Data models".to_string());
        md.push(String::new());
        if let Some(models) = interfaces.get("data_models").and_then(|v| v.as_array()) {
            if models.is_empty() {
                md.push("_(none)_".to_string());
            } else {
                for item in models {
                    if let Some(obj) = item.as_object() {
                        let name = obj.get("name").and_then(|v| v.as_str()).unwrap_or("");
                        let fields = obj
                            .get("fields")
                            .and_then(|v| v.as_array())
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|f| f.as_str())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            })
                            .unwrap_or_default();
                        md.push(format!("- `{}`: {}", name, fields));
                    }
                }
            }
        } else {
            md.push("_(none)_".to_string());
        }
        md.push(String::new());

        // Stories
        md.push("## Stories".to_string());
        md.push(String::new());
        if let Some(stories) = spec.get("stories").and_then(|v| v.as_array()) {
            if stories.is_empty() {
                md.push("_(none)_".to_string());
            } else {
                for story in stories {
                    let obj = story.as_object();
                    if obj.is_none() {
                        continue;
                    }
                    let obj = obj.unwrap();
                    let id = obj.get("id").and_then(|v| v.as_str()).unwrap_or("???");
                    let title = obj.get("title").and_then(|v| v.as_str()).unwrap_or("Untitled");
                    let category = obj.get("category").and_then(|v| v.as_str()).unwrap_or("core");
                    let context_est = obj.get("context_estimate").and_then(|v| v.as_str()).unwrap_or("medium");
                    let description = obj.get("description").and_then(|v| v.as_str()).unwrap_or("");

                    md.push(format!("### {}: {}", id, title));
                    md.push(String::new());
                    md.push(format!("- **Category:** `{}`", category));
                    md.push(format!("- **Context estimate:** `{}`", context_est));

                    if let Some(deps) = obj.get("dependencies").and_then(|v| v.as_array()) {
                        if !deps.is_empty() {
                            let dep_strs: Vec<&str> = deps.iter().filter_map(|d| d.as_str()).collect();
                            md.push(format!("- **Dependencies:** {}", dep_strs.join(", ")));
                        }
                    }
                    md.push(String::new());

                    if !description.is_empty() {
                        md.push(description.to_string());
                        md.push(String::new());
                    }

                    md.push("**Acceptance criteria:**".to_string());
                    md.push(String::new());
                    if let Some(ac) = obj.get("acceptance_criteria").and_then(|v| v.as_array()) {
                        for item in ac {
                            if let Some(s) = item.as_str() {
                                md.push(format!("- {}", s));
                            }
                        }
                    } else {
                        md.push("_(none)_".to_string());
                    }
                    md.push(String::new());
                    md.push("---".to_string());
                    md.push(String::new());
                }
            }
        } else {
            md.push("_(none)_".to_string());
        }
        md.push(String::new());

        // Open Questions
        md.push("## Open Questions".to_string());
        md.push(String::new());
        Self::render_list(&mut md, spec, "open_questions");

        md.join("\n")
    }

    /// Render a list field from a JSON object
    fn render_list(md: &mut Vec<String>, obj: &Value, field: &str) {
        if let Some(arr) = obj.get(field).and_then(|v| v.as_array()) {
            if arr.is_empty() {
                md.push("_(none)_".to_string());
            } else {
                for item in arr {
                    if let Some(s) = item.as_str() {
                        if !s.trim().is_empty() {
                            md.push(format!("- {}", s.trim()));
                        }
                    }
                }
            }
        } else {
            md.push("_(none)_".to_string());
        }
        md.push(String::new());
    }

    /// Compile spec.json into a Plan Cascade PRD dict
    fn compile_to_prd(spec: &Value, options: &CompileOptions) -> AppResult<Value> {
        let overview = spec.get("overview").cloned().unwrap_or(json!({}));
        let requirements = spec.get("requirements").cloned().unwrap_or(json!({}));
        let scope = spec.get("scope").cloned().unwrap_or(json!({}));

        // Goal: prefer overview.goal, fallback to overview.title
        let goal = overview
            .get("goal")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .or_else(|| overview.get("title").and_then(|v| v.as_str()).filter(|s| !s.is_empty()))
            .unwrap_or("Complete the task")
            .to_string();

        // Objectives: prefer functional requirements, fallback to scope.in_scope
        let mut objectives: Vec<String> = requirements
            .get("functional")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default();

        if objectives.is_empty() {
            objectives = scope
                .get("in_scope")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str())
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect()
                })
                .unwrap_or_default();
        }

        let mut notes: Vec<String> = vec![];
        if objectives.len() > 7 {
            notes.push("Objectives truncated to 7; see spec.json for full list.".to_string());
            objectives.truncate(7);
        }

        let description = if options.description.is_empty() {
            spec.get("metadata")
                .and_then(|m| m.get("description"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        } else {
            options.description.clone()
        };

        let mut prd_metadata = json!({
            "created_at": Utc::now().to_rfc3339(),
            "version": "1.0.0",
            "description": description,
            "source": "spec-compile",
            "spec_schema_version": spec.get("metadata")
                .and_then(|m| m.get("schema_version"))
                .and_then(|v| v.as_str())
                .unwrap_or(""),
        });

        if !notes.is_empty() {
            prd_metadata.as_object_mut().unwrap().insert(
                "notes".to_string(),
                json!(notes),
            );
        }

        // Category-to-priority defaults
        let category_priority: HashMap<&str, &str> = [
            ("setup", "high"),
            ("core", "high"),
            ("integration", "medium"),
            ("polish", "low"),
            ("test", "medium"),
        ]
        .into_iter()
        .collect();

        // Compile stories
        let mut prd_stories: Vec<Value> = vec![];
        if let Some(stories) = spec.get("stories").and_then(|v| v.as_array()) {
            for (idx, story) in stories.iter().enumerate() {
                let obj = story.as_object();
                if obj.is_none() {
                    continue;
                }
                let obj = obj.unwrap();

                let story_id = obj
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&format!("story-{:03}", idx + 1))
                    .to_string();
                let category = obj
                    .get("category")
                    .and_then(|v| v.as_str())
                    .unwrap_or("core")
                    .to_string();
                let title = obj
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&format!("Story {}", idx + 1))
                    .to_string();
                let desc = obj
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&title)
                    .to_string();

                let priority = obj
                    .get("priority")
                    .and_then(|v| v.as_str())
                    .unwrap_or_else(|| {
                        category_priority.get(category.as_str()).copied().unwrap_or("medium")
                    })
                    .to_string();

                let dependencies = obj
                    .get("dependencies")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();
                let acceptance_criteria = obj
                    .get("acceptance_criteria")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();
                let context_estimate = obj
                    .get("context_estimate")
                    .and_then(|v| v.as_str())
                    .unwrap_or("medium")
                    .to_string();

                let mut prd_story = json!({
                    "id": story_id,
                    "title": title,
                    "description": desc,
                    "priority": priority,
                    "dependencies": dependencies,
                    "status": "pending",
                    "acceptance_criteria": acceptance_criteria,
                    "context_estimate": context_estimate,
                    "tags": [format!("category:{}", category)],
                });

                // Verification commands
                if let Some(verification) = obj.get("verification").and_then(|v| v.as_object()) {
                    if let Some(commands) = verification.get("commands").and_then(|v| v.as_array()) {
                        if !commands.is_empty() {
                            prd_story.as_object_mut().unwrap().insert(
                                "verification_commands".to_string(),
                                json!(commands),
                            );
                        }
                    }
                    if let Some(manual) = verification.get("manual_steps").and_then(|v| v.as_array()) {
                        if !manual.is_empty() {
                            prd_story.as_object_mut().unwrap().insert(
                                "verification_manual_steps".to_string(),
                                json!(manual),
                            );
                        }
                    }
                }

                // Test expectations
                if let Some(test_exp) = obj.get("test_expectations") {
                    if test_exp.is_object() {
                        prd_story.as_object_mut().unwrap().insert(
                            "test_expectations".to_string(),
                            test_exp.clone(),
                        );
                    }
                }

                prd_stories.push(prd_story);
            }
        }

        let mut prd = json!({
            "metadata": prd_metadata,
            "goal": goal,
            "objectives": objectives,
            "stories": prd_stories,
        });

        // Flow config
        if let Some(ref flow_level) = options.flow_level {
            let flow_config = json!({
                "level": flow_level,
                "source": "spec-compile",
            });
            if flow_level == "full" {
                prd.as_object_mut().unwrap().insert(
                    "verification_gate".to_string(),
                    json!({ "enabled": true, "required": true }),
                );
                prd.as_object_mut().unwrap().insert(
                    "code_review".to_string(),
                    json!({ "enabled": true, "required": true }),
                );
            }
            prd.as_object_mut().unwrap().insert(
                "flow_config".to_string(),
                flow_config,
            );
        }

        // TDD config
        if let Some(ref tdd_mode) = options.tdd_mode {
            prd.as_object_mut().unwrap().insert(
                "tdd_config".to_string(),
                json!({
                    "mode": tdd_mode,
                    "enforce_for_high_risk": true,
                    "test_requirements": {
                        "require_test_changes": tdd_mode == "on",
                        "require_test_for_high_risk": true,
                        "minimum_coverage_delta": 0.0,
                        "test_patterns": ["test_", "_test.", ".test.", "tests/", "test/", "spec/"],
                    }
                }),
            );
        }

        // Confirm config
        if options.no_confirm {
            prd.as_object_mut().unwrap().insert(
                "execution_config".to_string(),
                json!({
                    "require_batch_confirm": false,
                    "no_confirm_override": true,
                }),
            );
        } else if options.confirm {
            prd.as_object_mut().unwrap().insert(
                "execution_config".to_string(),
                json!({
                    "require_batch_confirm": true,
                }),
            );
        }

        Ok(prd)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_spec_json() {
        let spec_data = json!({
            "overview": {
                "title": "Test Project",
                "goal": "Build a test system",
            },
            "scope": {
                "in_scope": ["Feature A", "Feature B"],
            },
            "requirements": {
                "functional": ["Must do X", "Must do Y"],
            },
            "stories": [
                {
                    "id": "story-001",
                    "title": "Setup",
                    "category": "setup",
                    "description": "Initial setup",
                    "acceptance_criteria": ["Project compiles"],
                    "verification": { "commands": ["cargo build"], "manual_steps": [] },
                    "dependencies": [],
                    "context_estimate": "small"
                }
            ]
        });

        let spec = SpecCompiler::build_spec_json(&spec_data).unwrap();
        assert_eq!(
            spec.get("overview")
                .and_then(|o| o.get("title"))
                .and_then(|t| t.as_str()),
            Some("Test Project")
        );
        assert!(spec.get("metadata").is_some());
        assert_eq!(
            spec.get("metadata")
                .and_then(|m| m.get("schema_version"))
                .and_then(|v| v.as_str()),
            Some("spec-0.1")
        );
    }

    #[test]
    fn test_render_spec_md() {
        let spec = json!({
            "metadata": {
                "schema_version": "spec-0.1",
                "created_at": "2024-01-01T00:00:00Z",
                "updated_at": "2024-01-01T00:00:00Z",
            },
            "overview": {
                "title": "My Spec",
                "goal": "Build something",
                "success_metrics": ["Works correctly"],
                "non_goals": [],
            },
            "scope": {
                "in_scope": ["Core feature"],
                "out_of_scope": [],
                "do_not_touch": [],
                "assumptions": [],
            },
            "requirements": {
                "functional": ["Must work"],
                "non_functional": {},
            },
            "interfaces": {},
            "stories": [],
            "open_questions": [],
        });

        let md = SpecCompiler::render_spec_md(&spec);
        assert!(md.contains("# Spec: My Spec"));
        assert!(md.contains("## Goal"));
        assert!(md.contains("Build something"));
        assert!(md.contains("- Works correctly"));
    }

    #[test]
    fn test_compile_to_prd() {
        let spec = json!({
            "metadata": {
                "schema_version": "spec-0.1",
                "description": "Test",
            },
            "overview": {
                "title": "Test PRD",
                "goal": "Complete the feature",
            },
            "requirements": {
                "functional": ["Do X", "Do Y"],
            },
            "scope": {},
            "stories": [
                {
                    "id": "story-001",
                    "title": "First Story",
                    "category": "core",
                    "description": "Do the first thing",
                    "acceptance_criteria": ["Done correctly"],
                    "verification": { "commands": ["cargo test"], "manual_steps": [] },
                    "dependencies": [],
                    "context_estimate": "medium"
                }
            ]
        });

        let options = CompileOptions::default();
        let prd = SpecCompiler::compile_to_prd(&spec, &options).unwrap();

        assert_eq!(
            prd.get("goal").and_then(|v| v.as_str()),
            Some("Complete the feature")
        );
        assert_eq!(
            prd.get("objectives").and_then(|v| v.as_array()).map(|a| a.len()),
            Some(2)
        );
        assert_eq!(
            prd.get("stories").and_then(|v| v.as_array()).map(|a| a.len()),
            Some(1)
        );

        let story = &prd["stories"][0];
        assert_eq!(story.get("id").and_then(|v| v.as_str()), Some("story-001"));
        assert_eq!(story.get("status").and_then(|v| v.as_str()), Some("pending"));
        assert_eq!(story.get("priority").and_then(|v| v.as_str()), Some("high"));
    }

    #[test]
    fn test_compile_with_flow_options() {
        let spec = json!({
            "metadata": { "schema_version": "spec-0.1" },
            "overview": { "goal": "Test" },
            "requirements": {},
            "stories": []
        });

        let options = CompileOptions {
            flow_level: Some("full".to_string()),
            tdd_mode: Some("on".to_string()),
            no_confirm: true,
            ..Default::default()
        };

        let prd = SpecCompiler::compile_to_prd(&spec, &options).unwrap();

        assert!(prd.get("flow_config").is_some());
        assert!(prd.get("verification_gate").is_some());
        assert!(prd.get("tdd_config").is_some());
        assert!(prd.get("execution_config").is_some());
        assert_eq!(
            prd["execution_config"]["no_confirm_override"].as_bool(),
            Some(true)
        );
    }

    #[test]
    fn test_full_compile() {
        let spec_data = json!({
            "overview": {
                "title": "Full Test",
                "goal": "Full compile test",
                "success_metrics": ["All tests pass"],
            },
            "requirements": {
                "functional": ["Feature A"],
            },
            "stories": [
                {
                    "id": "story-001",
                    "title": "Setup",
                    "category": "setup",
                    "description": "Initial setup",
                    "acceptance_criteria": [],
                    "verification": { "commands": [], "manual_steps": [] },
                    "dependencies": [],
                    "context_estimate": "small"
                }
            ]
        });

        let result = SpecCompiler::compile(&spec_data, &CompileOptions::default()).unwrap();

        // spec_json should be valid
        assert!(result.spec_json.get("metadata").is_some());
        assert!(result.spec_json.get("stories").is_some());

        // spec_md should be non-empty
        assert!(!result.spec_md.is_empty());
        assert!(result.spec_md.contains("Full Test"));

        // prd_json should have stories
        assert!(result.prd_json.get("stories").is_some());
    }
}
