use super::*;

/// Session memory injected into compacted conversations to prevent post-compaction re-reads.
///
/// Built from the tool executor's read cache and conversation snippets before LLM summary
/// compaction. After compaction, the memory is placed between the original prompt and the
/// LLM summary so the agent retains awareness of files it has already read and key findings.
#[derive(Debug, Clone)]
pub(super) struct SessionMemory {
    /// Files previously read in this session: (path, line_count, size_bytes)
    pub(super) files_read: Vec<(String, usize, u64)>,
    /// Key findings extracted from compacted conversation snippets
    pub(super) key_findings: Vec<String>,
    /// Original task description (first user message, truncated)
    pub(super) task_description: String,
    /// Tool usage counts: tool_name -> count
    pub(super) tool_usage_counts: HashMap<String, usize>,
}

impl SessionMemory {
    /// Generate a structured context string for injection into the conversation.
    ///
    /// The output explicitly lists files already read with sizes and includes a
    /// "Do NOT re-read" instruction to prevent wasteful duplicate file reads after
    /// context compaction.
    pub(super) fn to_context_string(&self) -> String {
        let mut parts: Vec<String> = Vec::new();

        parts.push("[Session Memory - Preserved across context compaction]".to_string());

        // Task description
        if !self.task_description.is_empty() {
            parts.push(format!("\n## Task\n{}", self.task_description));
        }

        // Files already read
        if !self.files_read.is_empty() {
            parts.push("\n## Files Already Read".to_string());
            parts.push(
                "IMPORTANT: Do NOT re-read these files. Their contents were already processed."
                    .to_string(),
            );
            for (path, lines, bytes) in &self.files_read {
                parts.push(format!("- {} ({} lines, {} bytes)", path, lines, bytes));
            }
        }

        // Key findings
        if !self.key_findings.is_empty() {
            parts.push("\n## Key Findings".to_string());
            for finding in &self.key_findings {
                parts.push(format!("- {}", finding));
            }
        }

        // Tool usage summary
        if !self.tool_usage_counts.is_empty() {
            let mut sorted_tools: Vec<(&String, &usize)> = self.tool_usage_counts.iter().collect();
            sorted_tools.sort_by(|a, b| b.1.cmp(a.1));
            let tool_summary: Vec<String> = sorted_tools
                .iter()
                .map(|(name, count)| format!("{}({})", name, count))
                .collect();
            parts.push(format!("\n## Tool Usage\n{}", tool_summary.join(", ")));
        }

        parts.join("\n")
    }
}

/// Extract key findings from conversation snippets being compacted.
///
/// Scans text snippets for lines that look like conclusions, discoveries, or decisions.
/// Returns deduplicated findings sorted by length (shortest first) to keep summaries concise.
pub(super) fn extract_key_findings(snippets: &[String]) -> Vec<String> {
    let finding_indicators = [
        "found",
        "discovered",
        "confirmed",
        "determined",
        "decided",
        "issue:",
        "error:",
        "warning:",
        "note:",
        "important:",
        "conclusion:",
        "result:",
        "observation:",
        "the file contains",
        "the code uses",
        "the project uses",
        "implemented",
        "fixed",
        "created",
        "modified",
        "updated",
    ];

    let mut findings: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let max_findings = 15;

    for snippet in snippets {
        for line in snippet.lines() {
            let trimmed = line.trim();
            if trimmed.len() < 20 || trimmed.len() > 300 {
                continue;
            }
            let lower = trimmed.to_lowercase();
            let is_finding = finding_indicators.iter().any(|ind| lower.contains(ind));
            if is_finding {
                // Normalize to avoid near-duplicates
                let normalized = trimmed.to_string();
                if !seen.contains(&lower) {
                    seen.insert(lower);
                    findings.push(normalized);
                    if findings.len() >= max_findings {
                        return findings;
                    }
                }
            }
        }
    }

    findings
}

