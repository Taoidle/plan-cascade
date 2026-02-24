//! Universal SKILL.md Parser
//!
//! Parses three skill file formats:
//! - Format A: Plan Cascade SKILL.md (full YAML frontmatter with hooks, metadata, etc.)
//! - Format B: adk-rust .skills/ (lightweight frontmatter: name, description, tags)
//! - Format C: Convention files (no frontmatter, e.g. CLAUDE.md, AGENTS.md)
//!
//! The parser uses a lightweight YAML frontmatter parser that handles the subset
//! of YAML used by SKILL.md files without requiring a full YAML library.

use std::collections::HashMap;
use std::path::Path;

use crate::services::skills::model::{HookAction, ParsedSkill, SkillHooks, ToolHookRule};
use crate::utils::error::{AppError, AppResult};

/// Parse any skill file (Format A, B, or C) and return a ParsedSkill.
///
/// 1. Try to extract YAML frontmatter (between --- delimiters)
/// 2. If frontmatter found: parse all known fields with kebab-case to snake_case mapping
/// 3. If no frontmatter (convention file): derive name from filename, description from first heading
/// 4. Validate: name and description must be non-empty
pub fn parse_skill_file(path: &Path, content: &str) -> AppResult<ParsedSkill> {
    let (frontmatter, body) = extract_frontmatter(content);

    match frontmatter {
        Some(fm) => parse_with_frontmatter(&fm, &body, path),
        None => parse_convention_file(path, content),
    }
}

/// Extract YAML frontmatter between --- delimiters.
/// Returns (Some(frontmatter_text), body_text) or (None, full_content).
fn extract_frontmatter(content: &str) -> (Option<String>, String) {
    let trimmed = content.trim_start();

    if !trimmed.starts_with("---") {
        return (None, content.to_string());
    }

    // Find the opening ---
    let after_open = &trimmed[3..];
    let after_open = after_open.trim_start_matches(|c: char| c == '-'); // handle ---- etc
    let after_open = if after_open.starts_with('\n') {
        &after_open[1..]
    } else if after_open.starts_with("\r\n") {
        &after_open[2..]
    } else {
        after_open
    };

    // Find the closing ---
    if let Some(close_pos) = find_closing_delimiter(after_open) {
        let frontmatter = after_open[..close_pos].to_string();
        let rest = &after_open[close_pos..];
        // Skip past the closing --- line
        let body = rest
            .lines()
            .skip(1) // skip the --- line
            .collect::<Vec<_>>()
            .join("\n");
        let body = body.trim_start_matches('\n').to_string();
        (Some(frontmatter), body)
    } else {
        (None, content.to_string())
    }
}

/// Find the position of the closing --- delimiter in the text after the opening delimiter.
fn find_closing_delimiter(text: &str) -> Option<usize> {
    let mut pos = 0;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed == "---" || (trimmed.chars().all(|c| c == '-') && trimmed.len() >= 3) {
            return Some(pos);
        }
        pos += line.len() + 1; // +1 for newline
    }
    None
}

