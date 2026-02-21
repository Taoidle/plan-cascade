//! Shared scan/traversal utilities for Glob and Grep tools.

use std::path::Path;

/// Default directory/file patterns to exclude from recursive scans.
pub(crate) const DEFAULT_SCAN_EXCLUDES: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    "coverage",
    ".venv",
    ".pytest_cache",
    ".mypy_cache",
    ".ruff_cache",
    ".plan-cascade",
    "builtin-skills",
    "external-skills",
    "claude-code",
    "codex",
];

/// Check if a candidate path's first directory component matches any default exclusion pattern.
pub(crate) fn is_default_scan_excluded(base: &Path, candidate: &Path) -> bool {
    if let Ok(relative) = candidate.strip_prefix(base) {
        if let Some(first) = relative.components().next() {
            let root = first.as_os_str().to_string_lossy();
            return DEFAULT_SCAN_EXCLUDES.contains(&root.as_ref());
        }
    }
    false
}