/// Escalation level returned by the loop detector.
///
/// Callers match on this to determine the appropriate intervention:
/// - `Warning`: Inject a warning message into the conversation.
/// - `StripTools`: Inject warning AND remove the listed tools from subsequent LLM calls.
/// - `ForceTerminate`: Exit the agentic loop immediately.
#[derive(Debug, Clone)]
pub(super) enum LoopDetection {
    /// Level 1: First detection - inject warning message
    Warning(String),
    /// Level 2: Second detection - warning + list of tool names to strip
    StripTools(String, Vec<String>),
    /// Level 3: Third+ detection - must exit loop immediately
    ForceTerminate(String),
}

/// Detects consecutive identical tool calls and macro-loop patterns to break infinite loops.
///
/// Tracks the last (tool_name, args_hash) and counts consecutive repetitions.
/// Also maintains a sliding window of recent calls for macro-pattern detection.
/// When the count reaches the configured threshold, returns a detection result
/// that can be used to escalate intervention (warn, strip tools, force terminate).
///
/// ADR-002: Sliding window for macro-loop detection.
/// ADR-004: Pattern-based loop detection is cheaper than waiting for max_iterations=50.
#[derive(Debug)]
pub(super) struct ToolCallLoopDetector {
    /// Threshold of consecutive identical calls before triggering
    pub(super) threshold: u32,
    /// Last seen (tool_name, args_hash) tuple
    last_call: Option<(String, u64)>,
    /// Count of consecutive identical calls
    pub(super) consecutive_count: u32,
    /// Cumulative detection count across the session (never reset)
    total_detections: u32,
    /// Sliding window of recent calls for macro-pattern detection
    pub(super) recent_calls: VecDeque<(String, u64)>,
    /// Maximum size of the sliding window
    pub(super) window_size: usize,
    /// Tool names that have been stripped due to Level 2 escalation
    stripped_tools: HashSet<String>,
}

impl ToolCallLoopDetector {
    pub(super) fn new(threshold: u32, window_size: usize) -> Self {
        Self {
            threshold,
            last_call: None,
            consecutive_count: 0,
            total_detections: 0,
            recent_calls: VecDeque::with_capacity(window_size),
            window_size,
            stripped_tools: HashSet::new(),
        }
    }

    /// Returns the cumulative number of loop detections (never resets).
    pub(super) fn total_detections(&self) -> u32 {
        self.total_detections
    }

    /// Returns the set of tool names stripped due to Level 2 escalation.
    pub(super) fn stripped_tools(&self) -> &HashSet<String> {
        &self.stripped_tools
    }

    /// Detect macro-loop (rotating pattern) in the sliding window.
    ///
    /// Examines `recent_calls` for repeating cycles of length 2 through 6.
    /// Returns `Some((tool_names_in_cycle, cycle_length))` if a macro-loop is found.
    pub(super) fn detect_macro_loop(&self) -> Option<(Vec<String>, usize)> {
        let n = self.recent_calls.len();
        // Try cycle lengths from 2 to 6
        for cycle_len in 2..=6 {
            // Need at least 2 full cycles in the window
            if n < 2 * cycle_len {
                continue;
            }
            // Extract the candidate pattern from the tail of recent_calls
            let tail_start = n - cycle_len;
            let prev_start = n - 2 * cycle_len;

            let mut matched = true;
            for i in 0..cycle_len {
                let tail_entry = &self.recent_calls[tail_start + i];
                let prev_entry = &self.recent_calls[prev_start + i];
                if tail_entry != prev_entry {
                    matched = false;
                    break;
                }
            }

            if matched {
                let tool_names: Vec<String> = (tail_start..tail_start + cycle_len)
                    .map(|i| self.recent_calls[i].0.clone())
                    .collect();
                return Some((tool_names, cycle_len));
            }
        }
        None
    }

