//! Core CodeReviewer implementation

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use cli_ai_analyzer::{prompt as ai_prompt, AnalyzeOptions, Backend};
use folder_watcher::FolderWatcher;

use crate::context::gather_context;
use crate::error::{CodeReviewError, Result};
use crate::git::get_git_diff;
use crate::prompt::{build_prompt, build_prompt_with_context, PromptType, DEFAULT_REVIEW_PROMPT};
use crate::result::ReviewResult;

/// Build the review prompt for a file, handling context gathering and prompt construction.
///
/// This helper function encapsulates the prompt building logic:
/// 1. Extracts the file name from the path
/// 2. Gathers project context if enabled in config
/// 3. Builds the final prompt using the appropriate template
///
/// # Arguments
/// * `path` - Path to the file being reviewed
/// * `content` - The content to review (git diff or file content)
/// * `config` - Review configuration containing prompt template and context settings
/// * `base_path` - Optional base path for context gathering (defaults to file's parent)
///
/// # Returns
/// The fully constructed prompt string ready to send to the AI
fn build_review_prompt(
    path: &Path,
    content: &str,
    config: &ReviewConfig,
    base_path: Option<&Path>,
) -> String {
    // Extract file name
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    // Determine base path for context gathering
    let base = base_path.unwrap_or_else(|| path.parent().unwrap_or(Path::new(".")));

    // Gather context if enabled
    let context_str = config
        .context_enabled
        .then(|| gather_context(path, base, config.context_depth).ok())
        .flatten()
        .filter(|ctx| !ctx.is_empty())
        .map(|ctx| ctx.to_prompt_string());

    // Build prompt with or without context
    match context_str {
        Some(ctx) => build_prompt_with_context(&config.prompt_template, &file_name, content, &ctx),
        None => build_prompt(&config.prompt_template, &file_name, content),
    }
}

/// Configuration for review execution
#[derive(Clone)]
pub(crate) struct ReviewConfig {
    pub backend: Backend,
    pub model: Option<String>,
    pub prompt_template: String,
    pub context_enabled: bool,
    pub context_depth: usize,
}

/// Core review logic - performs AI-powered code review on a file
///
/// This is the shared implementation used by both `review_file` and the `on_modify` callback.
pub(crate) fn perform_review(
    path: &Path,
    config: &ReviewConfig,
    base_path: Option<&Path>,
) -> Result<ReviewResult> {
    // Get content (git diff or file content), tracking which source it came from
    let (content, is_diff) = match get_git_diff(path) {
        Some(diff) => (diff, true),
        None => {
            let full = fs::read_to_string(path).map_err(CodeReviewError::IoError)?;
            (full, false)
        }
    };

    if content.trim().is_empty() {
        return Err(CodeReviewError::IoError(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "File content is empty",
        )));
    }

    // When content is the full file (not a diff), prepend a note so the AI
    // knows it is reviewing the entire file rather than a set of changes.
    let labeled_content;
    let prompt_content: &str = if is_diff {
        &content
    } else {
        labeled_content = format!("（注: git diffが取得できないため、ファイル全体を表示しています。変更点ではなくファイル全体をレビューしてください）\n\n{}", content);
        &labeled_content
    };

    // Build the prompt using the helper function
    let prompt = build_review_prompt(path, prompt_content, config, base_path);

    // Run the review
    let options = if let Some(ref m) = config.model {
        AnalyzeOptions::with_model(m).with_backend(config.backend)
    } else {
        AnalyzeOptions::default().with_backend(config.backend)
    };

    let review = ai_prompt(&prompt, options)?;
    Ok(ReviewResult::new(path.to_path_buf(), review).with_content(content))
}

/// Default debounce duration in milliseconds
const DEFAULT_DEBOUNCE_MS: u64 = 500;

/// Default code extensions to watch
const DEFAULT_EXTENSIONS: &[&str] = &["rs", "ts", "tsx", "js", "jsx", "py", "go", "java", "cpp", "c", "h"];

/// Type alias for review callback
type ReviewCallback = dyn Fn(ReviewResult) + Send + Sync + 'static;

/// Consolidated shared state for debouncing and logging
struct SharedState {
    last_review: HashMap<PathBuf, Instant>,
    log_path: Option<PathBuf>,
}

/// A code reviewer that watches files and performs AI-powered reviews
pub struct CodeReviewer {
    /// Path to watch
    path: PathBuf,
    /// Review configuration (backend, model, prompt, context settings)
    config: Arc<ReviewConfig>,
    /// File extensions to watch
    extensions: Vec<String>,
    /// Debounce duration in milliseconds
    debounce_ms: u64,
    /// Callback for review results
    on_review: Option<Arc<ReviewCallback>>,
    /// Internal watcher
    watcher: Option<FolderWatcher>,
    /// Running state
    running: Arc<AtomicBool>,
    /// Consolidated shared state for debouncing and logging
    shared_state: Arc<Mutex<SharedState>>,
}

