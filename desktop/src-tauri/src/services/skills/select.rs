//! Skill Selection
//!
//! Two-phase selection combining auto-detection and lexical scoring.
//!
//! Phase 1: Auto-detection (at session start)
//!   For each skill with detect rules, check files/patterns against project root.
//!
//! Phase 2: Lexical matching (per user message)
//!   Score project-local skills against user query using weighted term matching.

use std::collections::HashSet;
use std::path::Path;

use crate::services::skills::model::{
    InjectionPhase, MatchReason, SelectionPolicy, SkillIndex, SkillMatch, SkillSource,
};

/// Select skills for a session using two-phase approach.
///
/// Phase 1: Auto-detected skills (have detect rules that match the project)
/// Phase 2: Lexical scoring against user message
///
/// Auto-detected skills always included (up to top_k).
/// Lexical matches fill remaining slots.
/// Priority breaks ties.
pub fn select_skills_for_session(
    index: &SkillIndex,
    project_root: &Path,
    user_message: &str,
    phase: &InjectionPhase,
    policy: &SelectionPolicy,
) -> Vec<SkillMatch> {
    let mut results = Vec::new();

    // Phase 1: Auto-detected skills
    let auto_detected = detect_applicable_skills(index, project_root, phase);
    for (skill_idx, _score) in &auto_detected {
        let skill = &index.skills()[*skill_idx];
        if !skill.enabled {
            continue;
        }
        if !policy.include_tags.is_empty()
            && !skill.tags.iter().any(|t| policy.include_tags.contains(t))
        {
            continue;
        }
        if skill.tags.iter().any(|t| policy.exclude_tags.contains(t)) {
            continue;
        }

        results.push(SkillMatch {
            score: 100.0, // Auto-detected skills always have high score
            match_reason: MatchReason::AutoDetected,
            skill: skill.to_summary(true),
        });
    }

    // Phase 2: Lexical scoring for remaining slots
    if results.len() < policy.top_k && !user_message.is_empty() {
        let detected_ids: HashSet<String> = results.iter().map(|r| r.skill.id.clone()).collect();

        let mut lexical_matches = lexical_score_skills(index, user_message, phase);

        // Filter out already-detected skills and apply policy
        lexical_matches.retain(|m| {
            !detected_ids.contains(&m.skill.id)
                && m.score >= policy.min_score
                && m.skill.enabled
                && (policy.include_tags.is_empty()
                    || m.skill.tags.iter().any(|t| policy.include_tags.contains(t)))
                && !m.skill.tags.iter().any(|t| policy.exclude_tags.contains(t))
        });

        // Sort by score desc, then priority desc
        lexical_matches.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.skill.priority.cmp(&a.skill.priority))
        });

        let remaining_slots = policy.top_k - results.len();
        results.extend(lexical_matches.into_iter().take(remaining_slots));
    }

    results
}

/// Phase 1: Detect applicable skills by checking detect rules against project files.
///
/// For each skill with detect rules:
/// 1. Check detect.files -- do any exist in project root?
/// 2. Check detect.patterns -- do patterns appear in those files?
///
/// Returns (skill_index, match_score) tuples.
fn detect_applicable_skills(
    index: &SkillIndex,
    project_root: &Path,
    phase: &InjectionPhase,
) -> Vec<(usize, f32)> {
    let mut matches = Vec::new();

    for (idx, skill) in index.skills().iter().enumerate() {
        if !skill.enabled {
            continue;
        }

        // Check injection phase
        if !skill.inject_into.is_empty()
            && !skill.inject_into.contains(phase)
            && !skill.inject_into.contains(&InjectionPhase::Always)
        {
            continue;
        }

        if let Some(detect) = &skill.detect {
            let mut files_matched = Vec::new();
            let mut patterns_matched = Vec::new();

            // Check which detect.files exist
            for file_name in &detect.files {
                let file_path = project_root.join(file_name);
                if file_path.exists() {
                    files_matched.push(file_name.clone());
                }
            }

            if files_matched.is_empty() {
                continue; // No files matched, skip this skill
            }

            // If patterns specified, check them in matched files
            if !detect.patterns.is_empty() {
                for file_name in &files_matched {
                    let file_path = project_root.join(file_name);
                    if let Ok(content) = std::fs::read_to_string(&file_path) {
                        for pattern in &detect.patterns {
                            if content.contains(pattern.as_str()) {
                                patterns_matched.push(pattern.clone());
                            }
                        }
                    }
                }

                if patterns_matched.is_empty() {
                    continue; // Files exist but no patterns matched
                }
            }

            // Matched! Score based on number of matches
            let score = files_matched.len() as f32 + patterns_matched.len() as f32;
            matches.push((idx, score));
        }
    }

    // Sort by score desc
    matches.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    matches
}

