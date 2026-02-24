//! Conflict Resolution Module
//!
//! Parses git conflict markers from file content and provides
//! resolution strategies (ours, theirs, both).

use std::path::Path;

use crate::utils::error::{AppError, AppResult};

use super::types::{ConflictFile, ConflictRegion, ConflictStrategy};

// Conflict marker constants
const MARKER_OURS_START: &str = "<<<<<<<";
const MARKER_ANCESTOR_START: &str = "|||||||";
const MARKER_SEPARATOR: &str = "=======";
const MARKER_THEIRS_END: &str = ">>>>>>>";

/// Parse conflict regions from file content.
///
/// Supports both 2-way conflict markers:
/// ```text
/// <<<<<<< HEAD
/// our content
/// =======
/// their content
/// >>>>>>> branch
/// ```
///
/// And diff3-style 3-way markers:
/// ```text
/// <<<<<<< HEAD
/// our content
/// ||||||| base
/// ancestor content
/// =======
/// their content
/// >>>>>>> branch
/// ```
pub fn parse_conflicts(content: &str) -> Vec<ConflictRegion> {
    let mut regions = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;
    let mut current_line = 1u32; // 1-based line tracking

    while i < lines.len() {
        if lines[i].starts_with(MARKER_OURS_START) {
            let start_line = current_line;
            i += 1;
            current_line += 1;

            // Collect ours content
            let mut ours = String::new();
            let mut ancestor: Option<String> = None;
            let mut theirs = String::new();

            // Phase 1: collect ours (until ======= or |||||||)
            while i < lines.len()
                && !lines[i].starts_with(MARKER_SEPARATOR)
                && !lines[i].starts_with(MARKER_ANCESTOR_START)
            {
                if !ours.is_empty() {
                    ours.push('\n');
                }
                ours.push_str(lines[i]);
                i += 1;
                current_line += 1;
            }

            // Phase 2: check for ancestor (diff3)
            if i < lines.len() && lines[i].starts_with(MARKER_ANCESTOR_START) {
                i += 1;
                current_line += 1;
                let mut anc = String::new();
                while i < lines.len() && !lines[i].starts_with(MARKER_SEPARATOR) {
                    if !anc.is_empty() {
                        anc.push('\n');
                    }
                    anc.push_str(lines[i]);
                    i += 1;
                    current_line += 1;
                }
                ancestor = Some(anc);
            }

            // Skip separator
            if i < lines.len() && lines[i].starts_with(MARKER_SEPARATOR) {
                i += 1;
                current_line += 1;
            }

            // Phase 3: collect theirs (until >>>>>>>)
            while i < lines.len() && !lines[i].starts_with(MARKER_THEIRS_END) {
                if !theirs.is_empty() {
                    theirs.push('\n');
                }
                theirs.push_str(lines[i]);
                i += 1;
                current_line += 1;
            }

            let end_line = current_line;

            // Skip the end marker
            if i < lines.len() && lines[i].starts_with(MARKER_THEIRS_END) {
                i += 1;
                current_line += 1;
            }

            regions.push(ConflictRegion {
                ours,
                theirs,
                ancestor,
                start_line,
                end_line,
            });
        } else {
            i += 1;
            current_line += 1;
        }
    }

    regions
}

/// Resolve a single conflict region with the given strategy.
pub fn resolve_conflict(region: &ConflictRegion, strategy: ConflictStrategy) -> String {
    match strategy {
        ConflictStrategy::Ours => region.ours.clone(),
        ConflictStrategy::Theirs => region.theirs.clone(),
        ConflictStrategy::Both => {
            if region.ours.is_empty() {
                region.theirs.clone()
            } else if region.theirs.is_empty() {
                region.ours.clone()
            } else {
                format!("{}\n{}", region.ours, region.theirs)
            }
        }
    }
}

/// Resolve all conflicts in file content using a single strategy.
///
/// Returns the resolved file content.
pub fn resolve_file(content: &str, strategy: ConflictStrategy) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let mut result = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        if lines[i].starts_with(MARKER_OURS_START) {
            // Parse ours
            i += 1;
            let mut ours_lines = Vec::new();
            let mut ancestor_lines = Vec::new();
            let mut theirs_lines = Vec::new();
            let mut in_ancestor = false;

            // Collect ours (or detect ancestor)
            while i < lines.len()
                && !lines[i].starts_with(MARKER_SEPARATOR)
                && !lines[i].starts_with(MARKER_ANCESTOR_START)
            {
                ours_lines.push(lines[i]);
                i += 1;
            }

            // Check for diff3 ancestor
            if i < lines.len() && lines[i].starts_with(MARKER_ANCESTOR_START) {
                in_ancestor = true;
                i += 1;
                while i < lines.len() && !lines[i].starts_with(MARKER_SEPARATOR) {
                    ancestor_lines.push(lines[i]);
                    i += 1;
                }
            }
            let _ = (in_ancestor, ancestor_lines); // ancestor captured but not used in basic strategies

            // Skip separator
            if i < lines.len() && lines[i].starts_with(MARKER_SEPARATOR) {
                i += 1;
            }

            // Collect theirs
            while i < lines.len() && !lines[i].starts_with(MARKER_THEIRS_END) {
                theirs_lines.push(lines[i]);
                i += 1;
            }

            // Skip end marker
            if i < lines.len() && lines[i].starts_with(MARKER_THEIRS_END) {
                i += 1;
            }

            // Apply strategy
            match strategy {
                ConflictStrategy::Ours => {
                    result.extend(ours_lines);
                }
                ConflictStrategy::Theirs => {
                    result.extend(theirs_lines);
                }
                ConflictStrategy::Both => {
                    result.extend(ours_lines);
                    result.extend(theirs_lines);
                }
            }
        } else {
            result.push(lines[i]);
            i += 1;
        }
    }

    result.join("\n")
}

