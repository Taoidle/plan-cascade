//! Graph Workflow Checkpointer
//!
//! Provides a trait-based checkpointing system for graph workflow execution.
//! Checkpoints capture the full execution state of a graph workflow, enabling:
//! - Pause/resume of long-running workflows
//! - Human-in-the-loop interrupts (before/after specific nodes)
//! - Crash recovery
//!
//! ## Implementations
//! - `InMemoryCheckpointer` - for development and testing
//! - `SqliteCheckpointer` - for production (see checkpoint_store.rs)

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::RwLock;

use crate::utils::error::AppResult;

// ============================================================================
// Interrupt
// ============================================================================

/// Represents an interrupt point in graph workflow execution.
///
/// Interrupts allow pausing execution at specific points for human review
/// or external input.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Interrupt {
    /// Interrupt before executing a specific node.
    Before {
        /// The node ID to interrupt before.
        node_id: String,
    },
    /// Interrupt after executing a specific node.
    After {
        /// The node ID to interrupt after.
        node_id: String,
    },
    /// Dynamic interrupt with custom message and data.
    Dynamic {
        /// Human-readable message describing the interrupt.
        message: String,
        /// Optional structured data associated with the interrupt.
        #[serde(default)]
        data: Option<Value>,
    },
}

// ============================================================================
// GraphCheckpoint
// ============================================================================

/// A snapshot of graph workflow execution state.
///
/// Contains all information needed to resume a paused or interrupted workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphCheckpoint {
    /// Unique checkpoint identifier.
    pub id: String,
    /// Thread/execution identifier (groups related checkpoints).
    pub thread_id: String,
    /// Current state of all graph channels.
    pub state: HashMap<String, Value>,
    /// Current step/node being executed or about to be executed.
    pub step: String,
    /// Set of node IDs that are pending execution.
    pub pending_nodes: Vec<String>,
    /// The interrupt that caused this checkpoint (if any).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interrupt: Option<Interrupt>,
    /// ISO 8601 timestamp when the checkpoint was created.
    pub created_at: String,
}

impl GraphCheckpoint {
    /// Create a new checkpoint with the given parameters.
    pub fn new(
        thread_id: impl Into<String>,
        state: HashMap<String, Value>,
        step: impl Into<String>,
        pending_nodes: Vec<String>,
        interrupt: Option<Interrupt>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            thread_id: thread_id.into(),
            state,
            step: step.into(),
            pending_nodes,
            interrupt,
            created_at: chrono::Utc::now().to_rfc3339(),
        }
    }
}

// ============================================================================
// Checkpointer Trait
// ============================================================================

/// Trait for persisting and loading graph workflow checkpoints.
///
/// Implementations must be thread-safe (Send + Sync) and support
/// async operations for compatibility with the Tauri runtime.
#[async_trait]
pub trait Checkpointer: Send + Sync {
    /// Save a checkpoint. If a checkpoint with the same ID exists, it is replaced.
    async fn save(&self, checkpoint: GraphCheckpoint) -> AppResult<()>;

    /// Load the most recent checkpoint for a given thread ID.
    async fn load(&self, thread_id: &str) -> AppResult<Option<GraphCheckpoint>>;

    /// Load a specific checkpoint by its unique ID.
    async fn load_by_id(&self, checkpoint_id: &str) -> AppResult<Option<GraphCheckpoint>>;

    /// List all checkpoints for a given thread ID, ordered by creation time (newest first).
    async fn list(&self, thread_id: &str) -> AppResult<Vec<GraphCheckpoint>>;

    /// Delete a specific checkpoint by its unique ID.
    async fn delete(&self, checkpoint_id: &str) -> AppResult<bool>;
}

// ============================================================================
// InMemoryCheckpointer
// ============================================================================

