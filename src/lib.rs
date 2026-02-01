//! # ai-code-review
//!
//! AI-powered code review library with file watching support.
//!
//! This crate provides automatic code review functionality by:
//! - Watching directories for file changes
//! - Getting git diffs for changed files
//! - Sending code to AI backends (Gemini, Claude) for review
//! - Returning structured review results
//!
//! ## Features
//!
//! - **Multiple AI Backends**: Support for Gemini and Claude
//! - **File Watching**: Automatic review on file changes
//! - **Git Integration**: Reviews git diffs when available
//! - **Customizable Prompts**: Japanese prompts for architecture, security, quick reviews
//! - **Debouncing**: Prevents excessive API calls on rapid saves
//!
//! ## Example
//!
//! ```rust,no_run
//! use ai_code_review::{CodeReviewer, Backend, PromptType};
//! use std::path::Path;
//!
//! // Create a reviewer for a directory
//! let mut reviewer = CodeReviewer::new(Path::new("/path/to/project"))
//!     .unwrap()
//!     .with_backend(Backend::Gemini)
//!     .with_extensions(&["rs", "ts", "py"])
//!     .with_prompt_type(PromptType::Default)
//!     .on_review(|result| {
//!         println!("Review for {}: {}", result.name, result.review);
//!         if result.has_issues {
//!             println!("Issues found!");
//!         }
//!     });
//!
//! // Start watching
//! reviewer.start().unwrap();
//!
//! // ... do other work ...
//!
//! // Stop when done
//! reviewer.stop().unwrap();
//! ```
//!
//! ## One-shot Review
//!
//! ```rust,no_run
//! use ai_code_review::{CodeReviewer, Backend};
//! use std::path::Path;
//!
//! let reviewer = CodeReviewer::new(Path::new("."))
//!     .unwrap()
//!     .with_backend(Backend::Claude);
//!
//! let result = reviewer.review_file(Path::new("src/main.rs")).unwrap();
//! println!("{}", result.review);
//! ```

mod error;
mod git;
mod prompt;
mod result;
mod reviewer;

pub use cli_ai_analyzer::Backend;
pub use error::{CodeReviewError, Result};
pub use prompt::{
    build_prompt, PromptType, ARCHITECTURE_REVIEW_PROMPT, DEFAULT_REVIEW_PROMPT,
    QUICK_REVIEW_PROMPT, SECURITY_REVIEW_PROMPT,
};
pub use result::{ReviewResult, ReviewSeverity, ReviewSummary};
pub use reviewer::CodeReviewer;

/// Re-export folder-watcher types that might be useful
pub use folder_watcher::{FolderWatcher, WatcherBuilder};

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use tempfile::tempdir;

    #[test]
    fn test_public_api() {
        let dir = tempdir().unwrap();
        let reviewer = CodeReviewer::new(dir.path())
            .unwrap()
            .with_backend(Backend::Gemini)
            .with_extensions(&["rs"]);

        assert!(!reviewer.is_running());
        assert_eq!(reviewer.path(), dir.path());
    }

    #[test]
    fn test_review_result() {
        let result = ReviewResult::new(
            Path::new("test.rs").to_path_buf(),
            "✓ 問題なし".to_string(),
        );

        assert_eq!(result.name, "test.rs");
        assert!(!result.has_issues);
        assert!(result.is_passed());
    }

    #[test]
    fn test_review_result_with_issues() {
        let result = ReviewResult::new(
            Path::new("test.rs").to_path_buf(),
            "⚠ 関数が長すぎます".to_string(),
        );

        assert!(result.has_issues);
        assert!(!result.is_passed());
        assert_eq!(result.severity, ReviewSeverity::Warning);
    }

    #[test]
    fn test_prompt_building() {
        let prompt = build_prompt(QUICK_REVIEW_PROMPT, "main.rs", "fn main() {}");
        assert!(prompt.contains("main.rs"));
        assert!(prompt.contains("fn main() {}"));
    }
}
