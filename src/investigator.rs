//! Deep architectural investigation against reference codebases

use std::fs;
use std::path::{Path, PathBuf};
use cli_ai_analyzer::{analyze, AnalyzeOptions, Backend};
use crate::error::Result;
use walkdir::WalkDir;
use std::sync::{Mutex, OnceLock};

static SNIPPET_CACHE: OnceLock<Mutex<String>> = OnceLock::new();

pub struct Investigator {
    references: Vec<PathBuf>,
    backend: Backend,
    model: Option<String>,
    keywords: Vec<String>,
}

impl Investigator {
    pub fn new() -> Self {
        Self {
            references: Vec::new(),
            backend: Backend::default(),
            model: None,
            keywords: vec![
                "SetSwapChain".to_string(),
                "ISwapChainPanelNative".to_string(),
                "get_ActualWidth".to_string(),
                "put_RequestedTheme".to_string(),
                "HRESULT".to_string(),
                "VTable".to_string(),
            ],
        }
    }

    pub fn with_reference(mut self, path: impl Into<PathBuf>) -> Self {
        self.references.push(path.into());
        self
    }

    pub fn with_backend(mut self, backend: Backend) -> Self {
        self.backend = backend;
        self
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    fn gather_snippets(&self) -> String {
        let cache = SNIPPET_CACHE.get_or_init(|| Mutex::new(String::new()));
        let mut locked_cache = cache.lock().unwrap();
        
        if !locked_cache.is_empty() {
            return locked_cache.clone();
        }

        let mut snippets = String::new();
        for ref_path in &self.references {
            if !ref_path.exists() { continue; }
            
            let walker = WalkDir::new(ref_path).max_depth(5).into_iter();
            for entry_res in walker {
                let entry = match entry_res {
                    Ok(e) => e,
                    Err(_) => continue,
                };
                let path = entry.path();
                if path.is_file() && (path.extension().map_or(false, |ext| ext == "cpp" || ext == "h" || ext == "rs" || ext == "zig")) {
                    if let Ok(content) = fs::read_to_string(path) {
                        for kw in &self.keywords {
                            if content.contains(kw) {
                                snippets.push_str(&format!("\n--- Snippet from {:?} (keyword: {}) ---\n", path, kw));
                                if let Some(pos) = content.find(kw) {
                                    let start = if pos > 500 { pos - 500 } else { 0 };
                                    let end = if pos + 500 < content.len() { pos + 500 } else { content.len() };
                                    snippets.push_str(&content[start..end]);
                                    snippets.push_str("\n");
                                }
                                break;
                            }
                        }
                    }
                }
                if snippets.len() > 20000 { break; } 
            }
        }
        *locked_cache = snippets.clone();
        snippets
    }

    pub fn investigate(&self, target_file: &Path, error_log: &str) -> Result<String> {
        let target_content = fs::read_to_string(target_file)?;
        let target_name = target_file.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();

        let ref_context = self.gather_snippets();

        let prompt = format!(
            r#"ARCHITECTURAL INVESTIGATION & FIX GENERATION
You are an expert Windows System Engineer specializing in WinRT, DirectX, and Zig.

TARGET FILE ({}):
```zig
{}
```

BUILD ERRORS:
```
{}
```

REFERENCE SNIPPETS FROM ESTABLISHED CODEBASES:
{}

TASK:
1. Compare target logic with references.
2. Fix `emit.zig` so generated code compiles and matches Ghostty expectations (naming, return types).
3. Output ONLY the raw `emit.zig` content.
"#,
            target_name, target_content, error_log, ref_context
        );

        let options = if let Some(ref m) = self.model {
            AnalyzeOptions::with_model(m).with_backend(self.backend)
        } else {
            AnalyzeOptions::default().with_backend(self.backend)
        };

        let res = analyze(&prompt, &Vec::<PathBuf>::new(), options)
            .map_err(|e| crate::error::CodeReviewError::IoError(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;
        Ok(res)
    }
}
