//! Design Document Service
//!
//! Service for loading, caching, and querying two-level design documents.
//! Supports both project-level and feature-level design documents.

mod loader;

pub use loader::*;
