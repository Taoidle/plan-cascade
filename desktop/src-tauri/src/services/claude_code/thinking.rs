//! ThinkingManager
//!
//! Manages thinking block state for collapsible display in the UI.
//! Tracks multiple concurrent thinking blocks and provides state for frontend rendering.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// Maximum characters for auto-generated summary
const SUMMARY_MAX_LEN: usize = 100;

/// A thinking block with its content and UI state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingBlock {
    /// Unique identifier for this thinking block
    pub id: String,
    /// The full thinking content
    pub content: String,
    /// Whether the thinking process is complete
    pub is_complete: bool,
    /// Whether the block is collapsed in the UI
    pub is_collapsed: bool,
    /// Auto-generated summary (first N characters)
    pub summary: String,
    /// Character count for display
    pub char_count: usize,
    /// Line count for display
    pub line_count: usize,
}

impl ThinkingBlock {
    /// Create a new thinking block
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            content: String::new(),
            is_complete: false,
            is_collapsed: false,
            summary: String::new(),
            char_count: 0,
            line_count: 0,
        }
    }

    /// Append content to the block
    pub fn append_content(&mut self, content: &str) {
        self.content.push_str(content);
        self.update_stats();
    }

    /// Mark the block as complete and generate summary
    pub fn finalize(&mut self) {
        self.is_complete = true;
        self.generate_summary();
    }

    /// Toggle the collapsed state
    pub fn toggle_collapsed(&mut self) {
        self.is_collapsed = !self.is_collapsed;
    }

    /// Update statistics (char count, line count)
    fn update_stats(&mut self) {
        self.char_count = self.content.len();
        self.line_count = self.content.lines().count();
    }

    /// Generate a summary from the content
    fn generate_summary(&mut self) {
        let trimmed = self.content.trim();
        if trimmed.is_empty() {
            self.summary = String::new();
            return;
        }

        // Take first N characters, trying to break at word boundary
        if trimmed.len() <= SUMMARY_MAX_LEN {
            self.summary = trimmed.replace('\n', " ");
        } else {
            let truncated = &trimmed[..SUMMARY_MAX_LEN];
            // Try to find the last word boundary
            let summary = if let Some(last_space) = truncated.rfind(' ') {
                &truncated[..last_space]
            } else {
                truncated
            };
            self.summary = format!("{}...", summary.replace('\n', " "));
        }
    }
}

/// Manages multiple thinking blocks
#[derive(Debug, Default)]
pub struct ThinkingManager {
    /// Map of thinking_id to ThinkingBlock
    blocks: HashMap<String, ThinkingBlock>,
    /// Counter for generating IDs when none provided
    id_counter: u32,
}

impl ThinkingManager {
    /// Create a new thinking manager
    pub fn new() -> Self {
        Self::default()
    }

    /// Generate a unique thinking ID
    fn generate_id(&mut self) -> String {
        self.id_counter += 1;
        format!("thinking-{}", self.id_counter)
    }

    /// Handle a thinking start event
    ///
    /// Creates a new thinking block entry.
    pub fn on_thinking_start(&mut self, thinking_id: Option<&str>) -> String {
        let id = thinking_id
            .map(|s| s.to_string())
            .unwrap_or_else(|| self.generate_id());

        let block = ThinkingBlock::new(&id);
        self.blocks.insert(id.clone(), block);
        id
    }

