//! Repository inventory and chunk planning for deep analysis.
//!
//! The analysis pipeline uses this module to build a deterministic file inventory
//! and split it into stable chunks. Chunk summaries are then merged upstream.

use crate::utils::error::AppResult;
use ignore::WalkBuilder;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AnalysisProfile {
    Fast,
    Balanced,
    DeepCoverage,
}

impl Default for AnalysisProfile {
    fn default() -> Self {
        Self::DeepCoverage
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AnalysisLimits {
    pub max_files_per_chunk: usize,
    pub max_chunks_per_phase: usize,
    pub max_reads_per_chunk: usize,
    pub max_total_read_files: usize,
    pub max_index_file_size_bytes: u64,
    pub target_coverage_ratio: f64,
    pub target_test_coverage_ratio: f64,
    pub target_sampled_read_ratio: f64,
    pub max_symbols_per_file: usize,
}

impl Default for AnalysisLimits {
    fn default() -> Self {
        Self {
            max_files_per_chunk: 24,
            // Deep-coverage mode should be able to touch most shards in large repos.
            max_chunks_per_phase: 256,
            max_reads_per_chunk: 2,
            max_total_read_files: 1_200,
            max_index_file_size_bytes: 1_500_000,
            target_coverage_ratio: 0.80,
            target_test_coverage_ratio: 0.50,
            target_sampled_read_ratio: 0.35,
            max_symbols_per_file: 30,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SymbolKind {
    Function,
    Class,
    Struct,
    Enum,
    Interface,
    Type,
    Const,
    Module,
}

impl SymbolKind {
    /// Short display name for use in compact summaries (e.g. dedup messages).
    pub fn short_name(self) -> &'static str {
        match self {
            SymbolKind::Function => "fn",
            SymbolKind::Class => "class",
            SymbolKind::Struct => "struct",
            SymbolKind::Enum => "enum",
            SymbolKind::Interface => "iface",
            SymbolKind::Type => "type",
            SymbolKind::Const => "const",
            SymbolKind::Module => "mod",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SymbolInfo {
    pub name: String,
    pub kind: SymbolKind,
    pub line: usize,
    /// Parent symbol (e.g., class name for a method)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// Function/method signature (e.g., "fn foo(x: i32) -> bool")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    /// Documentation comment extracted from the source
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc_comment: Option<String>,
    /// End line of the symbol definition (0 if unknown)
    #[serde(default)]
    pub end_line: usize,
}

impl SymbolInfo {
    /// Create a basic SymbolInfo with only name, kind, and line.
    /// Extended fields (parent, signature, doc_comment, end_line) default to None/0.
    pub fn basic(name: String, kind: SymbolKind, line: usize) -> Self {
        Self {
            name,
            kind,
            line,
            parent: None,
            signature: None,
            doc_comment: None,
            end_line: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInventoryItem {
    pub path: String,
    pub component: String,
    pub language: String,
    pub extension: Option<String>,
    pub size_bytes: u64,
    pub line_count: usize,
    pub is_test: bool,
    pub symbols: Vec<SymbolInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FileInventory {
    pub total_files: usize,
    pub total_test_files: usize,
    pub indexed_files: usize,
    pub items: Vec<FileInventoryItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryChunk {
    pub chunk_id: String,
    pub component: String,
    pub files: Vec<String>,
    pub total_lines: usize,
    pub test_files: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChunkPlan {
    pub chunk_size: usize,
    pub chunks: Vec<InventoryChunk>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AnalysisCoverageReport {
    pub inventory_total_files: usize,
    pub inventory_indexed_files: usize,
    pub sampled_read_files: usize,
    pub test_files_total: usize,
    pub test_files_read: usize,
    pub coverage_ratio: f64,
    pub test_coverage_ratio: f64,
    pub sampled_read_ratio: f64,
    pub observed_test_coverage_ratio: f64,
    pub chunk_count: usize,
    pub synthesis_rounds: usize,
}

pub fn build_file_inventory(
    project_root: &Path,
    excluded_roots: &[String],
) -> AppResult<FileInventory> {
    build_file_inventory_with_limits(project_root, excluded_roots, &AnalysisLimits::default())
}

pub fn build_file_inventory_with_limits(
    project_root: &Path,
    excluded_roots: &[String],
    limits: &AnalysisLimits,
) -> AppResult<FileInventory> {
    let max_symbol_file_size: u64 = 500_000; // 500KB threshold for symbol extraction
    let mut excluded = HashSet::new();
    for root in excluded_roots {
        excluded.insert(root.trim().replace('\\', "/"));
    }

    let mut builder = WalkBuilder::new(project_root);
    builder
        .hidden(false)
        .follow_links(false)
        .git_ignore(true)
        .git_exclude(true)
        .git_global(true);

    let mut items = Vec::new();
    for entry in builder.build() {
        let Ok(entry) = entry else {
            continue;
        };
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Ok(rel) = path.strip_prefix(project_root) else {
            continue;
        };
        let rel_norm = normalize_rel_path(&rel.to_string_lossy());
        if rel_norm.is_empty() || is_excluded(&rel_norm, &excluded) {
            continue;
        }

        let metadata = match fs::metadata(path) {
            Ok(meta) => meta,
            Err(_) => continue,
        };
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_ascii_lowercase());
        let language = detect_language(ext.as_deref());
        let is_test = is_test_path(&rel_norm);
        let component = detect_component(&rel_norm);
        let line_count = estimate_line_count(path, metadata.len()).unwrap_or(0);

        let symbols = if metadata.len() <= max_symbol_file_size {
            extract_symbols(path, &language, limits.max_symbols_per_file)
        } else {
            Vec::new()
        };

        items.push(FileInventoryItem {
            path: rel_norm,
            component,
            language,
            extension: ext,
            size_bytes: metadata.len(),
            line_count,
            is_test,
            symbols,
        });
    }

    items.sort_by(|a, b| a.path.cmp(&b.path));
    let total_files = items.len();
    let total_test_files = items.iter().filter(|i| i.is_test).count();

    Ok(FileInventory {
        total_files,
        total_test_files,
        indexed_files: total_files,
        items,
    })
}

pub fn build_chunk_plan(inventory: &FileInventory, limits: &AnalysisLimits) -> ChunkPlan {
    let chunk_size = limits.max_files_per_chunk.max(1);
    let mut grouped: BTreeMap<String, Vec<&FileInventoryItem>> = BTreeMap::new();
    for item in &inventory.items {
        grouped
            .entry(item.component.clone())
            .or_default()
            .push(item);
    }

    let mut chunks = Vec::new();
    for (component, mut files) in grouped {
        files.sort_by(|a, b| a.path.cmp(&b.path));
        for (idx, chunk_items) in files.chunks(chunk_size).enumerate() {
            let chunk_id = format!("{}-{:03}", component_slug(&component), idx + 1);
            let mut paths = Vec::with_capacity(chunk_items.len());
            let mut total_lines = 0usize;
            let mut test_files = 0usize;
            for item in chunk_items {
                paths.push(item.path.clone());
                total_lines += item.line_count;
                if item.is_test {
                    test_files += 1;
                }
            }
            chunks.push(InventoryChunk {
                chunk_id,
                component: component.clone(),
                files: paths,
                total_lines,
                test_files,
            });
        }
    }

    ChunkPlan { chunk_size, chunks }
}

pub fn select_chunks_for_phase(
    phase_id: &str,
    chunk_plan: &ChunkPlan,
    limits: &AnalysisLimits,
    profile: &AnalysisProfile,
) -> Vec<InventoryChunk> {
    if chunk_plan.chunks.is_empty() {
        return Vec::new();
    }

    let max_chunks = limits.max_chunks_per_phase.max(1);
    match profile {
        AnalysisProfile::Fast => {
            let mut selected = chunk_plan
                .chunks
                .iter()
                .filter(|chunk| match phase_id {
                    "structure_discovery" => {
                        chunk.component == "repo-meta"
                            || chunk.component == "python-core"
                            || chunk.component == "desktop-rust"
                            || chunk.component == "desktop-web"
                            || chunk.component.ends_with("tests")
                    }
                    "architecture_trace" => {
                        chunk.component == "python-core"
                            || chunk.component == "mcp-server"
                            || chunk.component == "desktop-rust"
                            || chunk.component == "desktop-web"
                            || chunk.component.ends_with("tests")
                    }
                    "consistency_check" => {
                        chunk.component == "repo-meta"
                            || chunk.component.ends_with("tests")
                            || chunk.component == "python-core"
                            || chunk.component == "mcp-server"
                            || chunk.component == "desktop-rust"
                            || chunk.component == "desktop-web"
                    }
                    _ => true,
                })
                .cloned()
                .collect::<Vec<_>>();
            if selected.is_empty() {
                selected = chunk_plan.chunks.clone();
            }
            selected.truncate(max_chunks.min(48));
            selected
        }
        AnalysisProfile::Balanced => {
            let mut ranked = chunk_plan.chunks.clone();
            ranked.sort_by(|a, b| {
                let pa = balanced_phase_priority(phase_id, &a.component);
                let pb = balanced_phase_priority(phase_id, &b.component);
                pa.cmp(&pb).then_with(|| a.chunk_id.cmp(&b.chunk_id))
            });
            ranked.truncate(max_chunks);
            ranked
        }
        AnalysisProfile::DeepCoverage => {
            let slot = phase_slot(phase_id);
            let mut selected = Vec::new();
            let mut seen = HashSet::new();

            for (idx, chunk) in chunk_plan.chunks.iter().enumerate() {
                if idx % 3 == slot {
                    seen.insert(chunk.chunk_id.clone());
                    selected.push(chunk.clone());
                }
            }
            // Keep critical chunks (meta/tests) in every phase when room permits.
            for chunk in &chunk_plan.chunks {
                if selected.len() >= max_chunks {
                    break;
                }
                let critical = chunk.component == "repo-meta" || chunk.test_files > 0;
                if critical && !seen.contains(&chunk.chunk_id) {
                    seen.insert(chunk.chunk_id.clone());
                    selected.push(chunk.clone());
                }
            }
            // Fill remaining room deterministically.
            for chunk in &chunk_plan.chunks {
                if selected.len() >= max_chunks {
                    break;
                }
                if seen.insert(chunk.chunk_id.clone()) {
                    selected.push(chunk.clone());
                }
            }
            selected
        }
    }
}

pub fn compute_coverage_report(
    inventory: &FileInventory,
    observed_paths: &HashSet<String>,
    read_paths: &HashSet<String>,
    chunk_count: usize,
    synthesis_rounds: usize,
) -> AnalysisCoverageReport {
    let inventory_set = inventory
        .items
        .iter()
        .map(|i| i.path.clone())
        .collect::<HashSet<_>>();
    let test_set = inventory
        .items
        .iter()
        .filter(|i| i.is_test)
        .map(|i| i.path.clone())
        .collect::<HashSet<_>>();

    let mut covered_files = HashSet::new();
    for raw in observed_paths {
        let normalized = normalize_rel_path(raw);
        if inventory_set.contains(&normalized) {
            covered_files.insert(normalized.clone());
            continue;
        }
        if !normalized.is_empty() {
            let prefix = format!("{}/", normalized.trim_end_matches('/'));
            for file in &inventory_set {
                if file.starts_with(&prefix) {
                    covered_files.insert(file.clone());
                }
            }
        }
    }

    let mut sampled_read_files = HashSet::new();
    for raw in read_paths {
        let normalized = normalize_rel_path(raw);
        if inventory_set.contains(&normalized) {
            sampled_read_files.insert(normalized);
        }
    }
    let test_read = sampled_read_files
        .iter()
        .filter(|p| test_set.contains(*p))
        .count();
    let test_observed = covered_files
        .iter()
        .filter(|p| test_set.contains(*p))
        .count();

    let coverage_ratio = if inventory.total_files == 0 {
        1.0
    } else {
        covered_files.len() as f64 / inventory.total_files as f64
    };
    let sampled_read_ratio = if inventory.total_files == 0 {
        1.0
    } else {
        sampled_read_files.len() as f64 / inventory.total_files as f64
    };
    let observed_test_coverage_ratio = if inventory.total_test_files == 0 {
        1.0
    } else {
        test_observed as f64 / inventory.total_test_files as f64
    };
    let test_coverage_ratio = if inventory.total_test_files == 0 {
        1.0
    } else {
        test_read as f64 / inventory.total_test_files as f64
    };

    AnalysisCoverageReport {
        inventory_total_files: inventory.total_files,
        inventory_indexed_files: inventory.indexed_files,
        sampled_read_files: sampled_read_files.len(),
        test_files_total: inventory.total_test_files,
        test_files_read: test_read,
        coverage_ratio,
        test_coverage_ratio,
        sampled_read_ratio,
        observed_test_coverage_ratio,
        chunk_count,
        synthesis_rounds,
    }
}

/// Extract top-level symbol definitions from a source file using regex patterns.
///
/// Supports Python, Rust, TypeScript, JavaScript, Go, and Java.
/// Returns up to `max_symbols` results. Symbols are extracted line-by-line, so
/// only top-level definitions that match the language-specific patterns are found.
pub fn extract_symbols(path: &Path, language: &str, max_symbols: usize) -> Vec<SymbolInfo> {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    extract_symbols_from_str(&content, language, max_symbols)
}

/// Inner helper that operates on a string slice (simplifies testing).
///
/// Uses tree-sitter for accurate parsing when the language is supported,
/// falling back to regex-based extraction for unsupported languages.
fn extract_symbols_from_str(content: &str, language: &str, max_symbols: usize) -> Vec<SymbolInfo> {
    // Try tree-sitter first for supported languages
    if super::tree_sitter_parser::is_language_supported(language) {
        let symbols = super::tree_sitter_parser::parse_symbols(content, language, max_symbols);
        if !symbols.is_empty() {
            return symbols;
        }
        // Fall through to regex if tree-sitter returned empty (parse failure)
    }

    // Regex fallback for unsupported languages or tree-sitter parse failures
    let patterns: Vec<(Regex, SymbolKind)> = match language {
        "python" => vec![
            (Regex::new(r"^def\s+([A-Za-z_]\w*)\s*\(").unwrap(), SymbolKind::Function),
            (Regex::new(r"^class\s+([A-Za-z_]\w*)[\s:(]").unwrap(), SymbolKind::Class),
        ],
        "rust" => vec![
            (Regex::new(r"^\s*pub(?:\s*\([^)]*\))?\s+fn\s+([A-Za-z_]\w*)\s*[<(]").unwrap(), SymbolKind::Function),
            (Regex::new(r"^\s*fn\s+([A-Za-z_]\w*)\s*[<(]").unwrap(), SymbolKind::Function),
            (Regex::new(r"^\s*pub(?:\s*\([^)]*\))?\s+struct\s+([A-Za-z_]\w*)").unwrap(), SymbolKind::Struct),
            (Regex::new(r"^\s*struct\s+([A-Za-z_]\w*)").unwrap(), SymbolKind::Struct),
            (Regex::new(r"^\s*pub(?:\s*\([^)]*\))?\s+enum\s+([A-Za-z_]\w*)").unwrap(), SymbolKind::Enum),
            (Regex::new(r"^\s*enum\s+([A-Za-z_]\w*)").unwrap(), SymbolKind::Enum),
            (Regex::new(r"^\s*pub(?:\s*\([^)]*\))?\s+trait\s+([A-Za-z_]\w*)").unwrap(), SymbolKind::Interface),
            (Regex::new(r"^\s*trait\s+([A-Za-z_]\w*)").unwrap(), SymbolKind::Interface),
            (Regex::new(r"^\s*pub(?:\s*\([^)]*\))?\s+type\s+([A-Za-z_]\w*)").unwrap(), SymbolKind::Type),
            (Regex::new(r"^\s*type\s+([A-Za-z_]\w*)").unwrap(), SymbolKind::Type),
            (Regex::new(r"^\s*pub(?:\s*\([^)]*\))?\s+mod\s+([A-Za-z_]\w*)").unwrap(), SymbolKind::Module),
            (Regex::new(r"^\s*mod\s+([A-Za-z_]\w*)").unwrap(), SymbolKind::Module),
        ],
        "typescript" => vec![
            (Regex::new(r"(?:^|[\s;])(?:export\s+)?(?:async\s+)?function\s+([A-Za-z_$]\w*)\s*[<(]").unwrap(), SymbolKind::Function),
            (Regex::new(r"(?:^|[\s;])(?:export\s+)?class\s+([A-Za-z_$]\w*)").unwrap(), SymbolKind::Class),
            (Regex::new(r"(?:^|[\s;])(?:export\s+)?interface\s+([A-Za-z_$]\w*)").unwrap(), SymbolKind::Interface),
            (Regex::new(r"(?:^|[\s;])(?:export\s+)?type\s+([A-Za-z_$]\w*)\s*[<=]").unwrap(), SymbolKind::Type),
            (Regex::new(r"(?:^|[\s;])(?:export\s+)?enum\s+([A-Za-z_$]\w*)").unwrap(), SymbolKind::Enum),
            (Regex::new(r"(?:^|[\s;])(?:export\s+)?const\s+([A-Za-z_$]\w*)\s*[=:]").unwrap(), SymbolKind::Const),
        ],
        "javascript" => vec![
            (Regex::new(r"(?:^|[\s;])(?:export\s+)?(?:async\s+)?function\s+([A-Za-z_$]\w*)\s*[<(]").unwrap(), SymbolKind::Function),
            (Regex::new(r"(?:^|[\s;])(?:export\s+)?class\s+([A-Za-z_$]\w*)").unwrap(), SymbolKind::Class),
        ],
        "go" => vec![
            (Regex::new(r"^func\s+(?:\([^)]*\)\s+)?([A-Za-z_]\w*)\s*\(").unwrap(), SymbolKind::Function),
            (Regex::new(r"^type\s+([A-Za-z_]\w*)\s+struct\b").unwrap(), SymbolKind::Struct),
            (Regex::new(r"^type\s+([A-Za-z_]\w*)\s+interface\b").unwrap(), SymbolKind::Interface),
            (Regex::new(r"^type\s+([A-Za-z_]\w*)\s").unwrap(), SymbolKind::Type),
        ],
        "java" => vec![
            (Regex::new(r"(?:public|private|protected)?\s*(?:static\s+)?(?:final\s+)?class\s+([A-Za-z_]\w*)").unwrap(), SymbolKind::Class),
            (Regex::new(r"(?:public|private|protected)?\s*interface\s+([A-Za-z_]\w*)").unwrap(), SymbolKind::Interface),
            (Regex::new(r"(?:public|private|protected)?\s*enum\s+([A-Za-z_]\w*)").unwrap(), SymbolKind::Enum),
        ],
        _ => return Vec::new(),
    };

    let mut symbols = Vec::new();
    let mut seen_names = HashSet::new();

    for (line_number, line) in content.lines().enumerate() {
        if symbols.len() >= max_symbols {
            break;
        }
        for (re, kind) in &patterns {
            if let Some(caps) = re.captures(line) {
                if let Some(name_match) = caps.get(1) {
                    let name = name_match.as_str().to_string();
                    // Deduplicate: a pub fn will match both pub fn and bare fn patterns
                    let dedup_key = format!("{}:{}", name, line_number);
                    if seen_names.insert(dedup_key) {
                        symbols.push(SymbolInfo::basic(
                            name,
                            *kind,
                            line_number + 1, // 1-based line numbers
                        ));
                    }
                    break; // first match per line wins
                }
            }
        }
    }

    symbols
}

fn phase_slot(phase_id: &str) -> usize {
    match phase_id {
        "structure_discovery" => 0,
        "architecture_trace" => 1,
        "consistency_check" => 2,
        _ => 0,
    }
}

fn balanced_phase_priority(phase_id: &str, component: &str) -> u8 {
    match phase_id {
        "structure_discovery" => match component {
            "repo-meta" => 0,
            "python-core" | "desktop-rust" | "desktop-web" | "mcp-server" => 1,
            "python-tests" | "rust-tests" | "frontend-tests" => 2,
            "other" => 4,
            _ => 3,
        },
        "architecture_trace" => match component {
            "python-core" | "mcp-server" | "desktop-rust" | "desktop-web" => 0,
            "python-tests" | "rust-tests" | "frontend-tests" => 1,
            "repo-meta" => 2,
            "other" => 4,
            _ => 3,
        },
        "consistency_check" => match component {
            "python-tests" | "rust-tests" | "frontend-tests" => 0,
            "repo-meta" => 1,
            "python-core" | "mcp-server" | "desktop-rust" | "desktop-web" => 2,
            "other" => 4,
            _ => 3,
        },
        _ => 0,
    }
}

fn normalize_rel_path(path: &str) -> String {
    let mut normalized = path.replace('\\', "/");
    normalized = normalized.trim().trim_matches('"').to_string();
    if let Some(idx) = normalized.find(":/") {
        // Strip Windows drive prefix.
        normalized = normalized[(idx + 2)..].to_string();
    }
    normalized = normalized
        .trim_start_matches("./")
        .trim_start_matches('/')
        .to_string();
    normalized
}

fn is_excluded(path: &str, excluded_roots: &HashSet<String>) -> bool {
    let first = path.split('/').next().unwrap_or_default();
    excluded_roots.contains(first)
}

fn detect_language(ext: Option<&str>) -> String {
    match ext {
        Some("py") => "python",
        Some("rs") => "rust",
        Some("ts") | Some("tsx") => "typescript",
        Some("js") | Some("jsx") => "javascript",
        Some("go") => "go",
        Some("java") => "java",
        Some("json") | Some("toml") | Some("yaml") | Some("yml") => "config",
        Some("md") => "markdown",
        _ => "other",
    }
    .to_string()
}

fn detect_component(path: &str) -> String {
    if path == "pyproject.toml"
        || path == "README.md"
        || path == "README_zh.md"
        || path == "README_zh-CN.md"
        || path == "Cargo.toml"
        || path == "package.json"
    {
        return "repo-meta".to_string();
    }
    if path.starts_with("src/plan_cascade/") {
        return "python-core".to_string();
    }
    if path.starts_with("mcp_server/") {
        return "mcp-server".to_string();
    }
    if path.starts_with("desktop/src-tauri/src/") {
        return "desktop-rust".to_string();
    }
    if path.starts_with("desktop/src/") {
        return "desktop-web".to_string();
    }
    if path.starts_with("tests/") {
        return "python-tests".to_string();
    }
    if path.starts_with("desktop/src-tauri/tests/") {
        return "rust-tests".to_string();
    }
    if path.starts_with("desktop/src/components/__tests__/") {
        return "frontend-tests".to_string();
    }
    "other".to_string()
}

fn is_test_path(path: &str) -> bool {
    path.starts_with("tests/")
        || path.starts_with("desktop/src-tauri/tests/")
        || path.starts_with("desktop/src/components/__tests__/")
        || path.ends_with("_test.py")
        || path.ends_with(".test.ts")
        || path.ends_with(".test.tsx")
        || path.ends_with(".spec.ts")
        || path.ends_with(".spec.tsx")
}

fn estimate_line_count(path: &Path, file_size: u64) -> Option<usize> {
    // Avoid loading very large files into memory during indexing.
    if file_size > 2_000_000 {
        return Some(0);
    }
    let bytes = fs::read(path).ok()?;
    if bytes.is_empty() {
        return Some(0);
    }
    Some(bytes.iter().filter(|&&b| b == b'\n').count() + 1)
}

fn component_slug(component: &str) -> String {
    component
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn builds_inventory_and_chunk_plan() {
        let dir = tempdir().expect("temp dir");
        fs::create_dir_all(dir.path().join("src/plan_cascade/core")).expect("mkdir");
        fs::create_dir_all(dir.path().join("tests")).expect("mkdir");
        fs::write(
            dir.path().join("src/plan_cascade/core/orchestrator.py"),
            "class Orchestrator:\n    pass\n",
        )
        .expect("write");
        fs::write(
            dir.path().join("tests/test_orchestrator.py"),
            "def test_ok():\n    assert True\n",
        )
        .expect("write");

        let inventory = build_file_inventory(dir.path(), &[]).expect("inventory");
        assert_eq!(inventory.total_files, 2);
        assert_eq!(inventory.total_test_files, 1);

        let limits = AnalysisLimits {
            max_files_per_chunk: 1,
            ..Default::default()
        };
        let plan = build_chunk_plan(&inventory, &limits);
        assert_eq!(plan.chunks.len(), 2);
    }

    #[test]
    fn deep_profile_shards_chunks_across_phases() {
        let dir = tempdir().expect("temp dir");
        fs::create_dir_all(dir.path().join("src/plan_cascade/core")).expect("mkdir");
        fs::create_dir_all(dir.path().join("mcp_server")).expect("mkdir");
        fs::create_dir_all(dir.path().join("desktop/src-tauri/src")).expect("mkdir");
        fs::create_dir_all(dir.path().join("desktop/src")).expect("mkdir");
        fs::create_dir_all(dir.path().join("tests")).expect("mkdir");
        for idx in 0..9 {
            let path = match idx % 5 {
                0 => format!("src/plan_cascade/core/module_{idx}.py"),
                1 => format!("mcp_server/tool_{idx}.py"),
                2 => format!("desktop/src-tauri/src/mod_{idx}.rs"),
                3 => format!("desktop/src/view_{idx}.tsx"),
                _ => format!("tests/test_case_{idx}.py"),
            };
            fs::write(dir.path().join(path), "x\n").expect("write");
        }

        let inventory = build_file_inventory(dir.path(), &[]).expect("inventory");
        let limits = AnalysisLimits {
            max_files_per_chunk: 1,
            max_chunks_per_phase: 10,
            ..Default::default()
        };
        let plan = build_chunk_plan(&inventory, &limits);
        assert!(plan.chunks.len() >= 9);

        let a = select_chunks_for_phase(
            "structure_discovery",
            &plan,
            &limits,
            &AnalysisProfile::DeepCoverage,
        );
        let b = select_chunks_for_phase(
            "architecture_trace",
            &plan,
            &limits,
            &AnalysisProfile::DeepCoverage,
        );
        let c = select_chunks_for_phase(
            "consistency_check",
            &plan,
            &limits,
            &AnalysisProfile::DeepCoverage,
        );
        assert!(!a.is_empty());
        assert!(!b.is_empty());
        assert!(!c.is_empty());

        let mut seen = HashSet::new();
        for item in a.iter().chain(b.iter()).chain(c.iter()) {
            seen.insert(item.chunk_id.clone());
        }
        assert_eq!(seen.len(), plan.chunks.len());
    }

    #[test]
    fn coverage_tracks_sampled_and_observed_test_ratios_separately() {
        let inventory = FileInventory {
            total_files: 4,
            total_test_files: 2,
            indexed_files: 4,
            items: vec![
                FileInventoryItem {
                    path: "src/plan_cascade/core/a.py".to_string(),
                    component: "python-core".to_string(),
                    language: "python".to_string(),
                    extension: Some("py".to_string()),
                    size_bytes: 10,
                    line_count: 1,
                    is_test: false,
                    symbols: vec![],
                },
                FileInventoryItem {
                    path: "tests/test_a.py".to_string(),
                    component: "python-tests".to_string(),
                    language: "python".to_string(),
                    extension: Some("py".to_string()),
                    size_bytes: 10,
                    line_count: 1,
                    is_test: true,
                    symbols: vec![],
                },
                FileInventoryItem {
                    path: "tests/test_b.py".to_string(),
                    component: "python-tests".to_string(),
                    language: "python".to_string(),
                    extension: Some("py".to_string()),
                    size_bytes: 10,
                    line_count: 1,
                    is_test: true,
                    symbols: vec![],
                },
                FileInventoryItem {
                    path: "README.md".to_string(),
                    component: "repo-meta".to_string(),
                    language: "markdown".to_string(),
                    extension: Some("md".to_string()),
                    size_bytes: 10,
                    line_count: 1,
                    is_test: false,
                    symbols: vec![],
                },
            ],
        };
        let observed = HashSet::from([
            "src/plan_cascade/core/a.py".to_string(),
            "tests".to_string(),
            "README.md".to_string(),
        ]);
        let reads = HashSet::from(["src/plan_cascade/core/a.py".to_string()]);
        let report = compute_coverage_report(&inventory, &observed, &reads, 1, 1);

        assert_eq!(report.inventory_total_files, 4);
        assert_eq!(report.test_files_total, 2);
        assert_eq!(report.test_files_read, 0);
        assert!((report.observed_test_coverage_ratio - 1.0).abs() < 1e-6);
        assert!((report.test_coverage_ratio - 0.0).abs() < 1e-6);
    }

    // =====================================================================
    // Symbol extraction tests
    // =====================================================================

    #[test]
    fn extract_python_symbols() {
        let src = r#"
import os

def greet(name):
    return f"Hello {name}"

class MyService:
    def __init__(self):
        pass

    def run(self):
        pass

def helper():
    pass

class AnotherClass(Base):
    pass
"#;
        let symbols = extract_symbols_from_str(src, "python", 30);
        assert_eq!(symbols.len(), 4);

        assert_eq!(symbols[0].name, "greet");
        assert_eq!(symbols[0].kind, SymbolKind::Function);
        assert_eq!(symbols[0].line, 4);

        assert_eq!(symbols[1].name, "MyService");
        assert_eq!(symbols[1].kind, SymbolKind::Class);
        assert_eq!(symbols[1].line, 7);

        // Indented method `__init__` and `run` should NOT match (not top-level)
        assert_eq!(symbols[2].name, "helper");
        assert_eq!(symbols[2].kind, SymbolKind::Function);
        assert_eq!(symbols[2].line, 14);

        assert_eq!(symbols[3].name, "AnotherClass");
        assert_eq!(symbols[3].kind, SymbolKind::Class);
        assert_eq!(symbols[3].line, 17);
    }

    #[test]
    fn extract_rust_symbols() {
        let src = r#"
use std::io;

pub fn main() {
    println!("hello");
}

fn helper_fn(x: i32) -> i32 {
    x + 1
}

pub struct Config {
    name: String,
}

struct InternalState {
    count: usize,
}

pub enum Status {
    Active,
    Inactive,
}

enum InternalEnum {
    A,
    B,
}

pub trait Processor {
    fn process(&self);
}

trait InternalTrait {
    fn run(&self);
}

pub type Result<T> = std::result::Result<T, Error>;

type InternalAlias = Vec<String>;

pub mod utils;

mod internal;
"#;
        let symbols = extract_symbols_from_str(src, "rust", 30);

        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"main"), "should find pub fn main");
        assert!(names.contains(&"helper_fn"), "should find fn helper_fn");
        assert!(names.contains(&"Config"), "should find pub struct Config");
        assert!(names.contains(&"InternalState"), "should find struct InternalState");
        assert!(names.contains(&"Status"), "should find pub enum Status");
        assert!(names.contains(&"InternalEnum"), "should find enum InternalEnum");
        assert!(names.contains(&"Processor"), "should find pub trait Processor");
        assert!(names.contains(&"InternalTrait"), "should find trait InternalTrait");
        assert!(names.contains(&"Result"), "should find pub type Result");
        assert!(names.contains(&"InternalAlias"), "should find type InternalAlias");
        assert!(names.contains(&"utils"), "should find pub mod utils");
        assert!(names.contains(&"internal"), "should find mod internal");

        // Verify kinds
        let main_sym = symbols.iter().find(|s| s.name == "main").unwrap();
        assert_eq!(main_sym.kind, SymbolKind::Function);

        let config_sym = symbols.iter().find(|s| s.name == "Config").unwrap();
        assert_eq!(config_sym.kind, SymbolKind::Struct);

        let status_sym = symbols.iter().find(|s| s.name == "Status").unwrap();
        assert_eq!(status_sym.kind, SymbolKind::Enum);

        let processor_sym = symbols.iter().find(|s| s.name == "Processor").unwrap();
        assert_eq!(processor_sym.kind, SymbolKind::Interface);

        let result_sym = symbols.iter().find(|s| s.name == "Result").unwrap();
        assert_eq!(result_sym.kind, SymbolKind::Type);

        let utils_sym = symbols.iter().find(|s| s.name == "utils").unwrap();
        assert_eq!(utils_sym.kind, SymbolKind::Module);
    }

    #[test]
    fn extract_typescript_symbols() {
        let src = r#"
import { useState } from 'react';

export function createApp(config: AppConfig) {
    return new App(config);
}

export async function fetchData<T>(url: string): Promise<T> {
    return fetch(url).then(r => r.json());
}

export class AppService {
    constructor() {}
}

export interface Config {
    name: string;
    port: number;
}

export type Status = 'active' | 'inactive';

export enum Direction {
    Up,
    Down,
}

export const MAX_RETRIES = 5;

const INTERNAL_TIMEOUT = 1000;

function helperFn() {}
"#;
        let symbols = extract_symbols_from_str(src, "typescript", 30);

        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"createApp"), "should find export function createApp");
        assert!(names.contains(&"fetchData"), "should find export async function fetchData");
        assert!(names.contains(&"AppService"), "should find export class");
        assert!(names.contains(&"Config"), "should find export interface");
        assert!(names.contains(&"Status"), "should find export type");
        assert!(names.contains(&"Direction"), "should find export enum");
        assert!(names.contains(&"MAX_RETRIES"), "should find export const");
        assert!(names.contains(&"INTERNAL_TIMEOUT"), "should find const");
        assert!(names.contains(&"helperFn"), "should find plain function");

        // Verify kinds
        let create_app = symbols.iter().find(|s| s.name == "createApp").unwrap();
        assert_eq!(create_app.kind, SymbolKind::Function);

        let config = symbols.iter().find(|s| s.name == "Config").unwrap();
        assert_eq!(config.kind, SymbolKind::Interface);

        let status = symbols.iter().find(|s| s.name == "Status").unwrap();
        assert_eq!(status.kind, SymbolKind::Type);

        let direction = symbols.iter().find(|s| s.name == "Direction").unwrap();
        assert_eq!(direction.kind, SymbolKind::Enum);

        let max_retries = symbols.iter().find(|s| s.name == "MAX_RETRIES").unwrap();
        assert_eq!(max_retries.kind, SymbolKind::Const);
    }

    #[test]
    fn extract_javascript_symbols() {
        let src = r#"
const util = require('util');

function processData(items) {
    return items.map(i => i.value);
}

export class DataManager {
    constructor() {}
}

async function loadConfig() {
    return {};
}
"#;
        let symbols = extract_symbols_from_str(src, "javascript", 30);

        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"processData"), "should find function processData");
        assert!(names.contains(&"DataManager"), "should find class DataManager");
        assert!(names.contains(&"loadConfig"), "should find async function loadConfig");

        let process = symbols.iter().find(|s| s.name == "processData").unwrap();
        assert_eq!(process.kind, SymbolKind::Function);

        let manager = symbols.iter().find(|s| s.name == "DataManager").unwrap();
        assert_eq!(manager.kind, SymbolKind::Class);
    }

    #[test]
    fn extract_go_symbols() {
        let src = r#"
package main

import "fmt"

func main() {
    fmt.Println("hello")
}

func (s *Server) Start() error {
    return nil
}

type Config struct {
    Name string
    Port int
}

type Handler interface {
    Handle(req Request) Response
}

type StringSlice []string
"#;
        let symbols = extract_symbols_from_str(src, "go", 30);

        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"main"), "should find func main");
        assert!(names.contains(&"Start"), "should find method Start");
        assert!(names.contains(&"Config"), "should find type Config struct");
        assert!(names.contains(&"Handler"), "should find type Handler interface");
        assert!(names.contains(&"StringSlice"), "should find type alias");

        let config = symbols.iter().find(|s| s.name == "Config").unwrap();
        assert_eq!(config.kind, SymbolKind::Struct);

        let handler = symbols.iter().find(|s| s.name == "Handler").unwrap();
        assert_eq!(handler.kind, SymbolKind::Interface);
    }

    #[test]
    fn extract_java_symbols() {
        let src = r#"
package com.example;

import java.util.List;

public class UserService {
    private final UserRepository repo;

    public UserService(UserRepository repo) {
        this.repo = repo;
    }
}

public interface UserRepository {
    User findById(String id);
}

public enum Status {
    ACTIVE,
    INACTIVE;
}

class InternalHelper {
    // package-private
}
"#;
        let symbols = extract_symbols_from_str(src, "java", 30);

        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"UserService"), "should find public class");
        assert!(names.contains(&"UserRepository"), "should find public interface");
        assert!(names.contains(&"Status"), "should find public enum");
        assert!(names.contains(&"InternalHelper"), "should find package-private class");

        let svc = symbols.iter().find(|s| s.name == "UserService").unwrap();
        assert_eq!(svc.kind, SymbolKind::Class);

        let repo = symbols.iter().find(|s| s.name == "UserRepository").unwrap();
        assert_eq!(repo.kind, SymbolKind::Interface);

        let status = symbols.iter().find(|s| s.name == "Status").unwrap();
        assert_eq!(status.kind, SymbolKind::Enum);
    }

    #[test]
    fn extract_symbols_respects_max_limit() {
        let mut src = String::new();
        for i in 0..50 {
            src.push_str(&format!("def func_{}():\n    pass\n\n", i));
        }
        let symbols = extract_symbols_from_str(&src, "python", 5);
        assert_eq!(symbols.len(), 5);
        assert_eq!(symbols[0].name, "func_0");
        assert_eq!(symbols[4].name, "func_4");
    }

    #[test]
    fn extract_symbols_returns_empty_for_unknown_language() {
        let src = "some random content\nwith no structure\n";
        let symbols = extract_symbols_from_str(src, "config", 30);
        assert!(symbols.is_empty());
    }

    #[test]
    fn extract_symbols_returns_empty_for_empty_content() {
        let symbols = extract_symbols_from_str("", "python", 30);
        assert!(symbols.is_empty());
    }

    #[test]
    fn extract_symbols_correct_line_numbers() {
        let src = "# comment\n\ndef first():\n    pass\n\ndef second():\n    pass\n";
        let symbols = extract_symbols_from_str(src, "python", 30);
        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0].name, "first");
        assert_eq!(symbols[0].line, 3); // 1-based
        assert_eq!(symbols[1].name, "second");
        assert_eq!(symbols[1].line, 6);
    }

    #[test]
    fn extract_symbols_rust_no_duplicates_for_pub_fn() {
        // pub fn should match the pub fn pattern but not create a duplicate from the bare fn pattern
        let src = "pub fn my_function(x: i32) -> i32 {\n    x\n}\n";
        let symbols = extract_symbols_from_str(src, "rust", 30);
        let fn_count = symbols.iter().filter(|s| s.name == "my_function").count();
        assert_eq!(fn_count, 1, "pub fn should not produce duplicate entries");
    }

    #[test]
    fn build_inventory_extracts_symbols_for_small_files() {
        let dir = tempdir().expect("temp dir");
        fs::create_dir_all(dir.path().join("src/plan_cascade/core")).expect("mkdir");
        fs::write(
            dir.path().join("src/plan_cascade/core/service.py"),
            "def start():\n    pass\n\nclass Engine:\n    pass\n",
        )
        .expect("write");

        let inventory = build_file_inventory(dir.path(), &[]).expect("inventory");
        assert_eq!(inventory.items.len(), 1);
        let item = &inventory.items[0];
        assert_eq!(item.symbols.len(), 2);
        assert_eq!(item.symbols[0].name, "start");
        assert_eq!(item.symbols[0].kind, SymbolKind::Function);
        assert_eq!(item.symbols[1].name, "Engine");
        assert_eq!(item.symbols[1].kind, SymbolKind::Class);
    }

    #[test]
    fn build_inventory_skips_symbols_for_large_files() {
        let dir = tempdir().expect("temp dir");
        fs::create_dir_all(dir.path().join("src/plan_cascade/core")).expect("mkdir");
        // Create a file larger than 500KB
        let mut content = String::new();
        for i in 0..20_000 {
            content.push_str(&format!("def func_{}():\n    pass\n\n", i));
        }
        assert!(content.len() > 500_000, "test content should exceed 500KB");
        fs::write(
            dir.path().join("src/plan_cascade/core/huge.py"),
            &content,
        )
        .expect("write");

        let inventory = build_file_inventory(dir.path(), &[]).expect("inventory");
        assert_eq!(inventory.items.len(), 1);
        let item = &inventory.items[0];
        assert!(item.symbols.is_empty(), "symbols should be empty for files > 500KB");
    }

    #[test]
    fn symbol_kind_serialization() {
        let info = SymbolInfo::basic("test_fn".to_string(), SymbolKind::Function, 1);
        let json = serde_json::to_string(&info).expect("serialize");
        assert!(json.contains("Function"));
        let deserialized: SymbolInfo = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.kind, SymbolKind::Function);
        assert_eq!(deserialized.name, "test_fn");
    }
}
