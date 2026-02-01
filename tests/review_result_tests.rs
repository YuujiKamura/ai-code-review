//! Tests for ReviewResult and ReviewSummary

use ai_code_review::{ReviewResult, ReviewSeverity, ReviewSummary};
use std::path::PathBuf;

#[test]
fn test_review_result_ok() {
    let result = ReviewResult::new(
        PathBuf::from("test.rs"),
        "✓ 問題なし".to_string(),
    );

    assert_eq!(result.name, "test.rs");
    assert!(!result.has_issues);
    assert!(result.is_passed());
    assert!(!result.is_critical());
    assert_eq!(result.severity, ReviewSeverity::Ok);
}

#[test]
fn test_review_result_warning() {
    let result = ReviewResult::new(
        PathBuf::from("src/main.rs"),
        "⚠ 関数が長すぎます（80行）".to_string(),
    );

    assert_eq!(result.name, "main.rs");
    assert!(result.has_issues);
    assert!(!result.is_passed());
    assert!(!result.is_critical());
    assert_eq!(result.severity, ReviewSeverity::Warning);
}

#[test]
fn test_review_result_error() {
    // has_issues checks for ⚠, "warning", or "error"
    // severity checks for "critical" or 🚨
    let result = ReviewResult::new(
        PathBuf::from("config.rs"),
        "🚨 critical error: SQLインジェクション脆弱性".to_string(),
    );

    assert!(result.has_issues); // contains "error"
    assert!(!result.is_passed());
    assert!(result.is_critical());
    assert_eq!(result.severity, ReviewSeverity::Error);
}

#[test]
fn test_review_result_info() {
    let result = ReviewResult::new(
        PathBuf::from("lib.rs"),
        "💡 suggest: 変数名を改善できます".to_string(),
    );

    assert!(!result.has_issues);
    assert!(result.is_passed());
    assert_eq!(result.severity, ReviewSeverity::Info);
}

#[test]
fn test_review_result_with_content() {
    let result = ReviewResult::new(
        PathBuf::from("test.rs"),
        "✓ OK".to_string(),
    ).with_content("fn main() {}".to_string());

    assert_eq!(result.reviewed_content, Some("fn main() {}".to_string()));
}

#[test]
fn test_review_summary_empty() {
    let summary = ReviewSummary::new();

    assert_eq!(summary.total_files, 0);
    assert_eq!(summary.files_with_issues, 0);
    assert_eq!(summary.files_passed, 0);
    assert!(summary.all_passed());
}

#[test]
fn test_review_summary_all_passed() {
    let mut summary = ReviewSummary::new();

    summary.add(ReviewResult::new(
        PathBuf::from("a.rs"),
        "✓ OK".to_string(),
    ));
    summary.add(ReviewResult::new(
        PathBuf::from("b.rs"),
        "✓ 問題なし".to_string(),
    ));

    assert_eq!(summary.total_files, 2);
    assert_eq!(summary.files_passed, 2);
    assert_eq!(summary.files_with_issues, 0);
    assert!(summary.all_passed());
}

#[test]
fn test_review_summary_with_issues() {
    let mut summary = ReviewSummary::new();

    summary.add(ReviewResult::new(
        PathBuf::from("a.rs"),
        "✓ OK".to_string(),
    ));
    summary.add(ReviewResult::new(
        PathBuf::from("b.rs"),
        "⚠ warning: 問題あり".to_string(),
    ));
    summary.add(ReviewResult::new(
        PathBuf::from("c.rs"),
        "🚨 critical: 重大な問題".to_string(),
    ));

    assert_eq!(summary.total_files, 3);
    assert_eq!(summary.files_passed, 1);
    assert_eq!(summary.files_with_issues, 2);
    assert_eq!(summary.warning_count, 1);
    assert_eq!(summary.critical_count, 1);
    assert!(!summary.all_passed());
}

#[test]
fn test_review_result_serialization() {
    let result = ReviewResult::new(
        PathBuf::from("test.rs"),
        "✓ OK".to_string(),
    );

    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("test.rs"));
    assert!(json.contains("OK"));

    let deserialized: ReviewResult = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.name, result.name);
    assert_eq!(deserialized.review, result.review);
}