/// List files with conflict markers in a repository.
pub fn get_conflict_files(
    repo_path: &Path,
    conflicted_paths: &[String],
) -> AppResult<Vec<ConflictFile>> {
    let mut files = Vec::new();

    for path in conflicted_paths {
        let full_path = repo_path.join(path);
        if full_path.exists() {
            let content = std::fs::read_to_string(&full_path).map_err(|e| {
                AppError::command(format!("Failed to read conflict file {}: {}", path, e))
            })?;
            let regions = parse_conflicts(&content);
            if !regions.is_empty() {
                files.push(ConflictFile {
                    path: path.clone(),
                    conflict_count: regions.len() as u32,
                });
            }
        }
    }

    Ok(files)
}

/// Write resolved content to a file.
pub fn write_resolved(repo_path: &Path, file_path: &str, content: &str) -> AppResult<()> {
    let full_path = repo_path.join(file_path);
    std::fs::write(&full_path, content).map_err(|e| {
        AppError::command(format!(
            "Failed to write resolved file {}: {}",
            file_path, e
        ))
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_2way_conflict() {
        let content = "\
before conflict
<<<<<<< HEAD
our line 1
our line 2
=======
their line 1
>>>>>>> feature
after conflict";

        let regions = parse_conflicts(content);
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].ours, "our line 1\nour line 2");
        assert_eq!(regions[0].theirs, "their line 1");
        assert!(regions[0].ancestor.is_none());
    }

    #[test]
    fn test_parse_diff3_conflict() {
        let content = "\
<<<<<<< HEAD
our version
||||||| base
ancestor version
=======
their version
>>>>>>> feature";

        let regions = parse_conflicts(content);
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].ours, "our version");
        assert_eq!(regions[0].theirs, "their version");
        assert_eq!(regions[0].ancestor, Some("ancestor version".to_string()));
    }

    #[test]
    fn test_parse_multiple_conflicts() {
        let content = "\
line 1
<<<<<<< HEAD
ours A
=======
theirs A
>>>>>>> branch
line between
<<<<<<< HEAD
ours B
=======
theirs B
>>>>>>> branch
last line";

        let regions = parse_conflicts(content);
        assert_eq!(regions.len(), 2);
        assert_eq!(regions[0].ours, "ours A");
        assert_eq!(regions[0].theirs, "theirs A");
        assert_eq!(regions[1].ours, "ours B");
        assert_eq!(regions[1].theirs, "theirs B");
    }

    #[test]
    fn test_parse_empty_sides() {
        let content = "\
<<<<<<< HEAD
=======
their content
>>>>>>> branch";

        let regions = parse_conflicts(content);
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].ours, "");
        assert_eq!(regions[0].theirs, "their content");
    }

    #[test]
    fn test_parse_no_conflicts() {
        let content = "just a normal file\nwith no conflicts\n";
        let regions = parse_conflicts(content);
        assert!(regions.is_empty());
    }

    #[test]
    fn test_resolve_conflict_ours() {
        let region = ConflictRegion {
            ours: "our code".to_string(),
            theirs: "their code".to_string(),
            ancestor: None,
            start_line: 1,
            end_line: 5,
        };
        assert_eq!(
            resolve_conflict(&region, ConflictStrategy::Ours),
            "our code"
        );
    }

    #[test]
    fn test_resolve_conflict_theirs() {
        let region = ConflictRegion {
            ours: "our code".to_string(),
            theirs: "their code".to_string(),
            ancestor: None,
            start_line: 1,
            end_line: 5,
        };
        assert_eq!(
            resolve_conflict(&region, ConflictStrategy::Theirs),
            "their code"
        );
    }

    #[test]
    fn test_resolve_conflict_both() {
        let region = ConflictRegion {
            ours: "our code".to_string(),
            theirs: "their code".to_string(),
            ancestor: None,
            start_line: 1,
            end_line: 5,
        };
        assert_eq!(
            resolve_conflict(&region, ConflictStrategy::Both),
            "our code\ntheir code"
        );
    }

    #[test]
    fn test_resolve_conflict_both_empty_ours() {
        let region = ConflictRegion {
            ours: "".to_string(),
            theirs: "their code".to_string(),
            ancestor: None,
            start_line: 1,
            end_line: 5,
        };
        assert_eq!(
            resolve_conflict(&region, ConflictStrategy::Both),
            "their code"
        );
    }

    #[test]
    fn test_resolve_file_ours() {
        let content = "\
before
<<<<<<< HEAD
our line
=======
their line
>>>>>>> branch
after";

        let resolved = resolve_file(content, ConflictStrategy::Ours);
        assert_eq!(resolved, "before\nour line\nafter");
    }

    #[test]
    fn test_resolve_file_theirs() {
        let content = "\
before
<<<<<<< HEAD
our line
=======
their line
>>>>>>> branch
after";

        let resolved = resolve_file(content, ConflictStrategy::Theirs);
        assert_eq!(resolved, "before\ntheir line\nafter");
    }

    #[test]
    fn test_resolve_file_both() {
        let content = "\
before
<<<<<<< HEAD
our line
=======
their line
>>>>>>> branch
after";

        let resolved = resolve_file(content, ConflictStrategy::Both);
        assert_eq!(resolved, "before\nour line\ntheir line\nafter");
    }

    #[test]
    fn test_resolve_file_multiple_conflicts() {
        let content = "\
start
<<<<<<< HEAD
A ours
=======
A theirs
>>>>>>> branch
middle
<<<<<<< HEAD
B ours
=======
B theirs
>>>>>>> branch
end";

        let resolved = resolve_file(content, ConflictStrategy::Ours);
        assert_eq!(resolved, "start\nA ours\nmiddle\nB ours\nend");
    }

    #[test]
    fn test_resolve_file_with_diff3() {
        let content = "\
before
<<<<<<< HEAD
our version
||||||| base
base version
=======
their version
>>>>>>> branch
after";

        let resolved = resolve_file(content, ConflictStrategy::Theirs);
        assert_eq!(resolved, "before\ntheir version\nafter");
    }

    #[test]
    fn test_resolve_file_no_conflicts() {
        let content = "normal file content\nline 2\nline 3";
        let resolved = resolve_file(content, ConflictStrategy::Ours);
        assert_eq!(resolved, content);
    }

    #[test]
    fn test_parse_conflict_line_numbers() {
        let content = "\
line 1
line 2
<<<<<<< HEAD
ours
=======
theirs
>>>>>>> branch
line 8";

        let regions = parse_conflicts(content);
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].start_line, 3); // <<<<<<< is on line 3
    }

    #[test]
    fn test_parse_adjacent_conflicts() {
        let content = "\
<<<<<<< HEAD
ours1
=======
theirs1
>>>>>>> branch
<<<<<<< HEAD
ours2
=======
theirs2
>>>>>>> branch";

        let regions = parse_conflicts(content);
        assert_eq!(regions.len(), 2);
        assert_eq!(regions[0].ours, "ours1");
        assert_eq!(regions[1].ours, "ours2");
    }

    #[test]
    fn test_parse_multiline_conflict_content() {
        let content = "\
<<<<<<< HEAD
line 1 ours
line 2 ours
line 3 ours
=======
line 1 theirs
line 2 theirs
>>>>>>> branch";

        let regions = parse_conflicts(content);
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].ours, "line 1 ours\nline 2 ours\nline 3 ours");
        assert_eq!(regions[0].theirs, "line 1 theirs\nline 2 theirs");
    }

    #[test]
    fn test_write_resolved_and_get_conflict_files() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path();

        // Write a file with conflicts
        let conflict_content = "\
<<<<<<< HEAD
ours
=======
theirs
>>>>>>> branch";
        let file_path = "conflict.txt";
        std::fs::write(repo.join(file_path), conflict_content).unwrap();

        // Get conflict files
        let files = get_conflict_files(repo, &[file_path.to_string()]).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "conflict.txt");
        assert_eq!(files[0].conflict_count, 1);

        // Resolve and write
        let resolved = resolve_file(conflict_content, ConflictStrategy::Ours);
        write_resolved(repo, file_path, &resolved).unwrap();

        // Verify the file no longer has conflicts
        let new_content = std::fs::read_to_string(repo.join(file_path)).unwrap();
        assert_eq!(new_content, "ours");
        let regions = parse_conflicts(&new_content);
        assert!(regions.is_empty());
    }

    #[test]
    fn test_get_conflict_files_nonexistent() {
        let tmp = tempfile::tempdir().unwrap();
        let files = get_conflict_files(tmp.path(), &["nonexistent.txt".to_string()]).unwrap();
        assert!(files.is_empty());
    }
}
