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
    // Get content (git diff or file content)
    let content = get_git_diff(path)
        .or_else(|| fs::read_to_string(path).ok())
        .ok_or_else(|| {
            CodeReviewError::IoError(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Cannot read file",
            ))
        })?;

    if content.trim().is_empty() {
        return Err(CodeReviewError::IoError(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "File content is empty",
        )));
    }

    // Build the prompt
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    // Gather context if enabled
    let prompt = if config.context_enabled {
        let base = base_path.unwrap_or_else(|| path.parent().unwrap_or(Path::new(".")));
        match gather_context(path, base, config.context_depth) {
            Ok(ctx) if !ctx.is_empty() => {
                let context_str = ctx.to_prompt_string();
                build_prompt_with_context(&config.prompt_template, &file_name, &content, &context_str)
            }
            _ => build_prompt(&config.prompt_template, &file_name, &content),
        }
    } else {
        build_prompt(&config.prompt_template, &file_name, &content)
    };

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
type ReviewCallback = Box<dyn Fn(ReviewResult) + Send + Sync + 'static>;

/// State for debouncing reviews
struct DebounceState {
    last_review: HashMap<PathBuf, Instant>,
}

/// State for logging
struct LoggingState {
    log_path: Option<PathBuf>,
}

/// A code reviewer that watches files and performs AI-powered reviews
pub struct CodeReviewer {
    /// Path to watch
    path: PathBuf,
    /// AI backend to use
    backend: Backend,
    /// Model to use (optional, uses backend default if not set)
    model: Option<String>,
    /// File extensions to watch
    extensions: Vec<String>,
    /// Custom prompt template
    prompt_template: String,
    /// Prompt type
    prompt_type: PromptType,
    /// Debounce duration in milliseconds
    debounce_ms: u64,
    /// Whether context gathering is enabled
    context_enabled: bool,
    /// Depth limit for context gathering
    context_depth: usize,
    /// Callback for review results
    on_review: Option<Arc<ReviewCallback>>,
    /// Internal watcher
    watcher: Option<FolderWatcher>,
    /// Running state
    running: Arc<AtomicBool>,
    /// State for debouncing
    debounce_state: Arc<Mutex<DebounceState>>,
    /// State for logging
    logging_state: Arc<Mutex<LoggingState>>,
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
            backend: Backend::default(),
            model: None,
            extensions: DEFAULT_EXTENSIONS.iter().map(|s| s.to_string()).collect(),
            prompt_template: DEFAULT_REVIEW_PROMPT.to_string(),
            prompt_type: PromptType::Default,
            debounce_ms: DEFAULT_DEBOUNCE_MS,
            context_enabled: false,
            context_depth: 50,
            on_review: None,
            watcher: None,
            running: Arc::new(AtomicBool::new(false)),
            debounce_state: Arc::new(Mutex::new(DebounceState {
                last_review: HashMap::new(),
            })),
            logging_state: Arc::new(Mutex::new(LoggingState {
                log_path: None,
            })),
        })
    }

    /// Set the AI backend to use
    pub fn with_backend(mut self, backend: Backend) -> Self {
        self.backend = backend;
        self
    }

    /// Set a specific model to use
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Set file extensions to watch
    pub fn with_extensions(mut self, exts: &[&str]) -> Self {
        self.extensions = exts.iter().map(|s| s.to_lowercase()).collect();
        self
    }

    /// Set a custom prompt template
    pub fn with_prompt(mut self, template: impl Into<String>) -> Self {
        self.prompt_template = template.into();
        self.prompt_type = PromptType::Custom;
        self
    }

    /// Set the prompt type
    pub fn with_prompt_type(mut self, prompt_type: PromptType) -> Self {
        self.prompt_type = prompt_type;
        if prompt_type != PromptType::Custom {
            self.prompt_template = prompt_type.template().to_string();
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
        self.context_enabled = enabled;
        self
    }

    /// Set the context gathering depth limit
    pub fn with_context_depth(mut self, depth: usize) -> Self {
        self.context_depth = depth;
        self
    }

    /// Set a log file path for review results
    pub fn with_log_file(self, path: impl Into<PathBuf>) -> Self {
        let log_path = path.into();
        self.logging_state
            .lock()
            .expect("Failed to acquire logging state lock")
            .log_path = Some(log_path);
        self
    }

    /// Set callback for when a review completes
    pub fn on_review<F>(mut self, callback: F) -> Self
    where
        F: Fn(ReviewResult) + Send + Sync + 'static,
    {
        self.on_review = Some(Arc::new(Box::new(callback)));
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
        let review_config = ReviewConfig {
            backend: self.backend,
            model: self.model.clone(),
            prompt_template: self.prompt_template.clone(),
            context_enabled: self.context_enabled,
            context_depth: self.context_depth,
        };
        let debounce_ms = self.debounce_ms;
        let base_path = self.path.clone();
        let on_review = self.on_review.clone();
        let debounce_state = self.debounce_state.clone();
        let logging_state = self.logging_state.clone();
        let running = self.running.clone();

        // Create the folder watcher
        let ext_refs: Vec<&str> = extensions.iter().map(|s| s.as_str()).collect();

        let watcher = FolderWatcher::new(&self.path)?
            .with_filter(&ext_refs)
            .on_modify(move |path| {
                if !check_debounce(path, &debounce_state, debounce_ms) {
                    return;
                }
                if !running.load(Ordering::SeqCst) {
                    return;
                }
                handle_file_change(path, &review_config, &base_path, &logging_state, &on_review);
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
        let config = ReviewConfig {
            backend: self.backend,
            model: self.model.clone(),
            prompt_template: self.prompt_template.clone(),
            context_enabled: self.context_enabled,
            context_depth: self.context_depth,
        };
        perform_review(path, &config, Some(&self.path))
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
    debounce_state: &Arc<Mutex<DebounceState>>,
    debounce_ms: u64,
) -> bool {
    let mut state_lock = match debounce_state.lock() {
        Ok(s) => s,
        Err(_) => return false,
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
    logging_state: &Arc<Mutex<LoggingState>>,
    on_review: &Option<Arc<ReviewCallback>>,
) {
    // Write to log if configured
    if let Ok(state_lock) = logging_state.lock() {
        if let Some(ref log_path) = state_lock.log_path {
            let _ = append_review_log(log_path, &result);
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
    config: &ReviewConfig,
    base_path: &Path,
    logging_state: &Arc<Mutex<LoggingState>>,
    on_review: &Option<Arc<ReviewCallback>>,
) {
    match perform_review(path, config, Some(base_path)) {
        Ok(result) => {
            process_review_result(result, logging_state, on_review);
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

        assert_eq!(reviewer.backend, Backend::Claude);
        assert_eq!(reviewer.extensions, vec!["rs", "py"]);
        assert_eq!(reviewer.debounce_ms, 1000);
    }
}
