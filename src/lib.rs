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

mod analyzer;
mod context;
mod error;
mod git;
mod modules;
mod parser;
mod prompt;
mod result;
mod reviewer;
pub mod shared_finder;
mod utils;

/// Re-export of `Backend` from `cli_ai_analyzer` for convenience.
///
/// This re-export allows users to configure the AI backend without needing
/// to add `cli_ai_analyzer` as a direct dependency. The available backends are:
/// - `Backend::Gemini` - Google's Gemini API
/// - `Backend::Claude` - Anthropic's Claude API
///
/// While this creates a coupling with `cli_ai_analyzer`, it significantly
/// improves the ergonomics for end users who can simply write:
/// ```rust,ignore
/// use ai_code_review::{CodeReviewer, Backend};
/// ```
pub use cli_ai_analyzer::Backend;
pub use context::{
    gather_context, gather_context_default, gather_raw_context, gather_requirements, ProjectContext,
    RawContext,
};
pub use modules::generate_module_tree;
pub use prompt::{
    build_analyze_prompt, build_discovery_prompt, build_find_shared_prompt, build_prompt,
    build_prompt_with_context, PromptType, ANALYZE_PROMPT, ARCHITECTURE_REVIEW_PROMPT,
    ARCHITECTURE_REVIEW_WITH_CONTEXT_PROMPT, DEFAULT_REVIEW_PROMPT, DISCOVERY_PROMPT,
    FIND_SHARED_PROMPT, QUICK_REVIEW_PROMPT, SECURITY_REVIEW_PROMPT,
};
pub use result::{ReviewResult, ReviewSeverity, ReviewSummary};
pub use reviewer::CodeReviewer;
pub use utils::fs::SOURCE_EXTENSIONS;

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
        // Use with_severity for explicit, non-fragile testing
        let result = ReviewResult::new(
            Path::new("test.rs").to_path_buf(),
            "No issues found".to_string(),
        )
        .with_severity(ReviewSeverity::Ok);

        assert_eq!(result.name, "test.rs");
        assert!(!result.has_issues);
        assert!(result.is_passed());
    }

    #[test]
    fn test_review_result_with_issues() {
        // Use with_severity for explicit, non-fragile testing
        let result = ReviewResult::new(
            Path::new("test.rs").to_path_buf(),
            "Function is too long".to_string(),
        )
        .with_severity(ReviewSeverity::Warning);

        assert!(result.has_issues);
        assert!(!result.is_passed());
        assert_eq!(result.severity, ReviewSeverity::Warning);
    }

    #[test]
    fn test_with_severity_updates_has_issues() {
        // Verify that with_severity correctly updates has_issues field
        let ok_result = ReviewResult::new(Path::new("a.rs").to_path_buf(), "text".to_string())
            .with_severity(ReviewSeverity::Ok);
        assert!(!ok_result.has_issues);

        let info_result = ReviewResult::new(Path::new("b.rs").to_path_buf(), "text".to_string())
            .with_severity(ReviewSeverity::Info);
        assert!(!info_result.has_issues);

        let warning_result = ReviewResult::new(Path::new("c.rs").to_path_buf(), "text".to_string())
            .with_severity(ReviewSeverity::Warning);
        assert!(warning_result.has_issues);

        let error_result = ReviewResult::new(Path::new("d.rs").to_path_buf(), "text".to_string())
            .with_severity(ReviewSeverity::Error);
        assert!(error_result.has_issues);
    }

    #[test]
    fn test_prompt_building() {
        let prompt = crate::prompt::build_prompt(crate::prompt::QUICK_REVIEW_PROMPT, "main.rs", "fn main() {}");
        assert!(prompt.contains("main.rs"));
        assert!(prompt.contains("fn main() {}"));
    }

    #[test]
    fn test_context_builder() {
        let dir = tempdir().unwrap();
        let reviewer = CodeReviewer::new(dir.path())
            .unwrap()
            .with_context(true)
            .with_context_depth(100);

        assert!(!reviewer.is_running());
    }
}