impl CodeReviewer {
    /// Create a new code reviewer for the specified path
    pub fn new(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Err(CodeReviewError::PathNotFound(path.to_path_buf()));
        }
        if !path.is_dir() {
            return Err(CodeReviewError::NotADirectory(path.to_path_buf()));
        }

        Ok(Self {
            path: path.to_path_buf(),
            config: Arc::new(ReviewConfig {
                backend: Backend::default(),
                model: None,
                prompt_template: DEFAULT_REVIEW_PROMPT.to_string(),
                context_enabled: false,
                context_depth: 50,
            }),
            extensions: DEFAULT_EXTENSIONS.iter().map(|s| s.to_string()).collect(),
            debounce_ms: DEFAULT_DEBOUNCE_MS,
            on_review: None,
            watcher: None,
            running: Arc::new(AtomicBool::new(false)),
            shared_state: Arc::new(Mutex::new(SharedState {
                last_review: HashMap::new(),
                log_path: None,
            })),
        })
    }

    /// Set the AI backend to use
    pub fn with_backend(mut self, backend: Backend) -> Self {
        Arc::make_mut(&mut self.config).backend = backend;
        self
    }

    /// Set a specific model to use
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        Arc::make_mut(&mut self.config).model = Some(model.into());
        self
    }

    /// Set file extensions to watch
    pub fn with_extensions(mut self, exts: &[&str]) -> Self {
        self.extensions = exts.iter().map(|s| s.to_lowercase()).collect();
        self
    }

    /// Set a custom prompt template
    pub fn with_prompt(mut self, template: impl Into<String>) -> Self {
        Arc::make_mut(&mut self.config).prompt_template = template.into();
        self
    }

    /// Set the prompt type
    pub fn with_prompt_type(mut self, prompt_type: PromptType) -> Self {
        if prompt_type != PromptType::Custom {
            Arc::make_mut(&mut self.config).prompt_template = prompt_type.template().to_string();
        }
        self
    }

    /// Set debounce duration in milliseconds
    pub fn with_debounce(mut self, ms: u64) -> Self {
        self.debounce_ms = ms;
        self
    }

    /// Enable or disable context gathering
    pub fn with_context(mut self, enabled: bool) -> Self {
        Arc::make_mut(&mut self.config).context_enabled = enabled;
        self
    }

    /// Set the context gathering depth limit
    pub fn with_context_depth(mut self, depth: usize) -> Self {
        Arc::make_mut(&mut self.config).context_depth = depth;
        self
    }

    /// Set a log file path for review results
    pub fn with_log_file(self, path: impl Into<PathBuf>) -> Self {
        let log_path = path.into();
        self.shared_state
            .lock()
            .expect("shared state lock poisoned during construction")
            .log_path = Some(log_path);
        self
    }

    /// Set callback for when a review completes
    pub fn on_review<F>(mut self, callback: F) -> Self
    where
        F: Fn(ReviewResult) + Send + Sync + 'static,
    {
        self.on_review = Some(Arc::new(callback));
        self
    }

    /// Check if the reviewer is running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Get the path being watched
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Start the code reviewer
    pub fn start(&mut self) -> Result<()> {
        if self.running.load(Ordering::SeqCst) {
            return Err(CodeReviewError::AlreadyRunning);
        }

        let extensions = self.extensions.clone();
        let review_config = Arc::clone(&self.config);
        let debounce_ms = self.debounce_ms;
        let base_path = self.path.clone();
        let on_review = self.on_review.clone();
        let shared_state = Arc::clone(&self.shared_state);
        let running = Arc::clone(&self.running);

        // Create the folder watcher
        let ext_refs: Vec<&str> = extensions.iter().map(|s| s.as_str()).collect();

        let watcher = FolderWatcher::new(&self.path)?
            .with_filter(&ext_refs)
            .on_modify(move |path| {
                if !check_debounce(path, &shared_state, debounce_ms) {
                    return;
                }
                if !running.load(Ordering::SeqCst) {
                    return;
                }
                handle_file_change(path, &review_config, &base_path, &shared_state, &on_review);
            });

        watcher.start()?;
        self.watcher = Some(watcher);
        self.running.store(true, Ordering::SeqCst);

        log::info!("Code reviewer started for {:?}", self.path);
        Ok(())
    }

    /// Stop the code reviewer
    pub fn stop(&mut self) -> Result<()> {
        if !self.running.load(Ordering::SeqCst) {
            return Err(CodeReviewError::NotRunning);
        }

        if let Some(watcher) = self.watcher.take() {
            watcher.stop()?;
        }

        self.running.store(false, Ordering::SeqCst);
        log::info!("Code reviewer stopped");
        Ok(())
    }

    /// Review a single file immediately (without watching)
    pub fn review_file(&self, path: &Path) -> Result<ReviewResult> {
        perform_review(path, &self.config, Some(&self.path))
    }
}

