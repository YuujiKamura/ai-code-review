//! Dependency analysis and file system exploration
//!
//! This module handles cross-file analysis including finding
//! files that import a given file.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::parser::{analyze_file, FileAnalysis};
use crate::utils::fs::walk_source_files;

/// Check if an import path matches the target module name,
/// using the appropriate separator for each language.
///
/// - Rust (`.rs`): `::` separator
/// - Python (`.py`): `.` separator
/// - JS/TS and others: `/` separator
///
/// # Arguments
/// * `import_path` - The import path string (e.g. `crate::target` or `os.path`)
/// * `target_module` - The module name to search for
/// * `file_ext` - The file extension without a leading dot (e.g. `"rs"`, `"py"`)
///
/// # Returns
/// `true` if any segment of the import path equals `target_module`
fn path_matches_import(import_path: &str, target_module: &str, file_ext: &str) -> bool {
    let separator = match file_ext {
        "rs" => "::",
        "py" => ".",
        _ => "/", // JS/TS and others
    };
    let parts: Vec<&str> = import_path.split(separator).collect();
    parts.iter().any(|&p| p == target_module)
}

/// Find files that import the given file
///
/// Walks the directory tree starting from `base_path` and finds
/// all files that contain imports matching the target file name.
/// Uses an internal cache to avoid re-parsing files.
///
/// # Arguments
/// * `file_path` - The file to search for importers of
/// * `base_path` - The root directory to search in
///
/// # Returns
/// A vector of file paths that import the target file
pub fn find_importers(file_path: &Path, base_path: &Path) -> Vec<String> {
    let target_name = file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    if target_name.is_empty() {
        return Vec::new();
    }

    let mut cache = HashMap::new();
    find_importers_inner(file_path, base_path, target_name, &mut cache)
}

/// Find files that import the given file, with an externally provided cache.
///
/// This allows callers to share a parse cache across multiple calls.
///
/// # Arguments
/// * `file_path` - The file to search for importers of
/// * `base_path` - The root directory to search in
/// * `cache` - A mutable reference to a cache of previously parsed files
///
/// # Returns
/// A vector of file paths that import the target file
#[allow(dead_code)]
pub fn find_importers_cached(
    file_path: &Path,
    base_path: &Path,
    cache: &mut HashMap<PathBuf, FileAnalysis>,
) -> Vec<String> {
    let target_name = file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    if target_name.is_empty() {
        return Vec::new();
    }

    find_importers_inner(file_path, base_path, target_name, cache)
}

/// All source file extensions we want to scan for imports
const IMPORT_SCAN_EXTENSIONS: &[&str] = &[
    "rs", "ts", "tsx", "js", "jsx", "py", "go", "java", "cpp", "c", "h", "hpp", "cs", "rb",
    "swift", "kt",
];

fn find_importers_inner(
    file_path: &Path,
    base_path: &Path,
    target_name: &str,
    cache: &mut HashMap<PathBuf, FileAnalysis>,
) -> Vec<String> {
    let mut importers = Vec::new();

    let source_files = walk_source_files(base_path, IMPORT_SCAN_EXTENSIONS);

    for path in source_files {
        // Skip the target file itself
        if path == file_path {
            continue;
        }

        // Determine file extension for separator selection
        let file_ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        // Use cached analysis if available, otherwise parse and cache
        let analysis = if let Some(cached) = cache.get(&path) {
            Some(cached)
        } else if let Ok(parsed) = analyze_file(&path) {
            cache.insert(path.clone(), parsed);
            cache.get(&path)
        } else {
            None
        };

        if let Some(analysis) = analysis {
            for import in &analysis.imports {
                if path_matches_import(&import.module_path, target_name, file_ext)
                    || import.items.iter().any(|i| i == target_name)
                {
                    if let Some(p) = path.to_str() {
                        importers.push(p.to_string());
                    }
                    break;
                }
            }
        }
    }

    importers
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn test_find_importers_empty_target() {
        let dir = tempdir().unwrap();
        let file_path = PathBuf::from("");
        let result = find_importers(&file_path, dir.path());
        assert!(result.is_empty());
    }

    #[test]
    fn test_find_importers_nonexistent_dir() {
        let file_path = PathBuf::from("test.rs");
        let base_path = PathBuf::from("/nonexistent/path");
        let result = find_importers(&file_path, &base_path);
        assert!(result.is_empty());
    }

    #[test]
    fn test_find_importers_cached_populates_cache() {
        let dir = tempdir().unwrap();
        let file_path = PathBuf::from("target.rs");
        let mut cache = HashMap::new();

        // Call with an empty directory — cache should remain empty
        // since there are no source files to parse
        let result = find_importers_cached(&file_path, dir.path(), &mut cache);
        assert!(result.is_empty());
        assert!(cache.is_empty());
    }

    #[test]
    fn test_find_importers_cached_reuses_cache() {
        let dir = tempdir().unwrap();
        let file_path = PathBuf::from("target.rs");

        // Pre-populate the cache with a synthetic FileAnalysis
        let mut cache = HashMap::new();
        let synthetic_path = dir.path().join("other.rs");
        // Create the file so it's found during directory walk
        std::fs::write(&synthetic_path, "// empty").unwrap();

        use crate::parser::{FileAnalysis, ImportInfo};
        let analysis = FileAnalysis {
            imports: vec![ImportInfo {
                module_path: "crate::target".to_string(),
                items: vec![],
            }],
            exports: vec![],
            language: "rust".to_string(),
        };
        cache.insert(synthetic_path.clone(), analysis);

        let result = find_importers_cached(&file_path, dir.path(), &mut cache);
        // The cached analysis contains an import of "target", so it should match
        assert_eq!(result.len(), 1);
        assert!(result[0].contains("other.rs"));
    }

    #[test]
    fn test_path_matches_import_rust() {
        assert!(path_matches_import("crate::target", "target", "rs"));
        assert!(path_matches_import("crate::foo::bar", "foo", "rs"));
        assert!(!path_matches_import("crate::foo::bar", "baz", "rs"));
    }

    #[test]
    fn test_path_matches_import_python() {
        assert!(path_matches_import("os.path", "os", "py"));
        assert!(path_matches_import("os.path", "path", "py"));
        assert!(!path_matches_import("os.path", "sys", "py"));
    }

    #[test]
    fn test_path_matches_import_typescript() {
        assert!(path_matches_import("./utils/helper", "helper", "ts"));
        assert!(path_matches_import("../components/Button", "Button", "tsx"));
        assert!(!path_matches_import("./utils/helper", "other", "ts"));
    }

    #[test]
    fn test_path_matches_import_js() {
        assert!(path_matches_import("./lib/config", "config", "js"));
        assert!(path_matches_import("./lib/config", "config", "jsx"));
    }
}
