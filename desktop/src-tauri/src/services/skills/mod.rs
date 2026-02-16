//! Skill System
//!
//! Universal skill system compatible with plan-cascade SKILL.md format,
//! adk-rust .skills/ format, and convention files (CLAUDE.md, AGENTS.md, etc.).
//!
//! Architecture:
//! - model.rs:     Core data types (SkillDocument, SkillIndex, etc.)
//! - parser.rs:    Universal SKILL.md parser for 3 formats
//! - config.rs:    Load/merge external-skills.json + user config
//! - discovery.rs: Filesystem scanning from 4 sources
//! - index.rs:     SkillIndex construction with SHA-256 hashing
//! - select.rs:    Two-phase selection (auto-detection + lexical scoring)
//! - injector.rs:  Format skills into system prompt
//! - generator.rs: Auto-generate skills from successful sessions

pub mod config;
pub mod discovery;
pub mod generator;
pub mod index;
pub mod injector;
pub mod model;
pub mod parser;
pub mod select;

pub use model::*;
