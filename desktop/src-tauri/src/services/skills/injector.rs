//! Skill Injection
//!
//! Formats matched skills into system prompt sections for LLM consumption.

use crate::services::skills::model::{SelectionPolicy, SkillDocument, SkillMatch, SkillSource};

/// Inject matched skills into a system prompt section.
///
/// Produces a formatted section like:
///
/// ```text
/// ## Framework-Specific Best Practices
///
/// The following guidelines apply based on detected frameworks:
///
/// ### Skill Name
/// *Source: vercel (external) | Priority: 100*
///
/// {skill body, truncated to max_content_lines}
///
/// ---
/// ```
pub fn inject_skills(
    matched_skills: &[SkillMatch],
    skill_docs: &[&SkillDocument],
    policy: &SelectionPolicy,
) -> String {
    if matched_skills.is_empty() {
        return String::new();
    }

    let mut output = String::new();
    output.push_str("\n## Framework-Specific Best Practices\n\n");
    output.push_str("The following guidelines apply based on detected frameworks:\n");

    for (i, skill_match) in matched_skills.iter().enumerate() {
        let body = skill_docs
            .iter()
            .find(|d| d.id == skill_match.skill.id)
            .map(|d| d.body.as_str())
            .unwrap_or("");

        let source_label = format_source(&skill_match.skill.source);
        let truncated_body = truncate_body(body, policy.max_content_lines);

        output.push_str(&format!("\n### {}\n", skill_match.skill.name));
        output.push_str(&format!(
            "*Source: {} | Priority: {}*\n\n",
            source_label, skill_match.skill.priority
        ));
        output.push_str(&truncated_body);

        if i < matched_skills.len() - 1 {
            output.push_str("\n\n---\n");
        } else {
            output.push('\n');
        }
    }

    output
}

/// Simplified injection that only uses SkillMatch summaries (without full docs).
/// Uses the body from the skill summary path to reload, or falls back to a description.
pub fn inject_skill_summaries(matched_skills: &[SkillMatch], policy: &SelectionPolicy) -> String {
    if matched_skills.is_empty() {
        return String::new();
    }

    let mut output = String::new();
    output.push_str("\n## Framework-Specific Best Practices\n\n");
    output.push_str("The following guidelines apply based on detected frameworks:\n");

    for (i, skill_match) in matched_skills.iter().enumerate() {
        let source_label = format_source(&skill_match.skill.source);

        // Try to read body from file path
        let body = std::fs::read_to_string(&skill_match.skill.path)
            .unwrap_or_else(|_| skill_match.skill.description.clone());

        let truncated_body = truncate_body(&body, policy.max_content_lines);

        output.push_str(&format!("\n### {}\n", skill_match.skill.name));
        output.push_str(&format!(
            "*Source: {} | Priority: {}*\n\n",
            source_label, skill_match.skill.priority
        ));
        output.push_str(&truncated_body);

        if i < matched_skills.len() - 1 {
            output.push_str("\n\n---\n");
        } else {
            output.push('\n');
        }
    }

    output
}

/// Format a skill source for display.
fn format_source(source: &SkillSource) -> String {
    match source {
        SkillSource::Builtin => "builtin".to_string(),
        SkillSource::External { source_name } => format!("{} (external)", source_name),
        SkillSource::User => "user".to_string(),
        SkillSource::ProjectLocal => "project-local".to_string(),
        SkillSource::Generated => "auto-generated".to_string(),
    }
}

