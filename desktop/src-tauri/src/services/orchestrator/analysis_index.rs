//! Repository inventory and chunk planning for deep analysis.
//!
//! The analysis pipeline uses this module to build a deterministic file inventory
//! and split it into stable chunks. Chunk summaries are then merged upstream.

use crate::utils::error::AppResult;
use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
pub struct AnalysisLimits {
    pub max_files_per_chunk: usize,
    pub max_chunks_per_phase: usize,
    pub max_reads_per_chunk: usize,
    pub max_total_read_files: usize,
    pub max_index_file_size_bytes: u64,
    pub target_coverage_ratio: f64,
    pub target_test_coverage_ratio: f64,
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
pub struct AnalysisCoverageReport {
    pub inventory_total_files: usize,
    pub inventory_indexed_files: usize,
    pub sampled_read_files: usize,
    pub test_files_total: usize,
    pub test_files_read: usize,
    pub coverage_ratio: f64,
    pub test_coverage_ratio: f64,
    pub chunk_count: usize,
    pub synthesis_rounds: usize,
}

pub fn build_file_inventory(
    project_root: &Path,
    excluded_roots: &[String],
) -> AppResult<FileInventory> {
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

        items.push(FileInventoryItem {
            path: rel_norm,
            component,
            language,
            extension: ext,
            size_bytes: metadata.len(),
            line_count,
            is_test,
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
    let effective_test_covered = test_read.max(test_observed);

    let coverage_ratio = if inventory.total_files == 0 {
        1.0
    } else {
        covered_files.len() as f64 / inventory.total_files as f64
    };
    let test_coverage_ratio = if inventory.total_test_files == 0 {
        1.0
    } else {
        effective_test_covered as f64 / inventory.total_test_files as f64
    };

    AnalysisCoverageReport {
        inventory_total_files: inventory.total_files,
        inventory_indexed_files: inventory.indexed_files,
        sampled_read_files: sampled_read_files.len(),
        test_files_total: inventory.total_test_files,
        test_files_read: effective_test_covered,
        coverage_ratio,
        test_coverage_ratio,
        chunk_count,
        synthesis_rounds,
    }
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
        Some("ts") | Some("tsx") | Some("js") | Some("jsx") => "typescript",
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
    fn coverage_counts_observed_tests_even_with_sampled_reads_limited() {
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
                },
                FileInventoryItem {
                    path: "tests/test_a.py".to_string(),
                    component: "python-tests".to_string(),
                    language: "python".to_string(),
                    extension: Some("py".to_string()),
                    size_bytes: 10,
                    line_count: 1,
                    is_test: true,
                },
                FileInventoryItem {
                    path: "tests/test_b.py".to_string(),
                    component: "python-tests".to_string(),
                    language: "python".to_string(),
                    extension: Some("py".to_string()),
                    size_bytes: 10,
                    line_count: 1,
                    is_test: true,
                },
                FileInventoryItem {
                    path: "README.md".to_string(),
                    component: "repo-meta".to_string(),
                    language: "markdown".to_string(),
                    extension: Some("md".to_string()),
                    size_bytes: 10,
                    line_count: 1,
                    is_test: false,
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
        assert_eq!(report.test_files_read, 2);
        assert!((report.test_coverage_ratio - 1.0).abs() < 1e-6);
    }
}
