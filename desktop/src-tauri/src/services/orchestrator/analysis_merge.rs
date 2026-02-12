//! Chunk summary merge helpers for analysis synthesis.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkSummaryRecord {
    pub phase_id: String,
    pub chunk_id: String,
    pub component: String,
    pub summary: String,
    pub observed_paths: Vec<String>,
    pub read_files: Vec<String>,
}

pub fn merge_chunk_summaries(
    phase_id: &str,
    phase_title: &str,
    records: &[ChunkSummaryRecord],
    max_chars: usize,
) -> String {
    if records.is_empty() {
        return format!(
            "### {} ({})\n- No chunk summaries produced.",
            phase_title, phase_id
        );
    }

    let mut lines = Vec::new();
    lines.push(format!("### {} ({})", phase_title, phase_id));
    lines.push(format!("- Chunk summaries merged: {}", records.len()));

    let mut by_component: BTreeMap<&str, usize> = BTreeMap::new();
    let mut read_files_by_component: BTreeMap<&str, usize> = BTreeMap::new();
    let mut unique_read_files = HashSet::<String>::new();

    for record in records {
        *by_component.entry(&record.component).or_insert(0) += 1;
        *read_files_by_component
            .entry(&record.component)
            .or_insert(0) += record.read_files.len();
        for file in &record.read_files {
            unique_read_files.insert(file.clone());
        }
    }

    let component_digest = by_component
        .iter()
        .map(|(component, count)| {
            let sampled = read_files_by_component.get(component).copied().unwrap_or(0);
            format!(
                "{} (chunks={}, sampled_reads={})",
                component, count, sampled
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    lines.push(format!("- Components: {}", component_digest));
    lines.push(format!(
        "- Unique sampled read files across chunks: {}",
        unique_read_files.len()
    ));

    lines.push("- Sample chunk findings:".to_string());
    for record in records.iter().take(8) {
        lines.push(format!(
            "  - {} [{}]: {}",
            record.chunk_id,
            record.component,
            truncate(&record.summary, 220)
        ));
    }

    if !unique_read_files.is_empty() {
        let mut sample_reads = unique_read_files.into_iter().collect::<Vec<_>>();
        sample_reads.sort();
        lines.push(format!(
            "- Sample read files: {}",
            sample_reads
                .into_iter()
                .take(10)
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    let merged = lines.join("\n");
    truncate(&merged, max_chars.max(300))
}

fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut out = String::new();
    for (idx, ch) in text.chars().enumerate() {
        if idx >= max_chars {
            break;
        }
        out.push(ch);
    }
    out.push_str("...");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_records() {
        let records = vec![
            ChunkSummaryRecord {
                phase_id: "architecture_trace".to_string(),
                chunk_id: "python-core-001".to_string(),
                component: "python-core".to_string(),
                summary: "Mapped orchestrator and backend edges.".to_string(),
                observed_paths: vec!["src/plan_cascade/core/orchestrator.py".to_string()],
                read_files: vec!["src/plan_cascade/core/orchestrator.py".to_string()],
            },
            ChunkSummaryRecord {
                phase_id: "architecture_trace".to_string(),
                chunk_id: "desktop-rust-001".to_string(),
                component: "desktop-rust".to_string(),
                summary: "Traced Tauri command surface.".to_string(),
                observed_paths: vec!["desktop/src-tauri/src/main.rs".to_string()],
                read_files: vec!["desktop/src-tauri/src/main.rs".to_string()],
            },
        ];
        let merged =
            merge_chunk_summaries("architecture_trace", "Architecture Trace", &records, 800);
        assert!(merged.contains("Chunk summaries merged: 2"));
        assert!(merged.contains("python-core-001"));
        assert!(merged.contains("Components:"));
        assert!(!merged.contains("  - Read files:"));
    }
}
