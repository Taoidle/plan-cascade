//! Skill Indexing
//!
//! SkillIndex construction with SHA-256 hashing for change detection.
//! Builds a SkillIndex from discovered skills by parsing each file and
//! generating unique IDs.

use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;

use crate::services::skills::model::{
    DiscoveredSkill, SkillDocument, SkillIndex, SkillIndexStats, SkillSource,
};
use crate::services::skills::parser::parse_skill_file;
use crate::utils::error::AppResult;

/// Build a SkillIndex from discovered skills.
///
/// For each discovered skill:
/// 1. Parse the file content using the universal parser
/// 2. Compute SHA-256 hash of file content
/// 3. Generate unique ID: normalized-name + "-" + first 12 chars of hex hash
/// 4. Merge into index, deduplicating by name (higher priority wins)
pub fn build_index(skills: Vec<DiscoveredSkill>) -> AppResult<SkillIndex> {
    let mut docs: Vec<SkillDocument> = Vec::new();
    let mut seen_names: HashMap<String, usize> = HashMap::new(); // name -> index in docs

    for skill in skills {
        let parsed = match parse_skill_file(&skill.path, &skill.content) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(
                    "Failed to parse skill file {}: {}",
                    skill.path.display(),
                    e
                );
                continue;
            }
        };

        let hash = compute_sha256(&skill.content);
        let id = generate_skill_id(&parsed.name, &hash);

        let last_modified = skill
            .path
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64);

        let doc = SkillDocument {
            id,
            name: parsed.name.clone(),
            description: parsed.description,
            version: parsed.version,
            tags: parsed.tags,
            body: parsed.body,
            path: skill.path,
            hash,
            last_modified,
            user_invocable: parsed.user_invocable,
            allowed_tools: parsed.allowed_tools,
            license: parsed.license,
            metadata: parsed.metadata,
            hooks: parsed.hooks,
            source: skill.source,
            priority: skill.priority,
            detect: skill.detect,
            inject_into: skill.inject_into,
            enabled: skill.enabled,
        };

        // Dedup by name: higher priority wins
        let normalized_name = parsed.name.to_lowercase();
        if let Some(&existing_idx) = seen_names.get(&normalized_name) {
            if doc.priority > docs[existing_idx].priority {
                docs[existing_idx] = doc;
            }
        } else {
            seen_names.insert(normalized_name, docs.len());
            docs.push(doc);
        }
    }

    // Sort by priority descending (highest priority first)
    docs.sort_by(|a, b| b.priority.cmp(&a.priority));

    Ok(SkillIndex::new(docs))
}

/// Compute SHA-256 hash of content, returning the full hex string.
pub fn compute_sha256(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    let result = hasher.finalize();
    hex_encode(&result)
}

/// Generate a skill ID: normalized-name + "-" + first 12 chars of hex hash.
pub fn generate_skill_id(name: &str, hash: &str) -> String {
    let normalized = name
        .to_lowercase()
        .replace(' ', "-")
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .collect::<String>();

    let hash_prefix = if hash.len() >= 12 {
        &hash[..12]
    } else {
        hash
    };

    format!("{}-{}", normalized, hash_prefix)
}

/// Compute statistics for a SkillIndex.
pub fn compute_index_stats(index: &SkillIndex) -> SkillIndexStats {
    let skills = index.skills();

    let mut builtin_count = 0;
    let mut external_count = 0;
    let mut user_count = 0;
    let mut project_local_count = 0;
    let mut generated_count = 0;
    let mut enabled_count = 0;
    let mut detected_count = 0;

    for skill in skills {
        match &skill.source {
            SkillSource::Builtin => builtin_count += 1,
            SkillSource::External { .. } => external_count += 1,
            SkillSource::User => user_count += 1,
            SkillSource::ProjectLocal => project_local_count += 1,
            SkillSource::Generated => generated_count += 1,
        }
        if skill.enabled {
            enabled_count += 1;
        }
        if skill.detect.is_some() {
            detected_count += 1;
        }
    }

    SkillIndexStats {
        total: skills.len(),
        builtin_count,
        external_count,
        user_count,
        project_local_count,
        generated_count,
        enabled_count,
        detected_count,
    }
}