/// Parse a skill file that has YAML frontmatter (Format A or B).
fn parse_with_frontmatter(frontmatter: &str, body: &str, path: &Path) -> AppResult<ParsedSkill> {
    let fields = parse_yaml_fields(frontmatter);

    let name = fields
        .get("name")
        .map(|v| extract_string(v))
        .unwrap_or_default();
    let description = fields
        .get("description")
        .map(|v| extract_string(v))
        .unwrap_or_default();

    if name.is_empty() {
        return Err(AppError::parse(format!(
            "Skill file {} missing required 'name' field",
            path.display()
        )));
    }
    if description.is_empty() {
        return Err(AppError::parse(format!(
            "Skill file {} missing required 'description' field",
            path.display()
        )));
    }

    let version = fields.get("version").map(|v| extract_string(v));
    let tags = fields
        .get("tags")
        .map(|v| extract_string_list(v))
        .unwrap_or_default();
    let user_invocable = fields
        .get("user-invocable")
        .or_else(|| fields.get("user_invocable"))
        .map(|v| extract_bool(v))
        .unwrap_or(false);
    let allowed_tools = fields
        .get("allowed-tools")
        .or_else(|| fields.get("allowed_tools"))
        .map(|v| extract_string_list(v))
        .unwrap_or_default();
    let license = fields.get("license").map(|v| extract_string(v));

    // Parse hooks if present
    let hooks = parse_hooks(&fields);

    // Collect metadata from known metadata field and unknown top-level fields
    let mut metadata = HashMap::new();
    let known_fields = [
        "name",
        "description",
        "version",
        "tags",
        "user-invocable",
        "user_invocable",
        "allowed-tools",
        "allowed_tools",
        "license",
        "hooks",
        "metadata",
    ];

    // Parse explicit metadata field
    if let Some(meta_val) = fields.get("metadata") {
        if let YamlValue::Map(map) = meta_val {
            for (k, v) in map {
                metadata.insert(k.clone(), extract_string(v));
            }
        }
    }

    // Preserve unknown top-level fields in metadata
    for (key, val) in &fields {
        if !known_fields.contains(&key.as_str()) {
            metadata.insert(key.clone(), extract_string(val));
        }
    }

    Ok(ParsedSkill {
        name,
        description,
        version,
        tags,
        body: body.to_string(),
        user_invocable,
        allowed_tools,
        license,
        metadata,
        hooks,
    })
}

/// Parse a convention file without frontmatter (Format C).
/// Name is derived from filename (without extension), description from first heading.
fn parse_convention_file(path: &Path, content: &str) -> AppResult<ParsedSkill> {
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();

    // Description from first heading or first non-empty line
    let description = content
        .lines()
        .find(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty()
        })
        .map(|line| {
            let trimmed = line.trim();
            // Remove heading markers
            if trimmed.starts_with('#') {
                trimmed.trim_start_matches('#').trim().to_string()
            } else {
                trimmed.to_string()
            }
        })
        .unwrap_or_else(|| format!("Convention file: {}", name));

    Ok(ParsedSkill {
        name,
        description,
        version: None,
        tags: vec![],
        body: content.to_string(),
        user_invocable: false,
        allowed_tools: vec![],
        license: None,
        metadata: HashMap::new(),
        hooks: None,
    })
}

// --- Lightweight YAML subset parser ---

/// Represents a parsed YAML value (subset: strings, lists, maps)
#[derive(Debug, Clone)]
enum YamlValue {
    String(String),
    List(Vec<String>),
    Map(HashMap<String, YamlValue>),
    /// Multi-line block scalar (e.g. command: |)
    Block(String),
}

/// Parse YAML frontmatter into a flat map of field name -> YamlValue.
/// Handles: key: value, key: [list], key:\n  - item, nested maps (one level).
fn parse_yaml_fields(yaml: &str) -> HashMap<String, YamlValue> {
    let mut result = HashMap::new();
    let lines: Vec<&str> = yaml.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        // Skip empty lines and comments
        if trimmed.is_empty() || trimmed.starts_with('#') {
            i += 1;
            continue;
        }

        // Check for top-level key: value pair
        if let Some((key, value)) = parse_key_value(line) {
            if !key.contains(' ') && indent_level(line) == 0 {
                let value_trimmed = value.trim();

                if value_trimmed.is_empty() {
                    // Could be a nested map or list below
                    i += 1;
                    let (child_value, consumed) = parse_child_block(&lines, i);
                    result.insert(key, child_value);
                    i += consumed;
                } else if value_trimmed == "|" || value_trimmed == "|-" || value_trimmed == "|+" {
                    // Block scalar
                    i += 1;
                    let (block_text, consumed) = parse_block_scalar(&lines, i);
                    result.insert(key, YamlValue::Block(block_text));
                    i += consumed;
                } else if value_trimmed.starts_with('[') && value_trimmed.ends_with(']') {
                    // Inline list: [item1, item2]
                    let list = parse_inline_list(value_trimmed);
                    result.insert(key, YamlValue::List(list));
                    i += 1;
                } else {
                    // Simple string value
                    result.insert(key, YamlValue::String(unquote(value_trimmed)));
                    i += 1;
                }
            } else {
                i += 1;
            }
        } else {
            i += 1;
        }
    }

    result
}

