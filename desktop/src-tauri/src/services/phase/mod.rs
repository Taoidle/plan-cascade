//! Phase Management Service
//!
//! Manages Phase to Agent mapping and phase-specific configurations.

mod manager;

pub use manager::{Phase, PhaseConfig, PhaseError, PhaseManager};
