//! Review result structures

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Result of a code review
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewResult {
    /// Path to the reviewed file
    pub path: PathBuf,

    /// File name
    pub name: String,

    /// The review content from AI
    pub review: String,

    /// Timestamp of the review
    pub timestamp: String,

    /// Whether issues were found
    pub has_issues: bool,

    /// Severity level (info, warning, error)
    pub severity: ReviewSeverity,

    /// The diff or content that was reviewed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reviewed_content: Option<String>,
}

/// Severity level of review findings
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ReviewSeverity {
    /// No issues found
    #[default]
    Ok,
    /// Informational suggestions
    Info,
    /// Warnings that should be addressed
    Warning,
    /// Critical issues that must be fixed
    Error,
}

impl ReviewResult {
    /// Create a new review result
    pub fn new(path: PathBuf, review: String) -> Self {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let has_issues = review.contains('\u{26A0}') || review.contains("warning") || review.contains("error");
        let severity = Self::detect_severity(&review);
        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

        Self {
            path,
            name,
            review,
            timestamp,
            has_issues,
            severity,
            reviewed_content: None,
        }
    }

    /// Create a result with content
    pub fn with_content(mut self, content: String) -> Self {
        self.reviewed_content = Some(content);
        self
    }

    /// Create a result with explicit severity (for testing or manual override)
    ///
    /// This method allows setting the severity directly, bypassing automatic detection.
    /// Useful for testing or when the severity is known from external sources.
    pub fn with_severity(mut self, severity: ReviewSeverity) -> Self {
        self.severity = severity;
        self.has_issues = matches!(severity, ReviewSeverity::Warning | ReviewSeverity::Error);
        self
    }

    /// Detect severity from review text
    fn detect_severity(review: &str) -> ReviewSeverity {
        let review_lower = review.to_lowercase();

        if review_lower.contains("critical") || review_lower.contains("\u{1F6A8}") {
            ReviewSeverity::Error
        } else if review_lower.contains('\u{26A0}') || review_lower.contains("warning") {
            ReviewSeverity::Warning
        } else if review_lower.contains('\u{1F4A1}') || review_lower.contains("suggest") {
            ReviewSeverity::Info
        } else if review_lower.contains('\u{2713}') || review_lower.contains("ok") || review_lower.contains("no issue") {
            ReviewSeverity::Ok
        } else {
            ReviewSeverity::Info
        }
    }

    /// Check if the review found critical issues
    pub fn is_critical(&self) -> bool {
        self.severity == ReviewSeverity::Error
    }

    /// Check if the review passed (no warnings or errors)
    pub fn is_passed(&self) -> bool {
        matches!(self.severity, ReviewSeverity::Ok | ReviewSeverity::Info)
    }
}

/// Summary of multiple reviews
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReviewSummary {
    /// Total files reviewed
    pub total_files: usize,

    /// Files with issues
    pub files_with_issues: usize,

    /// Files passed
    pub files_passed: usize,

    /// Critical issues count
    pub critical_count: usize,

    /// Warning count
    pub warning_count: usize,

    /// Individual results
    pub results: Vec<ReviewResult>,
}

impl ReviewSummary {
    /// Create a new summary
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a result to the summary
    pub fn add(&mut self, result: ReviewResult) {
        self.total_files += 1;

        match result.severity {
            ReviewSeverity::Error => {
                self.files_with_issues += 1;
                self.critical_count += 1;
            }
            ReviewSeverity::Warning => {
                self.files_with_issues += 1;
                self.warning_count += 1;
            }
            _ => {
                self.files_passed += 1;
            }
        }

        self.results.push(result);
    }

    /// Check if all files passed
    pub fn all_passed(&self) -> bool {
        self.files_with_issues == 0
    }
}
