//! Design Document Service
//!
//! Service for loading, caching, and querying two-level design documents.
//! Supports both project-level and feature-level design documents.
//! Includes generation from PRDs and import from external formats.

mod generator;
mod importer;
mod loader;

pub use generator::*;
pub use importer::*;
pub use loader::*;
