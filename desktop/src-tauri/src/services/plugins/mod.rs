//! Plugin System
//!
//! Claude Code-compatible plugin system that loads community plugins
//! with their skills, commands, hooks, agents, and instructions.
//!
//! Architecture:
//! - models.rs:      Data types (PluginManifest, LoadedPlugin, HookEvent, etc.)
//! - loader.rs:      Plugin discovery and parsing from 3 source locations
//! - dispatcher.rs:  Bridge Claude Code shell hooks to AgenticHooks
//! - manager.rs:     Unified entry point for plugin management

pub mod dispatcher;
pub mod installer;
pub mod lifecycle;
pub mod loader;
pub mod manager;
pub mod marketplace;
pub mod models;
pub mod registry;
pub mod settings;

pub use models::*;
