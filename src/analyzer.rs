//! Dependency analysis and file system exploration
//!
//! This module handles cross-file analysis including finding
//! files that import a given file.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::parser::{analyze_file, FileAnalysis};
use crate::utils::fs::{is_source_file, should_skip_dir};

/// Find files that import the given file
///
/// Walks the directory tree starting from `base_path` and finds
/// all files that contain imports matching the target file name.
/// Uses an internal cache to avoid re-parsing files during the
/// recursive directory walk.
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

fn find_importers_inner(
    file_path: &Path,
    base_path: &Path,
    target_name: &str,
    cache: &mut HashMap<PathBuf, FileAnalysis>,
) -> Vec<String> {
    let mut importers = Vec::new();

    // Walk the base_path and find files that might import our target
    if let Ok(entries) = fs::read_dir(base_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            if path.is_file() && path != file_path && is_source_file(&path) {
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
                        if import.module_path.split("::").any(|seg| seg == target_name)
                            || import.items.iter().any(|i| i == target_name)
                        {
                            if let Some(p) = path.to_str() {
                                importers.push(p.to_string());
                            }
                            break;
                        }
                    }
                }
            } else if path.is_dir() && !should_skip_dir(&name_str) {
                // Recurse into subdirectories, passing the cache and target_name through
                importers.extend(find_importers_inner(file_path, &path, target_name, cache));
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
}
