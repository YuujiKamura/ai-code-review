//! Tests for git integration

use std::path::PathBuf;
use tempfile::tempdir;

// Note: These tests use internal functions that are not publicly exposed
// Testing git functionality through the public API

#[test]
fn test_reviewer_in_git_repo() {
    // The ai-code-review directory itself is a git repo
    let repo_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let reviewer = ai_code_review::CodeReviewer::new(&repo_path);
    assert!(reviewer.is_ok());
}

#[test]
fn test_reviewer_in_non_git_dir() {
    // Create a temp directory (not a git repo)
    let dir = tempdir().unwrap();

    let reviewer = ai_code_review::CodeReviewer::new(dir.path());
    assert!(reviewer.is_ok());
    // Should still work, just won't get git diffs
}

#[test]
fn test_review_file_in_git_repo() {
    // Can't test review_file without API keys, but can test path handling
    let repo_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let reviewer = ai_code_review::CodeReviewer::new(&repo_path).unwrap();

    // Verify path is set correctly
    assert_eq!(reviewer.path(), repo_path);
}
