//! Dependency analysis and file system exploration
//!
//! This module handles cross-file analysis including finding
//! files that import a given file.

use std::fs;
use std::path::Path;

use crate::parser::analyze_file;
use crate::utils::fs::{is_source_file, should_skip_dir};

/// Find files that import the given file
///
/// Walks the directory tree starting from `base_path` and finds
/// all files that contain imports matching the target file name.
///
/// # Arguments
/// * `file_path` - The file to search for importers of
/// * `base_path` - The root directory to search in
///
/// # Returns
/// A vector of file paths that import the target file
pub fn find_importers(file_path: &Path, base_path: &Path) -> Vec<String> {
    let mut importers = Vec::new();
    let target_name = file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    if target_name.is_empty() {
        return importers;
    }

    // Walk the base_path and find files that might import our target
    if let Ok(entries) = fs::read_dir(base_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            if path.is_file() && path != file_path && is_source_file(&path) {
                if let Ok(analysis) = analyze_file(&path) {
                    for import in &analysis.imports {
                        if import.module_path.contains(target_name)
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
                // Recurse into subdirectories
                importers.extend(find_importers(file_path, &path));
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
}