    /// Handle a thinking delta event
    ///
    /// Appends content to the active block.
    pub fn on_thinking_delta(&mut self, thinking_id: Option<&str>, content: &str) {
        let id = thinking_id
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                // Find the most recent incomplete block
                self.blocks
                    .iter()
                    .filter(|(_, b)| !b.is_complete)
                    .max_by_key(|(id, _)| id.as_str())
                    .map(|(id, _)| id.clone())
                    .unwrap_or_else(|| self.on_thinking_start(None))
            });

        if let Some(block) = self.blocks.get_mut(&id) {
            block.append_content(content);
        } else {
            // Create the block if it doesn't exist
            let mut block = ThinkingBlock::new(&id);
            block.append_content(content);
            self.blocks.insert(id, block);
        }
    }

    /// Handle a thinking end event
    ///
    /// Finalizes the block and generates a summary.
    pub fn on_thinking_end(&mut self, thinking_id: Option<&str>) {
        let id = thinking_id
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                // Find the most recent incomplete block
                self.blocks
                    .iter()
                    .filter(|(_, b)| !b.is_complete)
                    .max_by_key(|(id, _)| id.as_str())
                    .map(|(id, _)| id.clone())
                    .unwrap_or_default()
            });

        if let Some(block) = self.blocks.get_mut(&id) {
            block.finalize();
        }
    }

    /// Get a thinking block by ID
    pub fn get_block(&self, thinking_id: &str) -> Option<&ThinkingBlock> {
        self.blocks.get(thinking_id)
    }

    /// Get a mutable thinking block by ID
    pub fn get_block_mut(&mut self, thinking_id: &str) -> Option<&mut ThinkingBlock> {
        self.blocks.get_mut(thinking_id)
    }

    /// Get all thinking blocks
    pub fn get_all_blocks(&self) -> Vec<&ThinkingBlock> {
        self.blocks.values().collect()
    }

    /// Get all thinking blocks as owned values (for serialization)
    pub fn get_all_blocks_owned(&self) -> Vec<ThinkingBlock> {
        self.blocks.values().cloned().collect()
    }

    /// Toggle collapsed state for a block
    pub fn toggle_collapsed(&mut self, thinking_id: &str) -> bool {
        if let Some(block) = self.blocks.get_mut(thinking_id) {
            block.toggle_collapsed();
            true
        } else {
            false
        }
    }

    /// Set collapsed state for a block
    pub fn set_collapsed(&mut self, thinking_id: &str, collapsed: bool) -> bool {
        if let Some(block) = self.blocks.get_mut(thinking_id) {
            block.is_collapsed = collapsed;
            true
        } else {
            false
        }
    }

    /// Get the count of active (incomplete) thinking blocks
    pub fn active_count(&self) -> usize {
        self.blocks.values().filter(|b| !b.is_complete).count()
    }

    /// Get the total count of thinking blocks
    pub fn total_count(&self) -> usize {
        self.blocks.len()
    }

    /// Clear all thinking blocks
    pub fn clear(&mut self) {
        self.blocks.clear();
        self.id_counter = 0;
    }

    /// Remove a specific thinking block
    pub fn remove_block(&mut self, thinking_id: &str) -> Option<ThinkingBlock> {
        self.blocks.remove(thinking_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thinking_block_creation() {
        let block = ThinkingBlock::new("test-1");
        assert_eq!(block.id, "test-1");
        assert!(block.content.is_empty());
        assert!(!block.is_complete);
        assert!(!block.is_collapsed);
    }

    #[test]
    fn test_thinking_block_append_content() {
        let mut block = ThinkingBlock::new("test-1");
        block.append_content("Hello ");
        block.append_content("world!");
        assert_eq!(block.content, "Hello world!");
        assert_eq!(block.char_count, 12);
    }

    #[test]
    fn test_thinking_block_finalize() {
        let mut block = ThinkingBlock::new("test-1");
        block.append_content("This is a test thinking block with some content.");
        block.finalize();
        assert!(block.is_complete);
        assert!(!block.summary.is_empty());
    }

    #[test]
    fn test_thinking_block_summary_long() {
        let mut block = ThinkingBlock::new("test-1");
        let long_content = "a ".repeat(100); // 200 characters
        block.append_content(&long_content);
        block.finalize();
        assert!(block.summary.ends_with("..."));
        assert!(block.summary.len() <= SUMMARY_MAX_LEN + 3); // +3 for "..."
    }

    #[test]
    fn test_thinking_block_toggle_collapsed() {
        let mut block = ThinkingBlock::new("test-1");
        assert!(!block.is_collapsed);
        block.toggle_collapsed();
        assert!(block.is_collapsed);
        block.toggle_collapsed();
        assert!(!block.is_collapsed);
    }

    #[test]
    fn test_thinking_manager_creation() {
        let manager = ThinkingManager::new();
        assert_eq!(manager.total_count(), 0);
        assert_eq!(manager.active_count(), 0);
    }

    #[test]
    fn test_thinking_manager_on_thinking_start() {
        let mut manager = ThinkingManager::new();

        let id1 = manager.on_thinking_start(Some("t1"));
        assert_eq!(id1, "t1");

        let id2 = manager.on_thinking_start(None);
        assert_eq!(id2, "thinking-1");

        assert_eq!(manager.total_count(), 2);
    }

    #[test]
    fn test_thinking_manager_on_thinking_delta() {
        let mut manager = ThinkingManager::new();

        manager.on_thinking_start(Some("t1"));
        manager.on_thinking_delta(Some("t1"), "Hello ");
        manager.on_thinking_delta(Some("t1"), "world!");

        let block = manager.get_block("t1").unwrap();
        assert_eq!(block.content, "Hello world!");
    }

    #[test]
    fn test_thinking_manager_on_thinking_end() {
        let mut manager = ThinkingManager::new();

        manager.on_thinking_start(Some("t1"));
        manager.on_thinking_delta(Some("t1"), "Some thinking content");
        manager.on_thinking_end(Some("t1"));

        let block = manager.get_block("t1").unwrap();
        assert!(block.is_complete);
        assert!(!block.summary.is_empty());
    }

    #[test]
    fn test_thinking_manager_get_all_blocks() {
        let mut manager = ThinkingManager::new();

        manager.on_thinking_start(Some("t1"));
        manager.on_thinking_start(Some("t2"));

        let blocks = manager.get_all_blocks();
        assert_eq!(blocks.len(), 2);
    }

    #[test]
    fn test_thinking_manager_toggle_collapsed() {
        let mut manager = ThinkingManager::new();

        manager.on_thinking_start(Some("t1"));

        assert!(manager.toggle_collapsed("t1"));
        assert!(manager.get_block("t1").unwrap().is_collapsed);

        assert!(manager.toggle_collapsed("t1"));
        assert!(!manager.get_block("t1").unwrap().is_collapsed);

        // Non-existent block
        assert!(!manager.toggle_collapsed("nonexistent"));
    }

    #[test]
    fn test_thinking_manager_active_count() {
        let mut manager = ThinkingManager::new();

        manager.on_thinking_start(Some("t1"));
        manager.on_thinking_start(Some("t2"));
        assert_eq!(manager.active_count(), 2);

        manager.on_thinking_end(Some("t1"));
        assert_eq!(manager.active_count(), 1);

        manager.on_thinking_end(Some("t2"));
        assert_eq!(manager.active_count(), 0);
    }

    #[test]
    fn test_thinking_manager_clear() {
        let mut manager = ThinkingManager::new();

        manager.on_thinking_start(Some("t1"));
        manager.on_thinking_start(Some("t2"));
        assert_eq!(manager.total_count(), 2);

        manager.clear();
        assert_eq!(manager.total_count(), 0);
    }

    #[test]
    fn test_thinking_manager_remove_block() {
        let mut manager = ThinkingManager::new();

        manager.on_thinking_start(Some("t1"));
        manager.on_thinking_delta(Some("t1"), "content");

        let removed = manager.remove_block("t1");
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().content, "content");

        assert!(manager.get_block("t1").is_none());
    }

    #[test]
    fn test_thinking_manager_delta_without_start() {
        let mut manager = ThinkingManager::new();

        // Delta without start should auto-create block
        manager.on_thinking_delta(Some("auto"), "content");

        let block = manager.get_block("auto").unwrap();
        assert_eq!(block.content, "content");
    }

    #[test]
    fn test_thinking_block_serialization() {
        let mut block = ThinkingBlock::new("t1");
        block.append_content("test content");
        block.finalize();

        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains("\"id\":\"t1\""));
        assert!(json.contains("\"content\":\"test content\""));
        assert!(json.contains("\"is_complete\":true"));
    }
}
