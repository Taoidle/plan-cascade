//! Recovery Service
//!
//! Provides detection of interrupted executions and resumption capabilities.
//! Scans the SQLite database on app launch for incomplete tasks and allows
//! users to resume from the exact point of interruption.
//!
//! ## Components
//! - **Detector**: Scans for incomplete executions across all modes
//! - **Resume**: Restores execution context and continues from checkpoint

pub mod detector;
pub mod resume;

pub use detector::{IncompleteTask, RecoveryDetector};
pub use resume::{ResumeEngine, ResumeEvent, ResumeResult};