/// In-memory implementation of `Checkpointer` for development and testing.
///
/// Stores checkpoints in a `HashMap` protected by a `RwLock`.
/// Data is lost when the process exits.
pub struct InMemoryCheckpointer {
    store: Arc<RwLock<HashMap<String, GraphCheckpoint>>>,
}

impl InMemoryCheckpointer {
    /// Create a new empty in-memory checkpointer.
    pub fn new() -> Self {
        Self {
            store: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryCheckpointer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Checkpointer for InMemoryCheckpointer {
    async fn save(&self, checkpoint: GraphCheckpoint) -> AppResult<()> {
        let mut store = self.store.write().await;
        store.insert(checkpoint.id.clone(), checkpoint);
        Ok(())
    }

    async fn load(&self, thread_id: &str) -> AppResult<Option<GraphCheckpoint>> {
        let store = self.store.read().await;
        let mut checkpoints: Vec<&GraphCheckpoint> = store
            .values()
            .filter(|cp| cp.thread_id == thread_id)
            .collect();
        // Sort by created_at descending (newest first)
        checkpoints.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(checkpoints.first().cloned().cloned())
    }

    async fn load_by_id(&self, checkpoint_id: &str) -> AppResult<Option<GraphCheckpoint>> {
        let store = self.store.read().await;
        Ok(store.get(checkpoint_id).cloned())
    }

    async fn list(&self, thread_id: &str) -> AppResult<Vec<GraphCheckpoint>> {
        let store = self.store.read().await;
        let mut checkpoints: Vec<GraphCheckpoint> = store
            .values()
            .filter(|cp| cp.thread_id == thread_id)
            .cloned()
            .collect();
        // Sort by created_at descending (newest first)
        checkpoints.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(checkpoints)
    }

    async fn delete(&self, checkpoint_id: &str) -> AppResult<bool> {
        let mut store = self.store.write().await;
        Ok(store.remove(checkpoint_id).is_some())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Interrupt Tests
    // ========================================================================

    #[test]
    fn test_interrupt_before_serialization() {
        let interrupt = Interrupt::Before {
            node_id: "node-1".to_string(),
        };
        let json = serde_json::to_string(&interrupt).unwrap();
        assert!(json.contains("\"type\":\"before\""));
        assert!(json.contains("\"node_id\":\"node-1\""));

        let parsed: Interrupt = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, interrupt);
    }

    #[test]
    fn test_interrupt_after_serialization() {
        let interrupt = Interrupt::After {
            node_id: "node-2".to_string(),
        };
        let json = serde_json::to_string(&interrupt).unwrap();
        assert!(json.contains("\"type\":\"after\""));

        let parsed: Interrupt = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, interrupt);
    }

    #[test]
    fn test_interrupt_dynamic_serialization() {
        let interrupt = Interrupt::Dynamic {
            message: "Need human approval".to_string(),
            data: Some(serde_json::json!({"risk_level": "high"})),
        };
        let json = serde_json::to_string(&interrupt).unwrap();
        assert!(json.contains("\"type\":\"dynamic\""));
        assert!(json.contains("\"message\":\"Need human approval\""));

        let parsed: Interrupt = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, interrupt);
    }

    #[test]
    fn test_interrupt_dynamic_no_data() {
        let interrupt = Interrupt::Dynamic {
            message: "Paused".to_string(),
            data: None,
        };
        let json = serde_json::to_string(&interrupt).unwrap();
        let parsed: Interrupt = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, interrupt);
    }

    // ========================================================================
    // GraphCheckpoint Tests
    // ========================================================================

    #[test]
    fn test_graph_checkpoint_new() {
        let mut state = HashMap::new();
        state.insert("counter".to_string(), serde_json::json!(42));

        let cp = GraphCheckpoint::new(
            "thread-1",
            state.clone(),
            "node-a",
            vec!["node-b".to_string(), "node-c".to_string()],
            Some(Interrupt::Before {
                node_id: "node-a".to_string(),
            }),
        );

        assert!(!cp.id.is_empty());
        assert_eq!(cp.thread_id, "thread-1");
        assert_eq!(cp.state.get("counter"), Some(&serde_json::json!(42)));
        assert_eq!(cp.step, "node-a");
        assert_eq!(cp.pending_nodes.len(), 2);
        assert!(cp.interrupt.is_some());
        assert!(!cp.created_at.is_empty());
    }

