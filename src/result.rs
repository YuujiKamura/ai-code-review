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

        let severity = Self::detect_severity(&review);
        let has_issues = matches!(severity, ReviewSeverity::Warning | ReviewSeverity::Error);
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

    /// Detect severity from review text.
    ///
    /// Detection strategy:
    /// 1. Check for emoji markers first (unambiguous, inserted by prompt instructions):
    ///    - \u{1F6A8} (siren) -> Error
    ///    - \u{26A0} (warning sign) -> Warning
    ///    - \u{1F4A1} (light bulb) -> Info
    ///    - \u{2713} (check mark) -> Ok
    /// 2. If no emoji found, fall back to word-boundary matching.
    /// 3. Priority: Error > Warning > Info > Ok (return highest severity found).
    fn detect_severity(review: &str) -> ReviewSeverity {
        // Phase 1: Emoji-based detection (unambiguous)
        let has_error_emoji = review.contains('\u{1F6A8}');
        let has_warning_emoji = review.contains('\u{26A0}');
        let has_info_emoji = review.contains('\u{1F4A1}');
        let has_ok_emoji = review.contains('\u{2713}');

        let has_any_emoji = has_error_emoji || has_warning_emoji || has_info_emoji || has_ok_emoji;

        if has_any_emoji {
            // Return highest severity found among emojis
            if has_error_emoji {
                return ReviewSeverity::Error;
            }
            if has_warning_emoji {
                return ReviewSeverity::Warning;
            }
            if has_info_emoji {
                return ReviewSeverity::Info;
            }
            return ReviewSeverity::Ok;
        }

        // Phase 2: Word-boundary matching fallback (no emojis found)
        let review_lower = review.to_lowercase();

        if Self::contains_word(&review_lower, "critical") || Self::contains_word(&review_lower, "error") {
            ReviewSeverity::Error
        } else if Self::contains_word(&review_lower, "warning") {
            ReviewSeverity::Warning
        } else if Self::contains_word(&review_lower, "suggestion") || Self::contains_word(&review_lower, "suggest") {
            ReviewSeverity::Info
        } else if Self::contains_word(&review_lower, "no issue") || Self::contains_word(&review_lower, "no issues") || Self::contains_word(&review_lower, "ok") || Self::contains_word(&review_lower, "lgtm") {
            ReviewSeverity::Ok
        } else {
            ReviewSeverity::Info
        }
    }

    /// Check if `haystack` contains `word` as a standalone word (not part of a larger word).
    ///
    /// A word boundary is defined as: start/end of string, or a non-alphanumeric character.
    fn contains_word(haystack: &str, word: &str) -> bool {
        let word_len = word.len();
        let hay_len = haystack.len();
        if word_len > hay_len {
            return false;
        }
        let mut start = 0;
        while let Some(pos) = haystack[start..].find(word) {
            let abs_pos = start + pos;
            let before_ok = abs_pos == 0 || !haystack.as_bytes()[abs_pos - 1].is_ascii_alphanumeric();
            let after_pos = abs_pos + word_len;
            let after_ok = after_pos >= hay_len || !haystack.as_bytes()[after_pos].is_ascii_alphanumeric();
            if before_ok && after_ok {
                return true;
            }
            // Advance past this match to avoid infinite loop
            start = abs_pos + 1;
        }
        false
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn severity(text: &str) -> ReviewSeverity {
        ReviewResult::detect_severity(text)
    }

    // --- Emoji-based detection tests ---

    #[test]
    fn emoji_siren_returns_error() {
        assert_eq!(severity("\u{1F6A8} Critical issue found"), ReviewSeverity::Error);
    }

    #[test]
    fn emoji_warning_returns_warning() {
        assert_eq!(severity("\u{26A0} Potential problem"), ReviewSeverity::Warning);
    }

    #[test]
    fn emoji_bulb_returns_info() {
        assert_eq!(severity("\u{1F4A1} Consider refactoring this"), ReviewSeverity::Info);
    }

    #[test]
    fn emoji_check_returns_ok() {
        assert_eq!(severity("\u{2713} Looks good"), ReviewSeverity::Ok);
    }

    #[test]
    fn emoji_priority_error_over_warning() {
        // Both siren and warning present -> Error wins
        assert_eq!(severity("\u{1F6A8} Critical \u{26A0} also warning"), ReviewSeverity::Error);
    }

    #[test]
    fn emoji_priority_warning_over_info() {
        assert_eq!(severity("\u{26A0} Warning \u{1F4A1} suggestion"), ReviewSeverity::Warning);
    }

    #[test]
    fn emoji_priority_info_over_ok() {
        assert_eq!(severity("\u{1F4A1} Suggestion \u{2713} but ok"), ReviewSeverity::Info);
    }

    #[test]
    fn emoji_ok_with_critical_text_still_ok() {
        // The key fix: emoji takes precedence, "critical" text is ignored when emoji present
        assert_eq!(severity("\u{2713} OK, no critical issues"), ReviewSeverity::Ok);
    }

    // --- Word-boundary fallback tests ---

    #[test]
    fn word_ok_not_matched_in_token() {
        // "ok" inside "token" should NOT match
        assert_ne!(severity("The token is valid"), ReviewSeverity::Ok);
    }

    #[test]
    fn word_ok_not_matched_in_book() {
        assert_ne!(severity("Looking at the book"), ReviewSeverity::Ok);
    }

    #[test]
    fn word_ok_standalone_matches() {
        assert_eq!(severity("Everything is OK"), ReviewSeverity::Ok);
    }

    #[test]
    fn word_ok_with_punctuation() {
        assert_eq!(severity("Code is ok."), ReviewSeverity::Ok);
    }

    #[test]
    fn word_warning_not_matched_in_forewarning() {
        assert_ne!(severity("This is a forewarning of issues"), ReviewSeverity::Warning);
    }

    #[test]
    fn word_warning_standalone() {
        assert_eq!(severity("Warning: unused variable"), ReviewSeverity::Warning);
    }

    #[test]
    fn word_suggest_standalone() {
        assert_eq!(severity("I suggest using a different approach"), ReviewSeverity::Info);
    }

    #[test]
    fn word_critical_standalone() {
        assert_eq!(severity("This is a critical bug"), ReviewSeverity::Error);
    }

    #[test]
    fn word_no_issue_matches_ok() {
        assert_eq!(severity("There is no issue with this code"), ReviewSeverity::Ok);
    }

    #[test]
    fn word_no_issues_matches_ok() {
        assert_eq!(severity("No issues found in the review"), ReviewSeverity::Ok);
    }

    #[test]
    fn word_lgtm_matches_ok() {
        assert_eq!(severity("LGTM"), ReviewSeverity::Ok);
    }

    #[test]
    fn fallback_no_keywords_returns_info() {
        assert_eq!(severity("The code does something"), ReviewSeverity::Info);
    }

    // --- has_issues derived from severity tests ---

    #[test]
    fn has_issues_true_for_error() {
        let result = ReviewResult::new(PathBuf::from("test.rs"), "\u{1F6A8} Critical".into());
        assert!(result.has_issues);
        assert_eq!(result.severity, ReviewSeverity::Error);
    }

    #[test]
    fn has_issues_true_for_warning() {
        let result = ReviewResult::new(PathBuf::from("test.rs"), "\u{26A0} Warning".into());
        assert!(result.has_issues);
        assert_eq!(result.severity, ReviewSeverity::Warning);
    }

    #[test]
    fn has_issues_false_for_info() {
        let result = ReviewResult::new(PathBuf::from("test.rs"), "\u{1F4A1} Suggestion".into());
        assert!(!result.has_issues);
        assert_eq!(result.severity, ReviewSeverity::Info);
    }

    #[test]
    fn has_issues_false_for_ok() {
        let result = ReviewResult::new(PathBuf::from("test.rs"), "\u{2713} All good".into());
        assert!(!result.has_issues);
        assert_eq!(result.severity, ReviewSeverity::Ok);
    }

    #[test]
    fn has_issues_consistent_with_severity_ok_text() {
        // Previously "ok" in "token" would cause has_issues=false via old logic
        // but severity might differ. Now both derive from the same source.
        let result = ReviewResult::new(PathBuf::from("test.rs"), "The token is valid".into());
        // No emoji, "token" does not contain standalone "ok" -> Info (default)
        assert!(!result.has_issues);
        assert_eq!(result.severity, ReviewSeverity::Info);
    }

    // --- contains_word tests ---

    #[test]
    fn contains_word_at_start() {
        assert!(ReviewResult::contains_word("ok then", "ok"));
    }

    #[test]
    fn contains_word_at_end() {
        assert!(ReviewResult::contains_word("it is ok", "ok"));
    }

    #[test]
    fn contains_word_in_middle() {
        assert!(ReviewResult::contains_word("it is ok here", "ok"));
    }

    #[test]
    fn contains_word_whole_string() {
        assert!(ReviewResult::contains_word("ok", "ok"));
    }

    #[test]
    fn contains_word_with_punctuation() {
        assert!(ReviewResult::contains_word("is ok.", "ok"));
        assert!(ReviewResult::contains_word("ok!", "ok"));
        assert!(ReviewResult::contains_word("(ok)", "ok"));
    }

    #[test]
    fn contains_word_rejects_substring() {
        assert!(!ReviewResult::contains_word("token", "ok"));
        assert!(!ReviewResult::contains_word("book", "ok"));
        assert!(!ReviewResult::contains_word("looking", "ok"));
        assert!(!ReviewResult::contains_word("forewarning", "warning"));
    }

    #[test]
    fn contains_word_multi_word_phrase() {
        assert!(ReviewResult::contains_word("there is no issue here", "no issue"));
        assert!(!ReviewResult::contains_word("there is no issued here", "no issue"));
    }

    // --- with_severity keeps has_issues in sync ---

    #[test]
    fn with_severity_updates_has_issues() {
        let result = ReviewResult::new(PathBuf::from("test.rs"), "\u{2713} OK".into())
            .with_severity(ReviewSeverity::Error);
        assert!(result.has_issues);
        assert_eq!(result.severity, ReviewSeverity::Error);
    }
}
