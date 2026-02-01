//! Tests for CodeReviewer

use ai_code_review::{Backend, CodeReviewer, PromptType};
use std::path::Path;
use tempfile::tempdir;

#[test]
fn test_reviewer_creation() {
    let dir = tempdir().unwrap();
    let reviewer = CodeReviewer::new(dir.path());

    assert!(reviewer.is_ok());
    let reviewer = reviewer.unwrap();
    assert!(!reviewer.is_running());
    assert_eq!(reviewer.path(), dir.path());
}

#[test]
fn test_reviewer_nonexistent_path() {
    let reviewer = CodeReviewer::new(Path::new("/nonexistent/path/12345"));
    assert!(reviewer.is_err());
}

#[test]
fn test_reviewer_file_not_directory() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.txt");
    std::fs::write(&file_path, "test").unwrap();

    let reviewer = CodeReviewer::new(&file_path);
    assert!(reviewer.is_err());
}

#[test]
fn test_reviewer_builder_backend() {
    let dir = tempdir().unwrap();
    let _reviewer = CodeReviewer::new(dir.path())
        .unwrap()
        .with_backend(Backend::Claude);
}

#[test]
fn test_reviewer_builder_extensions() {
    let dir = tempdir().unwrap();
    let _reviewer = CodeReviewer::new(dir.path())
        .unwrap()
        .with_extensions(&["rs", "py", "ts"]);
}

#[test]
fn test_reviewer_builder_debounce() {
    let dir = tempdir().unwrap();
    let _reviewer = CodeReviewer::new(dir.path())
        .unwrap()
        .with_debounce(1000);
}

#[test]
fn test_reviewer_builder_prompt_type() {
    let dir = tempdir().unwrap();

    let _r1 = CodeReviewer::new(dir.path())
        .unwrap()
        .with_prompt_type(PromptType::Quick);

    let _r2 = CodeReviewer::new(dir.path())
        .unwrap()
        .with_prompt_type(PromptType::Security);

    let _r3 = CodeReviewer::new(dir.path())
        .unwrap()
        .with_prompt_type(PromptType::Architecture);
}

#[test]
fn test_reviewer_builder_custom_prompt() {
    let dir = tempdir().unwrap();
    let _reviewer = CodeReviewer::new(dir.path())
        .unwrap()
        .with_prompt("Custom prompt: {file_name}\n{content}");
}

#[test]
fn test_reviewer_builder_model() {
    let dir = tempdir().unwrap();
    let _reviewer = CodeReviewer::new(dir.path())
        .unwrap()
        .with_model("gemini-2.0-flash");
}

#[test]
fn test_reviewer_builder_log_file() {
    let dir = tempdir().unwrap();
    let log_path = dir.path().join("review.log");

    let _reviewer = CodeReviewer::new(dir.path())
        .unwrap()
        .with_log_file(&log_path);
}

#[test]
fn test_reviewer_builder_chain() {
    let dir = tempdir().unwrap();

    let reviewer = CodeReviewer::new(dir.path())
        .unwrap()
        .with_backend(Backend::Gemini)
        .with_extensions(&["rs"])
        .with_prompt_type(PromptType::Quick)
        .with_debounce(500)
        .on_review(|result| {
            println!("Reviewed: {}", result.name);
        });

    assert!(!reviewer.is_running());
}

#[test]
fn test_reviewer_stop_when_not_running() {
    let dir = tempdir().unwrap();
    let mut reviewer = CodeReviewer::new(dir.path()).unwrap();

    let result = reviewer.stop();
    assert!(result.is_err());
}
