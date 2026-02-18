//! Graph Workflow Services
//!
//! Provides checkpointing and persistence services for graph workflow execution.
//!
//! - `checkpointer.rs` - Checkpointer trait and InMemoryCheckpointer
//! - `checkpoint_store.rs` - SqliteCheckpointer for production persistence

pub mod checkpointer;
pub mod checkpoint_store;

pub use checkpointer::{Checkpointer, GraphCheckpoint, InMemoryCheckpointer, Interrupt};
pub use checkpoint_store::SqliteCheckpointer;
