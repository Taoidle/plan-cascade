//! Error Handling
//!
//! Unified error types for the application.
//! Uses thiserror for ergonomic error definitions.

use thiserror::Error;

/// Application-wide error type
#[derive(Error, Debug)]
pub enum AppError {
    /// Database errors
    #[error("Database error: {0}")]
    Database(String),

    /// SQLite errors (auto-converted from rusqlite::Error)
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    /// Keyring/secret storage errors
    #[error("Keyring error: {0}")]
    Keyring(String),

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

    /// Generic internal errors
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Result type alias for application errors
pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    /// Create a database error
    pub fn database(msg: impl Into<String>) -> Self {
        Self::Database(msg.into())
    }

    /// Create a keyring error
    pub fn keyring(msg: impl Into<String>) -> Self {
        Self::Keyring(msg.into())
    }

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

    /// Create an internal error
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }
}

/// Convert AppError to a string suitable for Tauri command responses
impl From<AppError> for String {
    fn from(err: AppError) -> String {
        err.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = AppError::database("connection failed");
        assert_eq!(err.to_string(), "Database error: connection failed");
    }

    #[test]
    fn test_error_conversion() {
        let err = AppError::config("invalid setting");
        let msg: String = err.into();
        assert!(msg.contains("Configuration error"));
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let app_err: AppError = io_err.into();
        assert!(matches!(app_err, AppError::Io(_)));
    }
}
