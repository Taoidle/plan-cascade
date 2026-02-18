//! Core Error Types
//!
//! Defines the foundational error types used across the Plan Cascade workspace.
//! These error types are dependency-free (only thiserror + std) to keep the core
//! crate lightweight.
//!
//! The main application crate extends these with additional error variants
//! (e.g., Database, Sqlite, Keyring) that require heavier dependencies.

use thiserror::Error;

/// Core error type for the Plan Cascade workspace.
///
/// This is the minimal error set that the core crate needs. The application
/// crate defines additional variants for storage, network, etc.
#[derive(Error, Debug)]
pub enum CoreError {
    /// Configuration errors
    #[error("Configuration error: {0}")]
    Config(String),

    /// File I/O errors
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization errors
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Command execution errors
    #[error("Command error: {0}")]
    Command(String),

    /// Validation errors
    #[error("Validation error: {0}")]
    Validation(String),

    /// Not found errors
    #[error("Not found: {0}")]
    NotFound(String),

    /// Parse errors
    #[error("Parse error: {0}")]
    Parse(String),

    /// Generic internal errors
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Result type alias for core errors
pub type CoreResult<T> = Result<T, CoreError>;

impl CoreError {
    /// Create a config error
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }

    /// Create a command error
    pub fn command(msg: impl Into<String>) -> Self {
        Self::Command(msg.into())
    }

    /// Create a validation error
    pub fn validation(msg: impl Into<String>) -> Self {
        Self::Validation(msg.into())
    }

    /// Create a not found error
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::NotFound(msg.into())
    }

    /// Create a parse error
    pub fn parse(msg: impl Into<String>) -> Self {
        Self::Parse(msg.into())
    }

    /// Create an internal error
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }
}

/// Convert CoreError to a string
impl From<CoreError> for String {
    fn from(err: CoreError) -> String {
        err.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = CoreError::config("invalid setting");
        assert_eq!(err.to_string(), "Configuration error: invalid setting");
    }

    #[test]
    fn test_error_conversion() {
        let err = CoreError::config("invalid setting");
        let msg: String = err.into();
        assert!(msg.contains("Configuration error"));
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let core_err: CoreError = io_err.into();
        assert!(matches!(core_err, CoreError::Io(_)));
    }

    #[test]
    fn test_validation_error() {
        let err = CoreError::validation("field is required");
        assert_eq!(err.to_string(), "Validation error: field is required");
    }

    #[test]
    fn test_not_found_error() {
        let err = CoreError::not_found("Tool not found: Read");
        assert_eq!(err.to_string(), "Not found: Tool not found: Read");
    }

    #[test]
    fn test_internal_error() {
        let err = CoreError::internal("lock poisoned");
        assert_eq!(err.to_string(), "Internal error: lock poisoned");
    }
}
