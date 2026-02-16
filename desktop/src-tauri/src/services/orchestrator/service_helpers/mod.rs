use super::*;

// Submodule declarations
mod session_state;
mod constructors;
mod agentic_loop;
mod analysis_pipeline;
mod tool_call_parsing;
mod analysis_prompts;
mod path_utils;

// Import all items from submodules into this module's namespace.
// Sibling submodules access these via `use super::*;`.
use session_state::*;
#[allow(unused_imports)]
use constructors::*;
#[allow(unused_imports)]
use agentic_loop::*;
use analysis_pipeline::*;
use tool_call_parsing::*;
use analysis_prompts::*;
use path_utils::*;

// The only pub(crate) export from this module
pub(crate) use tool_call_parsing::text_describes_pending_action;

// ── Shared constants (used by 3+ submodules) ──────────────────────────

/// Limit evidence verbosity to keep synthesis prompt focused and token-efficient.
const MAX_ANALYSIS_EVIDENCE_LINES: usize = 90;
/// Keep each phase summary short before feeding into synthesis.
const MAX_ANALYSIS_PHASE_SUMMARY_CHARS: usize = 1600;
/// Keep tool outputs bounded when they are fed back into the model during analysis.
const ANALYSIS_TOOL_RESULT_MAX_CHARS: usize = 1200;
const ANALYSIS_TOOL_RESULT_MAX_LINES: usize = 40;
const ANALYSIS_BASELINE_MAX_READ_FILES: usize = 24;
/// Keep phase context compact when feeding one phase into the next.
const MAX_SYNTHESIS_PHASE_CONTEXT_CHARS: usize = 900;
/// Limit chunk-level context in synthesis prompt (details stay in artifacts).
const MAX_SYNTHESIS_CHUNK_CONTEXT_CHARS: usize = 1400;
/// Keep evidence context concise to avoid synthesis overflow.
const MAX_SYNTHESIS_EVIDENCE_LINES: usize = 36;
/// Bound observed-path context passed to synthesis.
const MAX_SYNTHESIS_OBSERVED_PATHS: usize = 90;

// --- Regular (non-analysis) tool result truncation limits ---
// Applied when tool results are injected into the messages vec for the LLM
// during normal execution (outside analysis_phase mode). Frontend ToolResult
// events still receive the full untruncated content.

/// Maximum lines for Read tool output in regular execution context.
const REGULAR_READ_MAX_LINES: usize = 200;
/// Maximum characters for Read tool output in regular execution context.
const REGULAR_READ_MAX_CHARS: usize = 8000;
/// Maximum lines for Grep tool output in regular execution context.
const REGULAR_GREP_MAX_LINES: usize = 100;
/// Maximum characters for Grep tool output in regular execution context.
const REGULAR_GREP_MAX_CHARS: usize = 6000;
/// Maximum lines for LS/Glob tool output in regular execution context.
const REGULAR_LS_MAX_LINES: usize = 150;
/// Maximum characters for LS/Glob tool output in regular execution context.
const REGULAR_LS_MAX_CHARS: usize = 5000;
/// Maximum lines for Bash tool output in regular execution context.
const REGULAR_BASH_MAX_LINES: usize = 150;
/// Maximum characters for Bash tool output in regular execution context.
const REGULAR_BASH_MAX_CHARS: usize = 8000;

const ANALYZE_CACHE_MAX_ENTRIES: usize = 96;
const ANALYZE_CACHE_TTL_SECS: i64 = 60 * 60 * 6;

// ── Shared utility functions (used by 3+ submodules) ──────────────────

fn merge_usage(total: &mut UsageStats, delta: &UsageStats) {
    total.input_tokens += delta.input_tokens;
    total.output_tokens += delta.output_tokens;
    if let Some(thinking) = delta.thinking_tokens {
        total.thinking_tokens = Some(total.thinking_tokens.unwrap_or(0) + thinking);
    }
    if let Some(cache_read) = delta.cache_read_tokens {
        total.cache_read_tokens = Some(total.cache_read_tokens.unwrap_or(0) + cache_read);
    }
    if let Some(cache_creation) = delta.cache_creation_tokens {
        total.cache_creation_tokens =
            Some(total.cache_creation_tokens.unwrap_or(0) + cache_creation);
    }
}

fn truncate_for_log(text: &str, limit: usize) -> String {
    if limit == 0 {
        return String::new();
    }
    if text.len() <= limit {
        return text.to_string();
    }
    let mut cut = 0usize;
    for (idx, _) in text.char_indices() {
        if idx > limit {
            break;
        }
        cut = idx;
    }
    if cut == 0 {
        "...".to_string()
    } else {
        format!("{}...", &text[..cut])
    }
}

fn parse_tool_arguments(arguments: &Option<String>) -> Option<serde_json::Value> {
    arguments
        .as_ref()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
}

/// Parse an RFC3339 timestamp to Unix timestamp.
fn parse_timestamp(s: Option<String>) -> i64 {
    s.and_then(|ts| chrono::DateTime::parse_from_rfc3339(&ts).ok())
        .map(|dt| dt.timestamp())
        .unwrap_or_else(|| chrono::Utc::now().timestamp())
}

#[cfg(test)]
mod tests {
    use super::*;
    include!("../service_tests.rs");
}
