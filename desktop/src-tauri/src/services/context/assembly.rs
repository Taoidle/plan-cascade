//! Context assembly helpers.
//!
//! Contains budget compaction logic that is reusable across command entrypoints.

use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionAction {
    pub stage: String,
    pub action: String,
    pub source_id: String,
    pub before_tokens: usize,
    pub after_tokens: usize,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct AssemblyBudget {
    pub input_token_budget: usize,
    pub reserved_output_tokens: usize,
    pub hard_limit: usize,
    pub used_input_tokens: usize,
    pub over_budget: bool,
}

#[derive(Debug, Clone)]
pub struct AssemblyFallbackCompaction {
    pub trigger_reason: String,
    pub strategy: String,
    pub before_tokens: usize,
    pub after_tokens: usize,
    pub compaction_tokens: u32,
    pub net_saving: i64,
    pub quality_score: f32,
    pub quality_basis: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct AssemblySource {
    pub id: String,
    pub token_cost: usize,
    pub included: bool,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct AssemblyBlock {
    pub source_id: String,
    pub title: String,
    pub content: String,
    pub token_cost: usize,
    pub priority: i32,
    pub reason: String,
    pub anchor: bool,
}

#[derive(Debug, Clone)]
pub struct AssemblyCompactionPolicy {
    pub soft_threshold_ratio: f32,
    pub hard_threshold_ratio: f32,
    pub preserve_anchors: bool,
}

#[derive(Debug, Clone)]
pub struct AssemblyCompactionResult {
    pub blocks: Vec<AssemblyBlock>,
    pub sources: Vec<AssemblySource>,
    pub triggered: bool,
    pub trigger_reason: String,
    pub strategy: String,
    pub before_tokens: usize,
    pub after_tokens: usize,
    pub compaction_tokens: u32,
    pub net_saving: i64,
    pub quality_score: f32,
    pub compaction_actions: Vec<CompactionAction>,
    pub quality_basis: serde_json::Value,
}

pub fn infer_injected_source_kinds(
    has_history: bool,
    memory_enabled: bool,
    knowledge_enabled: bool,
    skills_enabled: bool,
) -> Vec<String> {
    let mut kinds = Vec::new();
    if has_history {
        kinds.push("history".to_string());
    }
    if memory_enabled {
        kinds.push("memory".to_string());
    }
    if knowledge_enabled {
        kinds.push("knowledge".to_string());
    }
    if skills_enabled {
        kinds.push("skills".to_string());
    }
    kinds
}

pub fn build_budget(
    input_token_budget: Option<usize>,
    reserved_output_tokens: Option<usize>,
    hard_limit: Option<usize>,
    default_input_token_budget: usize,
    default_reserved_output_tokens: usize,
    used_input_tokens: usize,
) -> AssemblyBudget {
    let input_budget = input_token_budget
        .unwrap_or(default_input_token_budget)
        .max(256);
    let reserved_output = reserved_output_tokens
        .unwrap_or(default_reserved_output_tokens)
        .max(128);
    let hard_limit_resolved = hard_limit
        .unwrap_or(default_input_token_budget + default_reserved_output_tokens)
        .max(384);

    AssemblyBudget {
        input_token_budget: input_budget,
        reserved_output_tokens: reserved_output,
        hard_limit: hard_limit_resolved,
        used_input_tokens,
        over_budget: used_input_tokens > input_budget,
    }
}

pub fn build_fallback_compaction(
    trigger_reason: impl Into<String>,
    strategy: impl Into<String>,
    token_cost: usize,
) -> AssemblyFallbackCompaction {
    AssemblyFallbackCompaction {
        trigger_reason: trigger_reason.into(),
        strategy: strategy.into(),
        before_tokens: token_cost,
        after_tokens: token_cost,
        compaction_tokens: 0,
        net_saving: 0,
        quality_score: 1.0,
        quality_basis: json!({
            "stage": "legacy_fallback",
        }),
    }
}

fn estimate_tokens_rough(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    (text.chars().count() + 3) / 4
}

fn truncate_chars(input: &str, max_chars: usize) -> String {
    if input.chars().count() <= max_chars {
        return input.to_string();
    }
    let mut out = String::new();
    for ch in input.chars().take(max_chars) {
        out.push(ch);
    }
    out
}

fn summarize_block_for_budget(content: &str, target_tokens: usize) -> String {
    let normalized = content.replace('\r', "");
    let lines: Vec<String> = normalized
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .map(|line| line.to_string())
        .collect();

    if lines.is_empty() {
        return String::new();
    }

    if lines.len() <= 4 {
        let target_chars = (target_tokens.max(64) * 4).max(256);
        return truncate_chars(&lines.join("\n"), target_chars);
    }

    let mut selected = Vec::new();
    selected.push(lines[0].clone());
    if lines.len() > 1 {
        selected.push(lines[1].clone());
    }
    let omitted = lines.len().saturating_sub(4);
    if omitted > 0 {
        selected.push(format!(
            "[summary] {} lines omitted to fit context budget",
            omitted
        ));
    }
    if lines.len() > 3 {
        selected.push(lines[lines.len() - 2].clone());
        selected.push(lines[lines.len() - 1].clone());
    }

    let target_chars = (target_tokens.max(64) * 4).max(256);
    truncate_chars(&selected.join("\n"), target_chars)
}

pub fn apply_budget_and_compaction(
    mut blocks: Vec<AssemblyBlock>,
    mut sources: Vec<AssemblySource>,
    input_budget: usize,
    policy: &AssemblyCompactionPolicy,
) -> AssemblyCompactionResult {
    let before_tokens = blocks.iter().map(|b| b.token_cost).sum::<usize>();
    let anchor_tokens_before = blocks
        .iter()
        .filter(|b| b.anchor)
        .map(|b| b.token_cost)
        .sum::<usize>();

    if before_tokens <= input_budget {
        return AssemblyCompactionResult {
            blocks,
            sources,
            triggered: false,
            trigger_reason: "within_budget".to_string(),
            strategy: "none".to_string(),
            before_tokens,
            after_tokens: before_tokens,
            compaction_tokens: 0,
            net_saving: 0,
            quality_score: 1.0,
            compaction_actions: Vec::new(),
            quality_basis: json!({
                "anchor_retention": 1.0,
                "compression_ratio": 1.0,
                "drop_ratio": 0.0,
                "summarize_ratio": 0.0,
                "stage": "none",
            }),
        };
    }

    let hard_threshold = (input_budget as f32 * policy.hard_threshold_ratio) as usize;
    let soft_threshold = (input_budget as f32 * policy.soft_threshold_ratio) as usize;
    let trigger_reason = if before_tokens > hard_threshold {
        "hard_threshold"
    } else if before_tokens > soft_threshold {
        "soft_threshold"
    } else {
        "input_budget"
    };

    let mut actions: Vec<CompactionAction> = Vec::new();
    blocks.sort_by(|a, b| {
        a.priority
            .cmp(&b.priority)
            .then_with(|| b.token_cost.cmp(&a.token_cost))
    });

    let mut current_tokens = before_tokens;
    let mut dropped_tokens = 0usize;
    let mut summarized_tokens_saved = 0usize;

    // Stage 1: prune low-priority blocks first.
    for block in &mut blocks {
        if current_tokens <= input_budget {
            break;
        }
        if policy.preserve_anchors && block.anchor {
            continue;
        }
        if block.priority > 55 {
            continue;
        }

        let before = block.token_cost;
        if before == 0 {
            continue;
        }

        for source in sources.iter_mut() {
            if source.id == block.source_id && source.included {
                source.included = false;
                source.reason = "trimmed_by_budget_stage1".to_string();
                break;
            }
        }
        current_tokens = current_tokens.saturating_sub(before);
        dropped_tokens = dropped_tokens.saturating_add(before);
        actions.push(CompactionAction {
            stage: "stage1_prune".to_string(),
            action: "drop_block".to_string(),
            source_id: block.source_id.clone(),
            before_tokens: before,
            after_tokens: 0,
            reason: "low_priority".to_string(),
        });
        block.reason = "trimmed_by_budget_stage1".to_string();
        block.content.clear();
        block.token_cost = 0;
    }

    // Stage 2: summarize long blocks before dropping more context.
    if current_tokens > input_budget {
        for block in &mut blocks {
            if current_tokens <= input_budget {
                break;
            }
            if policy.preserve_anchors && block.anchor {
                continue;
            }
            if block.token_cost < 280 || block.content.trim().is_empty() {
                continue;
            }

            let before = block.token_cost;
            let target_tokens = ((before as f32) * 0.45_f32).max(96.0) as usize;
            let summarized = summarize_block_for_budget(&block.content, target_tokens);
            let after = estimate_tokens_rough(&summarized);
            if summarized.trim().is_empty() || after >= before {
                continue;
            }

            block.content = summarized;
            block.token_cost = after;
            block.reason = "summarized_by_budget_stage2".to_string();
            summarized_tokens_saved =
                summarized_tokens_saved.saturating_add(before.saturating_sub(after));
            current_tokens = current_tokens.saturating_sub(before).saturating_add(after);
            actions.push(CompactionAction {
                stage: "stage2_summarize".to_string(),
                action: "summarize_block".to_string(),
                source_id: block.source_id.clone(),
                before_tokens: before,
                after_tokens: after,
                reason: "long_block".to_string(),
            });
        }
    }

    // Stage 3: enforce hard budget if still above limit.
    if current_tokens > input_budget {
        for block in &mut blocks {
            if current_tokens <= input_budget {
                break;
            }
            if policy.preserve_anchors && block.anchor {
                continue;
            }
            if block.token_cost == 0 {
                continue;
            }

            let before = block.token_cost;
            for source in sources.iter_mut() {
                if source.id == block.source_id && source.included {
                    source.included = false;
                    source.reason = "trimmed_by_budget_stage3".to_string();
                    break;
                }
            }
            current_tokens = current_tokens.saturating_sub(before);
            dropped_tokens = dropped_tokens.saturating_add(before);
            actions.push(CompactionAction {
                stage: "stage3_enforce".to_string(),
                action: "drop_block".to_string(),
                source_id: block.source_id.clone(),
                before_tokens: before,
                after_tokens: 0,
                reason: "still_over_budget".to_string(),
            });
            block.reason = "trimmed_by_budget_stage3".to_string();
            block.content.clear();
            block.token_cost = 0;
        }
    }

    let retained: Vec<AssemblyBlock> = blocks
        .into_iter()
        .filter(|b| !b.content.is_empty())
        .collect();
    let after_tokens = retained.iter().map(|b| b.token_cost).sum::<usize>();
    let anchor_tokens_after = retained
        .iter()
        .filter(|b| b.anchor)
        .map(|b| b.token_cost)
        .sum::<usize>();

    // Keep source-level included/token view consistent with retained blocks.
    let mut source_token_map: HashMap<String, usize> = HashMap::new();
    for block in &retained {
        *source_token_map.entry(block.source_id.clone()).or_insert(0) += block.token_cost;
    }
    for source in &mut sources {
        if let Some(cost) = source_token_map.get(&source.id) {
            source.included = true;
            source.token_cost = *cost;
            if source.reason.starts_with("trimmed_by_budget") {
                source.reason = "selected".to_string();
            }
        } else {
            source.included = false;
            source.token_cost = 0;
            if source.reason == "selected" {
                source.reason = "trimmed_by_budget".to_string();
            }
        }
    }

    let compression_ratio = if before_tokens == 0 {
        1.0
    } else {
        (after_tokens as f32 / before_tokens as f32).clamp(0.0, 1.0)
    };
    let anchor_retention = if anchor_tokens_before == 0 {
        1.0
    } else {
        (anchor_tokens_after as f32 / anchor_tokens_before as f32).clamp(0.0, 1.0)
    };
    let drop_ratio = if before_tokens == 0 {
        0.0
    } else {
        (dropped_tokens as f32 / before_tokens as f32).clamp(0.0, 1.0)
    };
    let summarize_ratio = if before_tokens == 0 {
        0.0
    } else {
        (summarized_tokens_saved as f32 / before_tokens as f32).clamp(0.0, 1.0)
    };
    let quality_score =
        (0.45 * anchor_retention + 0.30 * (1.0 - drop_ratio) + 0.25 * summarize_ratio)
            .clamp(0.0, 1.0);
    let net_saving = before_tokens as i64 - after_tokens as i64;
    let strategy = if actions.is_empty() {
        "none".to_string()
    } else if actions
        .iter()
        .any(|action| action.stage == "stage2_summarize")
    {
        "trim_then_semantic_summary".to_string()
    } else {
        "priority_trim".to_string()
    };

    AssemblyCompactionResult {
        blocks: retained,
        sources,
        triggered: true,
        trigger_reason: trigger_reason.to_string(),
        strategy,
        before_tokens,
        after_tokens,
        compaction_tokens: net_saving.max(0) as u32,
        net_saving,
        quality_score,
        compaction_actions: actions,
        quality_basis: json!({
            "anchor_retention": anchor_retention,
            "compression_ratio": compression_ratio,
            "drop_ratio": drop_ratio,
            "summarize_ratio": summarize_ratio,
            "input_budget": input_budget,
            "before_tokens": before_tokens,
            "after_tokens": after_tokens,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_source(id: &str, token_cost: usize) -> AssemblySource {
        AssemblySource {
            id: id.to_string(),
            token_cost,
            included: true,
            reason: "selected".to_string(),
        }
    }

    fn make_block(
        source_id: &str,
        token_cost: usize,
        priority: i32,
        anchor: bool,
        content_seed: &str,
    ) -> AssemblyBlock {
        AssemblyBlock {
            source_id: source_id.to_string(),
            title: source_id.to_string(),
            content: content_seed.repeat(token_cost.max(1)),
            token_cost,
            priority,
            reason: "selected".to_string(),
            anchor,
        }
    }

    #[test]
    fn apply_budget_skips_compaction_when_within_budget() {
        let blocks = vec![
            make_block("a", 60, 80, false, "alpha "),
            make_block("b", 40, 70, false, "beta "),
        ];
        let sources = vec![make_source("a", 60), make_source("b", 40)];

        let result = apply_budget_and_compaction(
            blocks.clone(),
            sources.clone(),
            200,
            &AssemblyCompactionPolicy {
                soft_threshold_ratio: 0.85,
                hard_threshold_ratio: 0.95,
                preserve_anchors: true,
            },
        );

        assert!(!result.triggered);
        assert!(result.compaction_actions.is_empty());
        assert_eq!(result.before_tokens, 100);
        assert_eq!(result.after_tokens, 100);
        assert_eq!(result.blocks.len(), blocks.len());
        assert_eq!(result.sources.len(), sources.len());
    }

    #[test]
    fn apply_budget_preserves_anchor_block() {
        let blocks = vec![
            make_block("anchor", 120, 100, true, "anchored "),
            make_block("trim", 80, 5, false, "trimmed "),
        ];
        let sources = vec![make_source("anchor", 120), make_source("trim", 80)];

        let result = apply_budget_and_compaction(
            blocks,
            sources,
            120,
            &AssemblyCompactionPolicy {
                soft_threshold_ratio: 0.85,
                hard_threshold_ratio: 0.95,
                preserve_anchors: true,
            },
        );

        assert!(result.triggered);
        assert_eq!(result.blocks.len(), 1);
        assert_eq!(result.blocks[0].source_id, "anchor");
        assert!(result
            .sources
            .iter()
            .any(|s| s.id == "trim" && !s.included && s.reason.starts_with("trimmed_by_budget")));
    }

    #[test]
    fn infer_injected_source_kinds_returns_enabled_kinds() {
        let kinds = infer_injected_source_kinds(true, true, false, true);
        assert_eq!(kinds, vec!["history", "memory", "skills"]);
    }

    #[test]
    fn build_budget_respects_defaults_and_bounds() {
        let budget = build_budget(None, None, None, 24_000, 3_000, 1_200);
        assert_eq!(budget.input_token_budget, 24_000);
        assert_eq!(budget.reserved_output_tokens, 3_000);
        assert_eq!(budget.hard_limit, 27_000);
        assert!(!budget.over_budget);
    }
}