    #[test]
    fn test_graph_checkpoint_serialization() {
        let cp = GraphCheckpoint::new("thread-1", HashMap::new(), "node-a", vec![], None);

        let json = serde_json::to_string(&cp).unwrap();
        let parsed: GraphCheckpoint = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.thread_id, "thread-1");
        assert_eq!(parsed.step, "node-a");
        assert!(parsed.interrupt.is_none());
    }

    #[test]
    fn test_graph_checkpoint_with_interrupt_serialization() {
        let cp = GraphCheckpoint::new(
            "thread-2",
            HashMap::new(),
            "review-node",
            vec!["next-node".to_string()],
            Some(Interrupt::Dynamic {
                message: "Awaiting approval".to_string(),
                data: Some(serde_json::json!({"changes": 5})),
            }),
        );

        let json = serde_json::to_string_pretty(&cp).unwrap();
        let parsed: GraphCheckpoint = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.thread_id, "thread-2");
        assert!(parsed.interrupt.is_some());
        match parsed.interrupt.unwrap() {
            Interrupt::Dynamic { message, data } => {
                assert_eq!(message, "Awaiting approval");
                assert!(data.is_some());
            }
            _ => panic!("Expected Dynamic interrupt"),
        }
    }

    // ========================================================================
    // InMemoryCheckpointer Tests
    // ========================================================================

    #[tokio::test]
    async fn test_in_memory_save_and_load() {
        let cp_store = InMemoryCheckpointer::new();

        let cp = GraphCheckpoint::new("thread-1", HashMap::new(), "node-a", vec![], None);
        let cp_id = cp.id.clone();

        cp_store.save(cp).await.unwrap();

        let loaded = cp_store.load("thread-1").await.unwrap();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.id, cp_id);
        assert_eq!(loaded.thread_id, "thread-1");
    }

    #[tokio::test]
    async fn test_in_memory_load_nonexistent() {
        let cp_store = InMemoryCheckpointer::new();
        let loaded = cp_store.load("nonexistent").await.unwrap();
        assert!(loaded.is_none());
    }

    #[tokio::test]
    async fn test_in_memory_load_by_id() {
        let cp_store = InMemoryCheckpointer::new();

        let cp = GraphCheckpoint::new("thread-1", HashMap::new(), "node-a", vec![], None);
        let cp_id = cp.id.clone();

        cp_store.save(cp).await.unwrap();

        let loaded = cp_store.load_by_id(&cp_id).await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().id, cp_id);

        let missing = cp_store.load_by_id("nonexistent").await.unwrap();
        assert!(missing.is_none());
    }

    #[tokio::test]
    async fn test_in_memory_load_returns_newest() {
        let cp_store = InMemoryCheckpointer::new();

        // Save two checkpoints for the same thread with different timestamps
        let cp1 = GraphCheckpoint {
            id: "cp-1".to_string(),
            thread_id: "thread-1".to_string(),
            state: HashMap::new(),
            step: "node-a".to_string(),
            pending_nodes: vec![],
            interrupt: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
        };
        let cp2 = GraphCheckpoint {
            id: "cp-2".to_string(),
            thread_id: "thread-1".to_string(),
            state: HashMap::new(),
            step: "node-b".to_string(),
            pending_nodes: vec![],
            interrupt: None,
            created_at: "2026-01-02T00:00:00Z".to_string(),
        };

        cp_store.save(cp1).await.unwrap();
        cp_store.save(cp2).await.unwrap();

        let loaded = cp_store.load("thread-1").await.unwrap();
        assert!(loaded.is_some());
        // Should return the newer one (cp-2)
        assert_eq!(loaded.unwrap().id, "cp-2");
    }

    #[tokio::test]
    async fn test_in_memory_list() {
        let cp_store = InMemoryCheckpointer::new();

        let cp1 = GraphCheckpoint {
            id: "cp-1".to_string(),
            thread_id: "thread-1".to_string(),
            state: HashMap::new(),
            step: "node-a".to_string(),
            pending_nodes: vec![],
            interrupt: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
        };
        let cp2 = GraphCheckpoint {
            id: "cp-2".to_string(),
            thread_id: "thread-1".to_string(),
            state: HashMap::new(),
            step: "node-b".to_string(),
            pending_nodes: vec![],
            interrupt: None,
            created_at: "2026-01-02T00:00:00Z".to_string(),
        };
        let cp3 = GraphCheckpoint {
            id: "cp-3".to_string(),
            thread_id: "thread-2".to_string(),
            state: HashMap::new(),
            step: "node-c".to_string(),
            pending_nodes: vec![],
            interrupt: None,
            created_at: "2026-01-03T00:00:00Z".to_string(),
        };

        cp_store.save(cp1).await.unwrap();
        cp_store.save(cp2).await.unwrap();
        cp_store.save(cp3).await.unwrap();

        let list_t1 = cp_store.list("thread-1").await.unwrap();
        assert_eq!(list_t1.len(), 2);
        // Should be newest first
        assert_eq!(list_t1[0].id, "cp-2");
        assert_eq!(list_t1[1].id, "cp-1");

        let list_t2 = cp_store.list("thread-2").await.unwrap();
        assert_eq!(list_t2.len(), 1);
        assert_eq!(list_t2[0].id, "cp-3");

        let list_empty = cp_store.list("nonexistent").await.unwrap();
        assert!(list_empty.is_empty());
    }

    #[tokio::test]
    async fn test_in_memory_delete() {
        let cp_store = InMemoryCheckpointer::new();

        let cp = GraphCheckpoint::new("thread-1", HashMap::new(), "node-a", vec![], None);
        let cp_id = cp.id.clone();

        cp_store.save(cp).await.unwrap();

        // Delete existing
        let deleted = cp_store.delete(&cp_id).await.unwrap();
        assert!(deleted);

        // Verify it's gone
        let loaded = cp_store.load_by_id(&cp_id).await.unwrap();
        assert!(loaded.is_none());

        // Delete non-existent
        let deleted_again = cp_store.delete(&cp_id).await.unwrap();
        assert!(!deleted_again);
    }

    #[tokio::test]
    async fn test_in_memory_save_overwrites() {
        let cp_store = InMemoryCheckpointer::new();

        let cp1 = GraphCheckpoint {
            id: "cp-1".to_string(),
            thread_id: "thread-1".to_string(),
            state: HashMap::new(),
            step: "node-a".to_string(),
            pending_nodes: vec![],
            interrupt: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
        };

        cp_store.save(cp1).await.unwrap();

        // Save with same ID but different step
        let cp1_updated = GraphCheckpoint {
            id: "cp-1".to_string(),
            thread_id: "thread-1".to_string(),
            state: HashMap::new(),
            step: "node-b".to_string(),
            pending_nodes: vec![],
            interrupt: None,
            created_at: "2026-01-02T00:00:00Z".to_string(),
        };

        cp_store.save(cp1_updated).await.unwrap();

        let loaded = cp_store.load_by_id("cp-1").await.unwrap().unwrap();
        assert_eq!(loaded.step, "node-b");

        // Should still have only 1 checkpoint for thread-1
        let list = cp_store.list("thread-1").await.unwrap();
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn test_in_memory_checkpointer_default() {
        let cp_store = InMemoryCheckpointer::default();
        // Just verify it creates without panicking
        assert!(Arc::strong_count(&cp_store.store) > 0);
    }
}
