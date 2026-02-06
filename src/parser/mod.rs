//! AST parsing with tree-sitter for import/export extraction

use std::fs;
use std::path::Path;

use crate::error::Result;

#[cfg(feature = "lang-python")]
pub(crate) mod python;
#[cfg(feature = "lang-rust")]
pub(crate) mod rust;
#[cfg(feature = "lang-typescript")]
pub(crate) mod typescript;

/// Information about an import statement
#[derive(Debug, Clone)]
pub struct ImportInfo {
    /// Module path (e.g., "crate::services::auth")
    pub module_path: String,
    /// Imported items (e.g., ["AuthService", "Token"])
    pub items: Vec<String>,
}

/// Analysis result for a single file
#[derive(Debug, Clone)]
pub struct FileAnalysis {
    /// Import statements found
    pub imports: Vec<ImportInfo>,
    /// Public exports (pub fn/struct/enum)
    /// Currently populated by parsers and used in tests; read access planned for future features.
    #[allow(dead_code)]
    pub exports: Vec<String>,
    /// Detected language
    /// Currently populated by parsers and used in tests; read access planned for future features.
    #[allow(dead_code)]
    pub language: String,
}

impl FileAnalysis {
    /// Create an empty analysis
    pub fn empty(language: &str) -> Self {
        Self {
            imports: Vec::new(),
            exports: Vec::new(),
            language: language.to_string(),
        }
    }
}

/// Analyze a source file to extract imports and exports
pub fn analyze_file(file_path: &Path) -> Result<FileAnalysis> {
    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    let source = fs::read_to_string(file_path)?;

    match ext {
        #[cfg(feature = "lang-rust")]
        "rs" => rust::analyze_rust(&source),
        #[cfg(feature = "lang-typescript")]
        "ts" | "tsx" => typescript::analyze_typescript(&source),
        #[cfg(feature = "lang-typescript")]
        "js" | "jsx" => typescript::analyze_typescript(&source),
        #[cfg(feature = "lang-python")]
        "py" => python::analyze_python(&source),
        _ => Ok(FileAnalysis::empty("unknown")),
    }
}