/// Encode bytes as a hexadecimal string.
fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::skills::model::{InjectionPhase, SkillSource};
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn make_discovered(
        name: &str,
        content: &str,
        source: SkillSource,
        priority: u32,
    ) -> DiscoveredSkill {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(format!("{}.md", name));
        std::fs::write(&path, content).unwrap();

        DiscoveredSkill {
            path,
            content: content.to_string(),
            source,
            priority,
            detect: None,
            inject_into: vec![InjectionPhase::Always],
            enabled: true,
        }
    }

    #[test]
    fn test_compute_sha256() {
        let hash = compute_sha256("hello world");
        assert_eq!(hash.len(), 64); // 32 bytes = 64 hex chars
        // Known SHA-256 of "hello world"
        assert_eq!(
            hash,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn test_generate_skill_id() {
        let id = generate_skill_id(
            "React Best Practices",
            "abcdef123456789012345678",
        );
        assert_eq!(id, "react-best-practices-abcdef123456");
    }

    #[test]
    fn test_generate_skill_id_short_hash() {
        let id = generate_skill_id("test", "abc");
        assert_eq!(id, "test-abc");
    }

    #[test]
    fn test_generate_skill_id_special_chars() {
        let id = generate_skill_id("my skill @ v2!", "abcdef123456");
        assert_eq!(id, "my-skill--v2-abcdef123456");
    }

    #[test]
    fn test_build_index_empty() {
        let index = build_index(vec![]).unwrap();
        assert!(index.is_empty());
    }

    #[test]
    fn test_build_index_single_skill() {
        let content = "---\nname: test-skill\ndescription: A test\n---\n# Test";
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("SKILL.md");
        std::fs::write(&path, content).unwrap();

        let skill = DiscoveredSkill {
            path,
            content: content.to_string(),
            source: SkillSource::ProjectLocal,
            priority: 201,
            detect: None,
            inject_into: vec![InjectionPhase::Always],
            enabled: true,
        };

        let index = build_index(vec![skill]).unwrap();
        assert_eq!(index.len(), 1);
        assert_eq!(index.skills()[0].name, "test-skill");
        assert!(!index.skills()[0].hash.is_empty());
        assert!(index.skills()[0].id.starts_with("test-skill-"));
    }

    #[test]
    fn test_build_index_dedup_by_name_higher_priority_wins() {
        let dir = TempDir::new().unwrap();

        let content1 = "---\nname: shared\ndescription: Lower priority version\n---\n# V1";
        let path1 = dir.path().join("v1.md");
        std::fs::write(&path1, content1).unwrap();

        let content2 = "---\nname: shared\ndescription: Higher priority version\n---\n# V2";
        let path2 = dir.path().join("v2.md");
        std::fs::write(&path2, content2).unwrap();

        let skills = vec![
            DiscoveredSkill {
                path: path1,
                content: content1.to_string(),
                source: SkillSource::Builtin,
                priority: 10,
                detect: None,
                inject_into: vec![],
                enabled: true,
            },
            DiscoveredSkill {
                path: path2,
                content: content2.to_string(),
                source: SkillSource::ProjectLocal,
                priority: 201,
                detect: None,
                inject_into: vec![],
                enabled: true,
            },
        ];

        let index = build_index(skills).unwrap();
        assert_eq!(index.len(), 1);
        assert!(index.skills()[0].description.contains("Higher priority"));
        assert_eq!(index.skills()[0].priority, 201);
    }

    #[test]
    fn test_build_index_sorted_by_priority_desc() {
        let dir = TempDir::new().unwrap();

        let content1 = "---\nname: alpha\ndescription: Low\n---\n# A";
        let path1 = dir.path().join("alpha.md");
        std::fs::write(&path1, content1).unwrap();

        let content2 = "---\nname: beta\ndescription: High\n---\n# B";
        let path2 = dir.path().join("beta.md");
        std::fs::write(&path2, content2).unwrap();

        let skills = vec![
            DiscoveredSkill {
                path: path1,
                content: content1.to_string(),
                source: SkillSource::Builtin,
                priority: 10,
                detect: None,
                inject_into: vec![],
                enabled: true,
            },
            DiscoveredSkill {
                path: path2,
                content: content2.to_string(),
                source: SkillSource::ProjectLocal,
                priority: 201,
                detect: None,
                inject_into: vec![],
                enabled: true,
            },
        ];

        let index = build_index(skills).unwrap();
        assert_eq!(index.len(), 2);
        // Higher priority should be first
        assert_eq!(index.skills()[0].name, "beta");
        assert_eq!(index.skills()[1].name, "alpha");
    }

    #[test]
    fn test_build_index_skips_invalid_files() {
        let dir = TempDir::new().unwrap();

        // Valid skill
        let content1 = "---\nname: valid\ndescription: Good\n---\n# Valid";
        let path1 = dir.path().join("valid.md");
        std::fs::write(&path1, content1).unwrap();

        // Invalid skill (missing name)
        let content2 = "---\ndescription: No name\n---\n# Invalid";
        let path2 = dir.path().join("invalid.md");
        std::fs::write(&path2, content2).unwrap();

        let skills = vec![
            DiscoveredSkill {
                path: path1,
                content: content1.to_string(),
                source: SkillSource::ProjectLocal,
                priority: 201,
                detect: None,
                inject_into: vec![],
                enabled: true,
            },
            DiscoveredSkill {
                path: path2,
                content: content2.to_string(),
                source: SkillSource::ProjectLocal,
                priority: 201,
                detect: None,
                inject_into: vec![],
                enabled: true,
            },
        ];

        let index = build_index(skills).unwrap();
        assert_eq!(index.len(), 1);
        assert_eq!(index.skills()[0].name, "valid");
    }

    #[test]
    fn test_compute_index_stats() {
        let dir = TempDir::new().unwrap();

        let skills_data = vec![
            ("a", SkillSource::Builtin, 10, true, true),
            ("b", SkillSource::External { source_name: "v".to_string() }, 80, true, false),
            ("c", SkillSource::ProjectLocal, 201, false, false),
        ];

        let mut docs = Vec::new();
        for (name, source, priority, enabled, has_detect) in skills_data {
            let content = format!("---\nname: {}\ndescription: Test\n---\n# {}", name, name);
            let path = dir.path().join(format!("{}.md", name));
            std::fs::write(&path, &content).unwrap();

            let detect = if has_detect {
                Some(crate::services::skills::model::SkillDetection {
                    files: vec!["package.json".to_string()],
                    patterns: vec![],
                })
            } else {
                None
            };

            docs.push(SkillDocument {
                id: format!("{}-test", name),
                name: name.to_string(),
                description: "Test".to_string(),
                version: None,
                tags: vec![],
                body: format!("# {}", name),
                path,
                hash: "testhash".to_string(),
                last_modified: None,
                user_invocable: false,
                allowed_tools: vec![],
                license: None,
                metadata: HashMap::new(),
                hooks: None,
                source,
                priority,
                detect,
                inject_into: vec![],
                enabled,
            });
        }

        let index = SkillIndex::new(docs);
        let stats = compute_index_stats(&index);

        assert_eq!(stats.total, 3);
        assert_eq!(stats.builtin_count, 1);
        assert_eq!(stats.external_count, 1);
        assert_eq!(stats.project_local_count, 1);
        assert_eq!(stats.user_count, 0);
        assert_eq!(stats.generated_count, 0);
        assert_eq!(stats.enabled_count, 2); // a and b
        assert_eq!(stats.detected_count, 1); // a
    }

    #[test]
    fn test_build_index_convention_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("CLAUDE.md");
        let content = "# Project Guidance\n\nThis is a CLAUDE.md file.";
        std::fs::write(&path, content).unwrap();

        let skill = DiscoveredSkill {
            path,
            content: content.to_string(),
            source: SkillSource::ProjectLocal,
            priority: 201,
            detect: None,
            inject_into: vec![InjectionPhase::Always],
            enabled: true,
        };

        let index = build_index(vec![skill]).unwrap();
        assert_eq!(index.len(), 1);
        assert_eq!(index.skills()[0].name, "CLAUDE");
    }
}