/// Phase 2: Lexical scoring of skills against a user message.
///
/// Scoring algorithm (same as adk-rust):
/// - Name match:        +4.0 per token
/// - Description match: +2.5 per token
/// - Tags match:        +2.0 per token
/// - Body match:        +1.0 per token
/// - Normalize by sqrt(body_token_count)
pub fn lexical_score_skills(
    index: &SkillIndex,
    query: &str,
    phase: &InjectionPhase,
) -> Vec<SkillMatch> {
    let query_tokens: HashSet<String> = tokenize(query);
    if query_tokens.is_empty() {
        return vec![];
    }

    let mut matches = Vec::new();

    for skill in index.skills() {
        if !skill.enabled {
            continue;
        }

        // Check injection phase
        if !skill.inject_into.is_empty()
            && !skill.inject_into.contains(phase)
            && !skill.inject_into.contains(&InjectionPhase::Always)
        {
            continue;
        }

        let name_tokens: HashSet<String> = tokenize(&skill.name);
        let desc_tokens: HashSet<String> = tokenize(&skill.description);
        let tag_tokens: HashSet<String> = skill.tags.iter().flat_map(|t| tokenize(t)).collect();
        let body_tokens: HashSet<String> = tokenize(&skill.body);

        let body_token_count = body_tokens.len().max(1);

        let mut score = 0.0_f32;

        // Score each matching token
        for token in &query_tokens {
            if name_tokens.contains(token) {
                score += 4.0;
            }
            if desc_tokens.contains(token) {
                score += 2.5;
            }
            if tag_tokens.contains(token) {
                score += 2.0;
            }
            if body_tokens.contains(token) {
                score += 1.0;
            }
        }

        // Normalize by sqrt(body_token_count) to avoid bias toward long skills
        score /= (body_token_count as f32).sqrt();

        if score > 0.0 {
            matches.push(SkillMatch {
                score,
                match_reason: MatchReason::LexicalMatch {
                    query: query.to_string(),
                },
                skill: skill.to_summary(false),
            });
        }
    }

    // Sort by score desc
    matches.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    matches
}