/// Parse a key: value line. Returns (key, value_part).
fn parse_key_value(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim();
    if let Some(colon_pos) = trimmed.find(':') {
        let key = trimmed[..colon_pos].trim().to_string();
        let value = if colon_pos + 1 < trimmed.len() {
            trimmed[colon_pos + 1..].to_string()
        } else {
            String::new()
        };
        if !key.is_empty() && !key.starts_with('-') {
            Some((key, value))
        } else {
            None
        }
    } else {
        None
    }
}

/// Get the indentation level of a line (number of leading spaces).
fn indent_level(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// Parse a child block (indented below a key with no inline value).
/// Returns the parsed value and how many lines were consumed.
fn parse_child_block(lines: &[&str], start: usize) -> (YamlValue, usize) {
    if start >= lines.len() {
        return (YamlValue::String(String::new()), 0);
    }

    let first_line = lines[start].trim();
    let base_indent = indent_level(lines[start]);

    // Check if this is a list (starts with -)
    if first_line.starts_with("- ") || first_line == "-" {
        let mut items = Vec::new();
        let mut i = start;
        while i < lines.len() {
            let line = lines[i];
            let indent = indent_level(line);
            let trimmed = line.trim();

            if trimmed.is_empty() {
                i += 1;
                continue;
            }

            if indent < base_indent {
                break;
            }

            if indent == base_indent && trimmed.starts_with("- ") {
                items.push(unquote(trimmed[2..].trim()));
                i += 1;
            } else if indent > base_indent {
                // Continuation of previous item (multi-line or nested)
                i += 1;
            } else {
                break;
            }
        }
        (YamlValue::List(items), i - start)
    } else if first_line.contains(':') {
        // Nested map (one level deep for hooks support)
        let mut map = HashMap::new();
        let mut i = start;
        while i < lines.len() {
            let line = lines[i];
            let indent = indent_level(line);
            let trimmed = line.trim();

            if trimmed.is_empty() {
                i += 1;
                continue;
            }

            if indent < base_indent {
                break;
            }

            if indent == base_indent {
                if let Some((key, value)) = parse_key_value(trimmed) {
                    let value_trimmed = value.trim();
                    if value_trimmed.is_empty() {
                        i += 1;
                        // Collect nested content as a block
                        let mut block_lines = Vec::new();
                        while i < lines.len() {
                            let inner_line = lines[i];
                            let inner_indent = indent_level(inner_line);
                            if inner_line.trim().is_empty() {
                                block_lines.push("");
                                i += 1;
                                continue;
                            }
                            if inner_indent <= base_indent {
                                break;
                            }
                            block_lines.push(inner_line.trim());
                            i += 1;
                        }
                        map.insert(key, YamlValue::Block(block_lines.join("\n")));
                    } else {
                        map.insert(key, YamlValue::String(unquote(value_trimmed)));
                        i += 1;
                    }
                } else {
                    i += 1;
                }
            } else {
                i += 1;
            }
        }
        (YamlValue::Map(map), i - start)
    } else {
        (YamlValue::String(first_line.to_string()), 1)
    }
}

/// Parse a YAML block scalar (lines after | indicator).
fn parse_block_scalar(lines: &[&str], start: usize) -> (String, usize) {
    let mut block_lines = Vec::new();
    let mut i = start;

    if i >= lines.len() {
        return (String::new(), 0);
    }

    let base_indent = indent_level(lines[i]);

    while i < lines.len() {
        let line = lines[i];
        if line.trim().is_empty() {
            block_lines.push(String::new());
            i += 1;
            continue;
        }

        let indent = indent_level(line);
        if indent < base_indent {
            break;
        }

        // Remove the base indentation
        if line.len() > base_indent {
            block_lines.push(line[base_indent..].to_string());
        } else {
            block_lines.push(String::new());
        }
        i += 1;
    }

    // Trim trailing empty lines
    while block_lines.last().map_or(false, |l| l.is_empty()) {
        block_lines.pop();
    }

    (block_lines.join("\n"), i - start)
}

/// Parse an inline YAML list: [item1, item2, "item3"]
fn parse_inline_list(text: &str) -> Vec<String> {
    let inner = text.trim_start_matches('[').trim_end_matches(']');
    inner
        .split(',')
        .map(|s| unquote(s.trim()))
        .filter(|s| !s.is_empty())
        .collect()
}

/// Remove surrounding quotes from a string value.
fn unquote(s: &str) -> String {
    let s = s.trim();
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

/// Extract a string from a YamlValue.
fn extract_string(val: &YamlValue) -> String {
    match val {
        YamlValue::String(s) => s.clone(),
        YamlValue::Block(s) => s.clone(),
        YamlValue::List(items) => items.join(", "),
        YamlValue::Map(_) => String::new(),
    }
}

/// Extract a list of strings from a YamlValue.
fn extract_string_list(val: &YamlValue) -> Vec<String> {
    match val {
        YamlValue::List(items) => items.clone(),
        YamlValue::String(s) => {
            // Could be a single-item "list"
            if s.is_empty() {
                vec![]
            } else {
                vec![s.clone()]
            }
        }
        _ => vec![],
    }
}

/// Extract a boolean from a YamlValue.
fn extract_bool(val: &YamlValue) -> bool {
    match val {
        YamlValue::String(s) => matches!(s.to_lowercase().as_str(), "true" | "yes" | "1"),
        _ => false,
    }
}

/// Parse hooks from the fields map.
fn parse_hooks(fields: &HashMap<String, YamlValue>) -> Option<SkillHooks> {
    let hooks_val = fields.get("hooks")?;

    match hooks_val {
        YamlValue::Map(map) => {
            let pre_tool_use =
                parse_hook_rules(map.get("PreToolUse").or_else(|| map.get("pre_tool_use")));
            let post_tool_use =
                parse_hook_rules(map.get("PostToolUse").or_else(|| map.get("post_tool_use")));
            let stop = parse_stop_hooks(map.get("Stop").or_else(|| map.get("stop")));

            if pre_tool_use.is_empty() && post_tool_use.is_empty() && stop.is_empty() {
                None
            } else {
                Some(SkillHooks {
                    pre_tool_use,
                    post_tool_use,
                    stop,
                })
            }
        }
        _ => None,
    }
}

/// Parse hook rules from a YAML value (simplified - extracts what we can).
fn parse_hook_rules(_val: Option<&YamlValue>) -> Vec<ToolHookRule> {
    // Hook parsing from the lightweight YAML parser is limited.
    // In production, hooks are complex nested YAML structures.
    // For now we return empty - hooks will be fully supported when
    // we add serde_yaml dependency in a future iteration.
    vec![]
}

/// Parse stop hooks from a YAML value.
fn parse_stop_hooks(_val: Option<&YamlValue>) -> Vec<HookAction> {
    vec![]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_parse_format_a_full_frontmatter() {
        let content = r#"---
name: hybrid-ralph
version: "3.2.0"
description: Hybrid architecture combining Ralph's PRD format with parallel execution
user-invocable: true
allowed-tools:
  - Read
  - Write
  - Edit
  - Bash
license: MIT
metadata:
  author: vercel
  version: "1.0.0"
tags: [planning, architecture]
---

# Hybrid Ralph

This skill provides hybrid architecture planning.

## Workflow
1. Analyze task
2. Generate PRD
"#;
        let result = parse_skill_file(&PathBuf::from("/skills/hybrid-ralph/SKILL.md"), content);
        assert!(result.is_ok(), "Parse failed: {:?}", result.err());

        let skill = result.unwrap();
        assert_eq!(skill.name, "hybrid-ralph");
        assert_eq!(skill.version.as_deref(), Some("3.2.0"));
        assert!(skill.description.contains("Hybrid architecture"));
        assert!(skill.user_invocable);
        assert_eq!(skill.allowed_tools, vec!["Read", "Write", "Edit", "Bash"]);
        assert_eq!(skill.license.as_deref(), Some("MIT"));
        assert_eq!(skill.tags, vec!["planning", "architecture"]);
        assert_eq!(
            skill.metadata.get("author").map(|s| s.as_str()),
            Some("vercel")
        );
        assert!(skill.body.contains("# Hybrid Ralph"));
        assert!(skill.body.contains("## Workflow"));
    }

    #[test]
    fn test_parse_format_b_lightweight() {
        let content = r#"---
name: adk-rust-app-bootstrap
description: Bootstrap new ADK-Rust applications with correct crate/features
tags: [bootstrap, setup]
---

# ADK Rust App Bootstrap

## Workflow
1. Choose dependency scope
2. Select provider feature flags
"#;
        let result = parse_skill_file(&PathBuf::from("/skills/bootstrap/SKILL.md"), content);
        assert!(result.is_ok());

        let skill = result.unwrap();
        assert_eq!(skill.name, "adk-rust-app-bootstrap");
        assert!(skill.description.contains("Bootstrap new ADK-Rust"));
        assert_eq!(skill.tags, vec!["bootstrap", "setup"]);
        assert!(!skill.user_invocable);
        assert!(skill.allowed_tools.is_empty());
        assert!(skill.version.is_none());
        assert!(skill.body.contains("# ADK Rust App Bootstrap"));
    }

    #[test]
    fn test_parse_format_c_convention_file() {
        let content = r#"# CLAUDE.md

This file provides guidance to Claude Code when working with code in this repository.

## Build Commands

```bash
pnpm install
pnpm dev
```

## Architecture

Three-layer structure with React frontend and Rust backend.
"#;
        let result = parse_skill_file(&PathBuf::from("/project/CLAUDE.md"), content);
        assert!(result.is_ok());

        let skill = result.unwrap();
        assert_eq!(skill.name, "CLAUDE");
        assert_eq!(skill.description, "CLAUDE.md");
        assert!(skill.tags.is_empty());
        assert!(skill.body.contains("This file provides guidance"));
        assert!(skill.body.contains("## Build Commands"));
    }

    #[test]
    fn test_parse_convention_file_agents_md() {
        let content = r#"# Custom Agents Configuration

Define your project-specific agents here.
"#;
        let result = parse_skill_file(&PathBuf::from("/project/AGENTS.md"), content);
        assert!(result.is_ok());

        let skill = result.unwrap();
        assert_eq!(skill.name, "AGENTS");
        assert_eq!(skill.description, "Custom Agents Configuration");
    }

    #[test]
    fn test_parse_missing_name_fails() {
        let content = r#"---
description: A skill without a name
---

# Body
"#;
        let result = parse_skill_file(&PathBuf::from("/skills/test.md"), content);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("missing required 'name'"));
    }

    #[test]
    fn test_parse_missing_description_fails() {
        let content = r#"---
name: no-desc
---

# Body
"#;
        let result = parse_skill_file(&PathBuf::from("/skills/test.md"), content);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("missing required 'description'"));
    }

    #[test]
    fn test_extract_frontmatter_basic() {
        let content = "---\nname: test\n---\n\n# Body";
        let (fm, body) = extract_frontmatter(content);
        assert!(fm.is_some());
        assert!(fm.unwrap().contains("name: test"));
        assert!(body.contains("# Body"));
    }

    #[test]
    fn test_extract_frontmatter_no_delimiter() {
        let content = "# Just a heading\n\nSome content";
        let (fm, body) = extract_frontmatter(content);
        assert!(fm.is_none());
        assert_eq!(body, content);
    }

    #[test]
    fn test_inline_list_parsing() {
        let list = parse_inline_list("[alpha, beta, gamma]");
        assert_eq!(list, vec!["alpha", "beta", "gamma"]);
    }

    #[test]
    fn test_inline_list_with_quotes() {
        let list = parse_inline_list("[\"react\", \"next\", \"@react\"]");
        assert_eq!(list, vec!["react", "next", "@react"]);
    }

    #[test]
    fn test_unquote() {
        assert_eq!(unquote("\"hello\""), "hello");
        assert_eq!(unquote("'world'"), "world");
        assert_eq!(unquote("plain"), "plain");
        assert_eq!(unquote("  \"spaced\"  "), "spaced");
    }

    #[test]
    fn test_parse_yaml_fields_basic() {
        let yaml = "name: test-skill\ndescription: A test skill\nversion: \"1.0.0\"";
        let fields = parse_yaml_fields(yaml);

        assert_eq!(extract_string(fields.get("name").unwrap()), "test-skill");
        assert_eq!(
            extract_string(fields.get("description").unwrap()),
            "A test skill"
        );
        assert_eq!(extract_string(fields.get("version").unwrap()), "1.0.0");
    }

    #[test]
    fn test_parse_yaml_fields_with_list() {
        let yaml = "name: test\ntags:\n  - alpha\n  - beta\n  - gamma";
        let fields = parse_yaml_fields(yaml);

        let tags = extract_string_list(fields.get("tags").unwrap());
        assert_eq!(tags, vec!["alpha", "beta", "gamma"]);
    }

    #[test]
    fn test_parse_yaml_fields_bool() {
        let yaml = "name: test\nuser-invocable: true";
        let fields = parse_yaml_fields(yaml);

        assert!(extract_bool(fields.get("user-invocable").unwrap()));
    }

    #[test]
    fn test_parse_yaml_fields_nested_metadata() {
        let yaml = "name: test\nmetadata:\n  author: vercel\n  version: \"1.0.0\"";
        let fields = parse_yaml_fields(yaml);

        if let Some(YamlValue::Map(map)) = fields.get("metadata") {
            assert_eq!(extract_string(map.get("author").unwrap()), "vercel");
            assert_eq!(extract_string(map.get("version").unwrap()), "1.0.0");
        } else {
            panic!("metadata should be a Map");
        }
    }

    #[test]
    fn test_parse_preserves_unknown_fields() {
        let content = r#"---
name: test-skill
description: A test skill
custom-field: some-value
another: thing
---

# Body
"#;
        let result = parse_skill_file(&PathBuf::from("/test/SKILL.md"), content).unwrap();
        assert_eq!(
            result.metadata.get("custom-field").map(|s| s.as_str()),
            Some("some-value")
        );
        assert_eq!(
            result.metadata.get("another").map(|s| s.as_str()),
            Some("thing")
        );
    }

    #[test]
    fn test_parse_kebab_case_mapping() {
        let content = r#"---
name: kebab-test
description: Test kebab case mapping
user-invocable: true
allowed-tools:
  - Read
  - Write
---

# Body
"#;
        let result = parse_skill_file(&PathBuf::from("/test/SKILL.md"), content).unwrap();
        assert!(result.user_invocable);
        assert_eq!(result.allowed_tools, vec!["Read", "Write"]);
    }

    #[test]
    fn test_convention_file_empty_content() {
        let content = "";
        let result = parse_skill_file(&PathBuf::from("/project/CLAUDE.md"), content);
        assert!(result.is_ok());
        let skill = result.unwrap();
        assert_eq!(skill.name, "CLAUDE");
        // Empty content should derive description from name
        assert!(skill.description.contains("CLAUDE"));
    }

    #[test]
    fn test_convention_file_no_heading() {
        let content = "Some text without a heading marker.\nMore text.";
        let result = parse_skill_file(&PathBuf::from("/project/NOTES.md"), content).unwrap();
        assert_eq!(result.name, "NOTES");
        assert_eq!(result.description, "Some text without a heading marker.");
    }
}