    /// Build the appropriate escalation level based on total_detections.
    ///
    /// Adds looping tool names to `stripped_tools` for Level 2+.
    pub(super) fn escalate(&mut self, msg: String, tool_names: Vec<String>) -> LoopDetection {
        match self.total_detections {
            1 => LoopDetection::Warning(msg),
            2 => {
                for t in &tool_names {
                    self.stripped_tools.insert(t.clone());
                }
                LoopDetection::StripTools(msg, tool_names)
            }
            _ => {
                // Level 3+: force terminate. Still record stripped tools for completeness.
                for t in &tool_names {
                    self.stripped_tools.insert(t.clone());
                }
                LoopDetection::ForceTerminate(format!(
                    "[FORCE TERMINATE] {} Loop detected {} times. The agent is unable to make progress. \
                     Terminating the agentic loop. Please review the conversation and intervene manually.",
                    msg, self.total_detections
                ))
            }
        }
    }

    /// Record a tool call and return a detection result if a loop is detected.
    ///
    /// Returns `Some(LoopDetection)` when the same tool+args have been called `threshold`
    /// times consecutively, or when a macro-loop pattern is detected. `None` otherwise.
    /// Each call is also pushed onto the `recent_calls` sliding window.
    ///
    /// When `is_dedup` is true, a lower threshold (2) is used for consecutive detection
    /// since dedup results are deterministic and repeating them wastes tokens.
    pub(super) fn record_call(
        &mut self,
        tool_name: &str,
        args_str: &str,
        is_dedup: bool,
    ) -> Option<LoopDetection> {
        // Skip invalid tool calls with empty names — these are adapter
        // parse errors (e.g. Qwen3-MAX producing empty tool names) and
        // should not pollute loop detection.
        if tool_name.trim().is_empty() {
            return None;
        }

        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        args_str.hash(&mut hasher);
        let args_hash = hasher.finish();

        let call_key = (tool_name.to_string(), args_hash);

        // Push onto sliding window, evicting oldest if at capacity
        if self.recent_calls.len() >= self.window_size {
            self.recent_calls.pop_front();
        }
        self.recent_calls.push_back(call_key.clone());

        if self.last_call.as_ref() == Some(&call_key) {
            self.consecutive_count += 1;
        } else {
            self.last_call = Some(call_key);
            self.consecutive_count = 1;
        }

        // Use a lower threshold for dedup cycles (deterministic, no need to wait)
        let effective_threshold = if is_dedup {
            self.threshold.min(2)
        } else {
            self.threshold
        };

        // Check consecutive-identical detection first
        if self.consecutive_count >= effective_threshold
            && self.consecutive_count % effective_threshold == 0
        {
            self.total_detections += 1;
            let msg = format!(
                "[LOOP DETECTED] You have made the same identical tool call ({}) {} times consecutively \
                 with the same arguments. This is an infinite loop. STOP repeating this call. \
                 Use the information you already have from previous tool results to proceed with the task. \
                 If the previous result was a dedup/cache message, the file content was already read earlier \
                 in this session 閳?refer to the session memory above for details.",
                tool_name, self.consecutive_count
            );
            let tool_names = vec![tool_name.to_string()];
            return Some(self.escalate(msg, tool_names));
        }

        // If no consecutive detection, check for macro-loop patterns
        if let Some((cycle_tools, _cycle_len)) = self.detect_macro_loop() {
            self.total_detections += 1;
            let cycle_desc = cycle_tools.join(" -> ");
            let cycle_repeated = format!("{} -> {}", &cycle_desc, &cycle_desc);
            let msg = format!(
                "[MACRO-LOOP DETECTED] Repeating cycle: {}. \
                 You are stuck in a loop repeating the same sequence of tool calls. \
                 STOP this pattern and use the information you already have to answer the question. \
                 Do NOT call any of these tools again: {}.",
                cycle_repeated,
                cycle_tools.join(", ")
            );
            let tool_names = cycle_tools;
            return Some(self.escalate(msg, tool_names));
        }

        None
    }
}