impl Drop for CodeReviewer {
    fn drop(&mut self) {
        if self.running.load(Ordering::SeqCst) {
            let _ = self.stop();
        }
    }
}

/// Append a review result to the log file (JSON Lines format)
fn append_review_log(log_path: &Path, result: &ReviewResult) -> Result<()> {
    let json = serde_json::to_string(result)?;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)?;
    writeln!(file, "{}", json)?;
    Ok(())
}

/// Check if the file should be reviewed based on debounce timing
///
/// Returns `true` if review should proceed, `false` if debounced.
fn check_debounce(
    path: &Path,
    shared_state: &Arc<Mutex<SharedState>>,
    debounce_ms: u64,
) -> bool {
    let mut state_lock = match shared_state.lock() {
        Ok(s) => s,
        Err(e) => {
            log::error!("Failed to acquire shared state lock: {}", e);
            return false;
        }
    };
    let now = Instant::now();
    if let Some(last) = state_lock.last_review.get(path) {
        if now.duration_since(*last).as_millis() < debounce_ms as u128 {
            return false;
        }
    }
    state_lock.last_review.insert(path.to_path_buf(), now);
    true
}

/// Process review result - log to file and invoke callback
fn process_review_result(
    result: ReviewResult,
    shared_state: &Arc<Mutex<SharedState>>,
    on_review: &Option<Arc<ReviewCallback>>,
) {
    // Write to log if configured
    match shared_state.lock() {
        Ok(state_lock) => {
            if let Some(ref log_path) = state_lock.log_path {
                let _ = append_review_log(log_path, &result);
            }
        }
        Err(e) => {
            log::error!("Failed to acquire shared state lock: {}", e);
        }
    }

    // Call the callback
    if let Some(ref callback) = on_review {
        callback(result);
    }
}

/// Handle file modification event - perform review and process result
fn handle_file_change(
    path: &Path,
    config: &Arc<ReviewConfig>,
    base_path: &Path,
    shared_state: &Arc<Mutex<SharedState>>,
    on_review: &Option<Arc<ReviewCallback>>,
) {
    match perform_review(path, config.as_ref(), Some(base_path)) {
        Ok(result) => {
            process_review_result(result, shared_state, on_review);
        }
        Err(e) => {
            log::error!("Review error for {:?}: {}", path, e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_reviewer_creation() {
        let dir = tempdir().unwrap();
        let reviewer = CodeReviewer::new(dir.path());
        assert!(reviewer.is_ok());
    }

    #[test]
    fn test_reviewer_nonexistent_path() {
        let reviewer = CodeReviewer::new(Path::new("/nonexistent/path"));
        assert!(reviewer.is_err());
    }

    #[test]
    fn test_reviewer_builder() {
        let dir = tempdir().unwrap();
        let reviewer = CodeReviewer::new(dir.path())
            .unwrap()
            .with_backend(Backend::Claude)
            .with_extensions(&["rs", "py"])
            .with_debounce(1000)
            .with_prompt_type(PromptType::Quick);

        assert_eq!(reviewer.config.backend, Backend::Claude);
        assert_eq!(reviewer.extensions, vec!["rs", "py"]);
        assert_eq!(reviewer.debounce_ms, 1000);
    }

    #[test]
    fn test_build_review_prompt_without_context() {
        let config = ReviewConfig {
            backend: Backend::default(),
            model: None,
            prompt_template: "Review {file_name}: {content}".to_string(),
            context_enabled: false,
            context_depth: 50,
        };
        let path = Path::new("/test/example.rs");
        let content = "fn main() {}";

        let prompt = build_review_prompt(path, content, &config, None);

        assert!(prompt.contains("example.rs"));
        assert!(prompt.contains("fn main() {}"));
    }

    #[test]
    fn test_build_review_prompt_unknown_filename() {
        let config = ReviewConfig {
            backend: Backend::default(),
            model: None,
            prompt_template: "Review {file_name}: {content}".to_string(),
            context_enabled: false,
            context_depth: 50,
        };
        // Path with no file name component
        let path = Path::new("/");
        let content = "test content";

        let prompt = build_review_prompt(path, content, &config, None);

        assert!(prompt.contains("unknown"));
    }
}
