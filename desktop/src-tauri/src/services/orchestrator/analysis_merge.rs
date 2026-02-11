//! Chunk summary merge helpers for analysis synthesis.

use serde::{Deserialize, Serialize};

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
    for record in records {
        lines.push(format!(
            "- {} [{}]: {}",
            record.chunk_id,
            record.component,
            truncate(&record.summary, 420)
        ));
        if !record.read_files.is_empty() {
            lines.push(format!(
                "  - Read files: {}",
                record
                    .read_files
                    .iter()
                    .take(6)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
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
    }
}