/// Tokenize text into lowercase words for matching.
fn tokenize(text: &str) -> HashSet<String> {
    text.split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
        .filter(|s| s.len() >= 2)
        .map(|s| s.to_lowercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::skills::model::{SkillDetection, SkillDocument, SkillIndex, SkillSource};
    use std::collections::HashMap;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn make_doc(
        name: &str,
        description: &str,
        source: SkillSource,
        priority: u32,
        detect: Option<SkillDetection>,
        tags: Vec<&str>,
    ) -> SkillDocument {
        SkillDocument {
            id: format!("{}-test123", name),
            name: name.to_string(),
            description: description.to_string(),
            version: None,
            tags: tags.into_iter().map(String::from).collect(),
            body: format!("# {}\n\nSkill body content for {}.", name, name),
            path: PathBuf::from(format!("/skills/{}/SKILL.md", name)),
            hash: "testhash123456".to_string(),
            last_modified: None,
            user_invocable: false,
            allowed_tools: vec![],
            license: None,
            metadata: HashMap::new(),
            hooks: None,
            source,
            priority,
            detect,
            inject_into: vec![InjectionPhase::Always],
            enabled: true,
        }
    }

    #[test]
    fn test_tokenize() {
        let tokens = tokenize("Hello World test-skill");
        assert!(tokens.contains("hello"));
        assert!(tokens.contains("world"));
        assert!(tokens.contains("test-skill"));
    }

    #[test]
    fn test_tokenize_short_words_filtered() {
        let tokens = tokenize("a b cd ef");
        assert!(!tokens.contains("a"));
        assert!(!tokens.contains("b"));
        assert!(tokens.contains("cd"));
        assert!(tokens.contains("ef"));
    }

    #[test]
    fn test_lexical_score_empty_query() {
        let index = SkillIndex::new(vec![make_doc(
            "test",
            "A test skill",
            SkillSource::Builtin,
            10,
            None,
            vec![],
        )]);
        let results = lexical_score_skills(&index, "", &InjectionPhase::Always);
        assert!(results.is_empty());
    }

    #[test]
    fn test_lexical_score_name_match_highest() {
        let index = SkillIndex::new(vec![
            make_doc(
                "react-hooks",
                "Hooks for React components",
                SkillSource::Builtin,
                10,
                None,
                vec!["react"],
            ),
            make_doc(
                "vue-setup",
                "Vue composition API setup",
                SkillSource::Builtin,
                10,
                None,
                vec!["vue"],
            ),
        ]);

        let results = lexical_score_skills(&index, "react hooks", &InjectionPhase::Always);
        assert!(!results.is_empty());
        // react-hooks should score highest because "react" matches name
        assert_eq!(results[0].skill.name, "react-hooks");
    }

    #[test]
    fn test_lexical_score_tag_match() {
        let index = SkillIndex::new(vec![
            make_doc(
                "skill-a",
                "Generic skill",
                SkillSource::Builtin,
                10,
                None,
                vec!["typescript", "testing"],
            ),
            make_doc(
                "skill-b",
                "Another skill",
                SkillSource::Builtin,
                10,
                None,
                vec!["python"],
            ),
        ]);

        let results = lexical_score_skills(&index, "typescript testing", &InjectionPhase::Always);
        assert!(!results.is_empty());
        assert_eq!(results[0].skill.name, "skill-a");
    }

    #[test]
    fn test_lexical_score_disabled_skill_excluded() {
        let mut doc = make_doc("test", "Test skill", SkillSource::Builtin, 10, None, vec![]);
        doc.enabled = false;

        let index = SkillIndex::new(vec![doc]);
        let results = lexical_score_skills(&index, "test skill", &InjectionPhase::Always);
        assert!(results.is_empty());
    }

    #[test]
    fn test_detect_applicable_skills_file_match() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"dependencies": {"react": "18"}}"#,
        )
        .unwrap();

        let doc = make_doc(
            "react-best-practices",
            "React best practices",
            SkillSource::External {
                source_name: "vercel".to_string(),
            },
            100,
            Some(SkillDetection {
                files: vec!["package.json".to_string()],
                patterns: vec!["react".to_string()],
            }),
            vec!["react"],
        );

        let index = SkillIndex::new(vec![doc]);
        let detected =
            detect_applicable_skills(&index, dir.path(), &InjectionPhase::Implementation);
        assert_eq!(detected.len(), 1);
    }

    #[test]
    fn test_detect_applicable_skills_no_file() {
        let dir = TempDir::new().unwrap();
        // No package.json

        let doc = make_doc(
            "react-best-practices",
            "React best practices",
            SkillSource::External {
                source_name: "vercel".to_string(),
            },
            100,
            Some(SkillDetection {
                files: vec!["package.json".to_string()],
                patterns: vec!["react".to_string()],
            }),
            vec!["react"],
        );

        let index = SkillIndex::new(vec![doc]);
        let detected =
            detect_applicable_skills(&index, dir.path(), &InjectionPhase::Implementation);
        assert!(detected.is_empty());
    }

    #[test]
    fn test_detect_applicable_skills_file_exists_pattern_no_match() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"dependencies": {"vue": "3"}}"#,
        )
        .unwrap();

        let doc = make_doc(
            "react-best-practices",
            "React best practices",
            SkillSource::External {
                source_name: "vercel".to_string(),
            },
            100,
            Some(SkillDetection {
                files: vec!["package.json".to_string()],
                patterns: vec!["react".to_string()],
            }),
            vec!["react"],
        );

        let index = SkillIndex::new(vec![doc]);
        let detected =
            detect_applicable_skills(&index, dir.path(), &InjectionPhase::Implementation);
        assert!(detected.is_empty());
    }

    #[test]
    fn test_detect_applicable_skills_files_only_no_patterns() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();

        let doc = make_doc(
            "rust-guidelines",
            "Rust coding guidelines",
            SkillSource::External {
                source_name: "community".to_string(),
            },
            100,
            Some(SkillDetection {
                files: vec!["Cargo.toml".to_string()],
                patterns: vec![], // No patterns = file existence is enough
            }),
            vec!["rust"],
        );

        let index = SkillIndex::new(vec![doc]);
        let detected =
            detect_applicable_skills(&index, dir.path(), &InjectionPhase::Implementation);
        assert_eq!(detected.len(), 1);
    }

    #[test]
    fn test_select_skills_auto_detected_included() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"dependencies": {"react": "18"}}"#,
        )
        .unwrap();

        let doc = make_doc(
            "react",
            "React best practices",
            SkillSource::External {
                source_name: "vercel".to_string(),
            },
            100,
            Some(SkillDetection {
                files: vec!["package.json".to_string()],
                patterns: vec!["react".to_string()],
            }),
            vec![],
        );

        let index = SkillIndex::new(vec![doc]);
        let policy = SelectionPolicy::default();

        let results = select_skills_for_session(
            &index,
            dir.path(),
            "help me build a component",
            &InjectionPhase::Implementation,
            &policy,
        );

        assert_eq!(results.len(), 1);
        assert!(matches!(results[0].match_reason, MatchReason::AutoDetected));
    }

    #[test]
    fn test_select_skills_respects_top_k() {
        let dir = TempDir::new().unwrap();

        let docs: Vec<SkillDocument> = (0..10)
            .map(|i| {
                make_doc(
                    &format!("skill-{}", i),
                    &format!("Test skill number {}", i),
                    SkillSource::ProjectLocal,
                    201,
                    None,
                    vec!["test"],
                )
            })
            .collect();

        let index = SkillIndex::new(docs);
        let policy = SelectionPolicy {
            top_k: 3,
            min_score: 0.0, // Low threshold for test
            ..Default::default()
        };

        let results = select_skills_for_session(
            &index,
            dir.path(),
            "test skill",
            &InjectionPhase::Always,
            &policy,
        );

        assert!(results.len() <= 3);
    }

    #[test]
    fn test_select_skills_policy_include_tags() {
        let dir = TempDir::new().unwrap();

        let index = SkillIndex::new(vec![
            make_doc(
                "react-skill",
                "React skill",
                SkillSource::ProjectLocal,
                201,
                None,
                vec!["react"],
            ),
            make_doc(
                "vue-skill",
                "Vue skill",
                SkillSource::ProjectLocal,
                201,
                None,
                vec!["vue"],
            ),
        ]);

        let policy = SelectionPolicy {
            top_k: 10,
            min_score: 0.0,
            include_tags: vec!["react".to_string()],
            ..Default::default()
        };

        let results = select_skills_for_session(
            &index,
            dir.path(),
            "react vue skill",
            &InjectionPhase::Always,
            &policy,
        );

        assert!(results
            .iter()
            .all(|r| r.skill.tags.contains(&"react".to_string())));
    }

    #[test]
    fn test_select_skills_policy_exclude_tags() {
        let dir = TempDir::new().unwrap();

        let index = SkillIndex::new(vec![
            make_doc(
                "react-skill",
                "React skill",
                SkillSource::ProjectLocal,
                201,
                None,
                vec!["react"],
            ),
            make_doc(
                "vue-skill",
                "Vue skill",
                SkillSource::ProjectLocal,
                201,
                None,
                vec!["vue"],
            ),
        ]);

        let policy = SelectionPolicy {
            top_k: 10,
            min_score: 0.0,
            exclude_tags: vec!["vue".to_string()],
            ..Default::default()
        };

        let results = select_skills_for_session(
            &index,
            dir.path(),
            "react vue skill",
            &InjectionPhase::Always,
            &policy,
        );

        assert!(results
            .iter()
            .all(|r| !r.skill.tags.contains(&"vue".to_string())));
    }
}
