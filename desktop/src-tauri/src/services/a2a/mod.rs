//! A2A (Agent-to-Agent) Protocol Module
//!
//! Implements the A2A communication protocol for discovering and interacting
//! with remote agents. Based on JSON-RPC 2.0 over HTTP(S) with SSE for streaming.
//!
//! # Architecture
//!
//! - `types` — Core data structures: `AgentCard`, `A2aTaskRequest`, `A2aTaskResponse`,
//!   `A2aStreamEvent`, and `A2aError`
//! - `discovery` — Agent discovery via `GET /.well-known/agent.json`
//! - `client` — `A2aClient` for sending tasks and receiving streaming responses
//!
//! # Usage
//!
//! ```rust,no_run
//! use crate::services::a2a::{A2aClient, A2aTaskRequest};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let client = A2aClient::new()?;
//!
//! // Discover a remote agent
//! let card = client.discover("https://agent.example.com").await?;
//! println!("Found agent: {} ({})", card.name, card.description);
//!
//! // Send a task
//! let request = A2aTaskRequest::send_task("task-1", "Review this code", 1);
//! let response = client.send_task(&card.endpoint, request).await?;
//! let result = response.into_result()?;
//! println!("Task result: {:?}", result.output);
//! # Ok(())
//! # }
//! ```
//!
//! # Design Decision (ADR-F003)
//!
//! Uses JSON-RPC 2.0 over HTTP(S) with SSE for streaming, matching
//! the adk-rust A2A protocol design.

pub mod client;
pub mod discovery;
pub mod remote_agent;
pub mod service;
pub mod types;

// Re-export core types for convenient access
pub use client::{A2aClient, A2aClientConfig};
pub use discovery::{discover_agent, WELL_KNOWN_PATH};
pub use remote_agent::RemoteA2aAgent;
pub use service::{A2aService, DiscoveredAgent, RegisteredRemoteAgent};
pub use types::{
    A2aError, A2aStreamEvent, A2aTaskParams, A2aTaskRequest, A2aTaskResponse, A2aTaskResult,
    AgentCard, JsonRpcError,
};
