//! Error types for ai-code-review

use std::path::PathBuf;
use thiserror::Error;

/// Result type for ai-code-review operations
pub type Result<T> = std::result::Result<T, CodeReviewError>;

/// Errors that can occur during code review operations
#[derive(Error, Debug)]
pub enum CodeReviewError {
    /// Path does not exist
    #[error("Path does not exist: {0}")]
    PathNotFound(PathBuf),

    /// Not a directory
    #[error("Path is not a directory: {0}")]
    NotADirectory(PathBuf),

    /// Watcher error
    #[error("Watcher error: {0}")]
    WatcherError(#[from] folder_watcher::WatcherError),

    /// AI analysis error
    #[error("AI analysis error: {0}")]
    AiError(#[from] cli_ai_analyzer::Error),

    /// IO error
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// Serialization error
    #[error("Serialization error: {0}")]
    SerdeError(#[from] serde_json::Error),

    /// Reviewer not running
    #[error("Code reviewer is not running")]
    NotRunning,

    /// Reviewer already running
    #[error("Code reviewer is already running")]
    AlreadyRunning,

    /// Lock error
    #[error("Lock error: {0}")]
    LockError(String),

    /// Git error
    #[error("Git error: {0}")]
    GitError(String),

    /// Parse error (tree-sitter)
    #[error("Parse error: {0}")]
    ParseError(String),
}