/// Marker string embedded in session memory messages for compaction identification.
///
/// Both `compact_messages()` (LLM-summary) and `compact_messages_prefix_stable()` use
/// this marker to locate and preserve the Layer 2 session memory message during compaction.
pub(super) const SESSION_MEMORY_V1_MARKER: &str = "[SESSION_MEMORY_V1]";

/// Manages the Layer 2 session memory within the three-layer context architecture.
///
/// # Three-Layer Context Architecture
/// - **Layer 1 (Stable):** System prompt + index summary + tools (message index 0)
/// - **Layer 2 (Semi-stable):** Session memory 閳?files read, key findings (fixed index)
/// - **Layer 3 (Volatile):** Conversation messages (tool calls, responses, etc.)
///
/// `SessionMemoryManager` maintains the session memory at a fixed message index,
/// accumulates file reads and findings, and updates the memory in-place before
/// each LLM call. The `[SESSION_MEMORY_V1]` marker enables compaction strategies
/// to identify and preserve this layer.
pub(super) struct SessionMemoryManager {
    /// Fixed position in the messages vec (after system prompt at index 0)
    memory_index: usize,
    /// Marker string prepended to session memory content
    marker: &'static str,
}

impl SessionMemoryManager {
    /// Create a new SessionMemoryManager with the given memory index.
    ///
    /// Typically `memory_index` is 1 (right after the system prompt at index 0).
    pub(super) fn new(memory_index: usize) -> Self {
        Self {
            memory_index,
            marker: SESSION_MEMORY_V1_MARKER,
        }
    }

    /// Build a session memory message with the V1 marker prepended.
    ///
    /// The message is an assistant-role message containing:
    /// 1. The `[SESSION_MEMORY_V1]` marker (for compaction identification)
    /// 2. The full session memory context string (files read, findings, etc.)
    pub(super) fn build_memory_message(
        &self,
        files_read: Vec<(String, usize, u64)>,
        findings: Vec<String>,
    ) -> Message {
        let memory = SessionMemory {
            files_read,
            key_findings: findings,
            task_description: String::new(),
            tool_usage_counts: HashMap::new(),
        };

        let content = format!("{}\n{}", self.marker, memory.to_context_string());
        Message::assistant(content)
    }

    /// Update existing session memory in-place, or insert a new one if none exists.
    ///
    /// If the message at `memory_index` contains the `SESSION_MEMORY_V1` marker,
    /// it is replaced with a new session memory message built from the provided data.
    /// Otherwise, a new message is inserted at `memory_index`.
    pub(super) fn update_or_insert(
        &self,
        messages: &mut Vec<Message>,
        files_read: Vec<(String, usize, u64)>,
        findings: Vec<String>,
    ) {
        let new_msg = self.build_memory_message(files_read, findings);

        // Check if there's already a session memory message at the expected index
        if self.memory_index < messages.len() {
            if Self::message_has_marker(&messages[self.memory_index]) {
                // Replace in-place
                messages[self.memory_index] = new_msg;
                return;
            }
        }

        // Also scan for the marker elsewhere (in case messages shifted)
        if let Some(idx) = Self::find_memory_index(messages) {
            messages[idx] = new_msg;
            return;
        }

        // No existing session memory 閳?insert at the memory_index position
        let insert_at = self.memory_index.min(messages.len());
        messages.insert(insert_at, new_msg);
    }

    /// Scan messages for the SESSION_MEMORY_V1 marker and return the index if found.
    pub(super) fn find_memory_index(messages: &[Message]) -> Option<usize> {
        for (i, msg) in messages.iter().enumerate() {
            if Self::message_has_marker(msg) {
                return Some(i);
            }
        }
        None
    }

    /// Check whether a message contains the SESSION_MEMORY_V1 marker.
    pub(super) fn message_has_marker(msg: &Message) -> bool {
        for content in &msg.content {
            if let MessageContent::Text { text } = content {
                if text.contains(SESSION_MEMORY_V1_MARKER) {
                    return true;
                }
            }
        }
        false
    }
}