/// Truncate body text to max_lines, appending a truncation notice if needed.
fn truncate_body(body: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = body.lines().collect();
    if lines.len() <= max_lines {
        body.to_string()
    } else {
        let truncated: String = lines[..max_lines].join("\n");
        format!(
            "{}\n\n*... (truncated, {} more lines)*",
            truncated,
            lines.len() - max_lines
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::skills::model::{InjectionPhase, MatchReason, SkillSource, SkillSummary};
    use std::path::PathBuf;

    fn make_match(name: &str, source: SkillSource, priority: u32) -> SkillMatch {
        SkillMatch {
            score: 10.0,
            match_reason: MatchReason::AutoDetected,
            skill: SkillSummary {
                id: format!("{}-test123", name),
                name: name.to_string(),
                description: format!("Description for {}", name),
                version: None,
                tags: vec![],
                source,
                priority,
                enabled: true,
                detected: true,
                user_invocable: false,
                has_hooks: false,
                inject_into: vec![InjectionPhase::Always],
                path: PathBuf::from(format!("/test/{}/SKILL.md", name)),
            },
        }
    }

    #[test]
    fn test_inject_empty() {
        let result = inject_skills(&[], &[], &SelectionPolicy::default());
        assert!(result.is_empty());
    }

    #[test]
    fn test_format_source_builtin() {
        assert_eq!(format_source(&SkillSource::Builtin), "builtin");
    }

    #[test]
    fn test_format_source_external() {
        assert_eq!(
            format_source(&SkillSource::External {
                source_name: "vercel".to_string()
            }),
            "vercel (external)"
        );
    }

    #[test]
    fn test_format_source_project_local() {
        assert_eq!(format_source(&SkillSource::ProjectLocal), "project-local");
    }

    #[test]
    fn test_format_source_generated() {
        assert_eq!(format_source(&SkillSource::Generated), "auto-generated");
    }

    #[test]
    fn test_truncate_body_short() {
        let body = "Line 1\nLine 2\nLine 3";
        let result = truncate_body(body, 10);
        assert_eq!(result, body);
    }

    #[test]
    fn test_truncate_body_long() {
        let lines: Vec<String> = (0..300).map(|i| format!("Line {}", i)).collect();
        let body = lines.join("\n");
        let result = truncate_body(&body, 200);

        assert!(result.contains("Line 0"));
        assert!(result.contains("Line 199"));
        assert!(result.contains("truncated"));
        assert!(result.contains("100 more lines"));
    }

    #[test]
    fn test_inject_single_skill() {
        let matches = vec![make_match(
            "react-best-practices",
            SkillSource::External {
                source_name: "vercel".to_string(),
            },
            100,
        )];

        let doc = crate::services::skills::model::SkillDocument {
            id: "react-best-practices-test123".to_string(),
            name: "react-best-practices".to_string(),
            description: "React best practices".to_string(),
            version: None,
            tags: vec![],
            body: "# React Best Practices\n\nUse functional components.".to_string(),
            path: PathBuf::from("/test/react/SKILL.md"),
            hash: "test".to_string(),
            last_modified: None,
            user_invocable: false,
            allowed_tools: vec![],
            license: None,
            metadata: std::collections::HashMap::new(),
            hooks: None,
            source: SkillSource::External {
                source_name: "vercel".to_string(),
            },
            priority: 100,
            detect: None,
            inject_into: vec![],
            enabled: true,
        };

        let docs: Vec<&crate::services::skills::model::SkillDocument> = vec![&doc];
        let policy = SelectionPolicy::default();
        let result = inject_skills(&matches, &docs, &policy);

        assert!(result.contains("## Framework-Specific Best Practices"));
        assert!(result.contains("### react-best-practices"));
        assert!(result.contains("vercel (external)"));
        assert!(result.contains("Priority: 100"));
        assert!(result.contains("Use functional components"));
    }

    #[test]
    fn test_inject_multiple_skills_with_separator() {
        let matches = vec![
            make_match("skill-a", SkillSource::Builtin, 10),
            make_match("skill-b", SkillSource::ProjectLocal, 201),
        ];

        let doc_a = crate::services::skills::model::SkillDocument {
            id: "skill-a-test123".to_string(),
            name: "skill-a".to_string(),
            description: "Skill A".to_string(),
            version: None,
            tags: vec![],
            body: "Body A".to_string(),
            path: PathBuf::from("/test/a.md"),
            hash: "test".to_string(),
            last_modified: None,
            user_invocable: false,
            allowed_tools: vec![],
            license: None,
            metadata: std::collections::HashMap::new(),
            hooks: None,
            source: SkillSource::Builtin,
            priority: 10,
            detect: None,
            inject_into: vec![],
            enabled: true,
        };

        let doc_b = crate::services::skills::model::SkillDocument {
            id: "skill-b-test123".to_string(),
            name: "skill-b".to_string(),
            description: "Skill B".to_string(),
            version: None,
            tags: vec![],
            body: "Body B".to_string(),
            path: PathBuf::from("/test/b.md"),
            hash: "test".to_string(),
            last_modified: None,
            user_invocable: false,
            allowed_tools: vec![],
            license: None,
            metadata: std::collections::HashMap::new(),
            hooks: None,
            source: SkillSource::ProjectLocal,
            priority: 201,
            detect: None,
            inject_into: vec![],
            enabled: true,
        };

        let docs: Vec<&crate::services::skills::model::SkillDocument> = vec![&doc_a, &doc_b];
        let policy = SelectionPolicy::default();
        let result = inject_skills(&matches, &docs, &policy);

        assert!(result.contains("### skill-a"));
        assert!(result.contains("### skill-b"));
        assert!(result.contains("---")); // separator between skills
        assert!(result.contains("Body A"));
        assert!(result.contains("Body B"));
    }
}
