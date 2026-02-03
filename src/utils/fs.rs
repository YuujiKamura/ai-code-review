//! File system utilities
//!
//! Common utilities for traversing directories and identifying source files.

use std::fs;
use std::path::{Path, PathBuf};

/// Source code file extensions supported for analysis
const SOURCE_EXTENSIONS: &[&str] = &[
    ".rs", ".ts", ".tsx", ".js", ".jsx", ".py", ".go", ".java", ".cpp", ".c", ".h", ".hpp", ".cs",
    ".rb", ".swift", ".kt",
];

/// Directories to skip during traversal
const SKIP_DIRS: &[&str] = &["target", "node_modules", "__pycache__"];

/// Check if a directory should be skipped during traversal
///
/// Skips hidden directories (starting with '.'), target, node_modules, __pycache__
///
/// # Arguments
/// * `name` - The directory name to check
///
/// # Returns
/// `true` if the directory should be skipped
pub fn should_skip_dir(name: &str) -> bool {
    name.starts_with('.') || SKIP_DIRS.contains(&name)
}

/// Check if a file is a source code file based on its extension
///
/// # Arguments
/// * `path` - The path to check
///
/// # Returns
/// `true` if the file has a recognized source code extension
pub fn is_source_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|name| SOURCE_EXTENSIONS.iter().any(|ext| name.ends_with(ext)))
        .unwrap_or(false)
}

/// Check if a filename looks like a source code file
///
/// # Arguments
/// * `name` - The filename to check
///
/// # Returns
/// `true` if the filename has a recognized source code extension
pub fn is_source_filename(name: &str) -> bool {
    SOURCE_EXTENSIONS.iter().any(|ext| name.ends_with(ext))
}

/// Recursively find source files in a directory
///
/// Skips hidden directories, target, node_modules, __pycache__
///
/// # Arguments
/// * `base_path` - The root directory to start searching from
///
/// # Returns
/// A vector of paths to source files found
#[allow(dead_code)]
pub fn find_source_files(base_path: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    find_source_files_recursive(base_path, &mut files);
    files
}

fn find_source_files_recursive(dir: &Path, files: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if path.is_dir() {
            if !should_skip_dir(&name_str) {
                find_source_files_recursive(&path, files);
            }
        } else if path.is_file() && is_source_file(&path) {
            files.push(path);
        }
    }
}

/// Walk a directory tree and apply a function to each source file
///
/// # Arguments
/// * `base_path` - The root directory to start from
/// * `callback` - Function to call for each source file found
#[allow(dead_code)]
pub fn walk_source_files<F>(base_path: &Path, mut callback: F)
where
    F: FnMut(&Path),
{
    walk_source_files_recursive(base_path, &mut callback);
}

fn walk_source_files_recursive<F>(dir: &Path, callback: &mut F)
where
    F: FnMut(&Path),
{
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if path.is_dir() {
            if !should_skip_dir(&name_str) {
                walk_source_files_recursive(&path, callback);
            }
        } else if path.is_file() && is_source_file(&path) {
            callback(&path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::tempdir;

    #[test]
    fn test_should_skip_dir() {
        assert!(should_skip_dir(".git"));
        assert!(should_skip_dir(".hidden"));
        assert!(should_skip_dir("target"));
        assert!(should_skip_dir("node_modules"));
        assert!(should_skip_dir("__pycache__"));
        assert!(!should_skip_dir("src"));
        assert!(!should_skip_dir("lib"));
    }

    #[test]
    fn test_is_source_file() {
        assert!(is_source_file(Path::new("main.rs")));
        assert!(is_source_file(Path::new("app.tsx")));
        assert!(is_source_file(Path::new("script.py")));
        assert!(is_source_file(Path::new("/path/to/file.go")));
        assert!(!is_source_file(Path::new("Cargo.toml")));
        assert!(!is_source_file(Path::new("README.md")));
    }

    #[test]
    fn test_is_source_filename() {
        assert!(is_source_filename("main.rs"));
        assert!(is_source_filename("app.tsx"));
        assert!(!is_source_filename("Cargo.toml"));
    }

    #[test]
    fn test_find_source_files() {
        let dir = tempdir().unwrap();
        let src_dir = dir.path().join("src");
        std::fs::create_dir(&src_dir).unwrap();

        // Create some source files
        File::create(src_dir.join("main.rs")).unwrap();
        File::create(src_dir.join("lib.rs")).unwrap();
        File::create(dir.path().join("Cargo.toml")).unwrap();

        // Create a directory that should be skipped
        let target_dir = dir.path().join("target");
        std::fs::create_dir(&target_dir).unwrap();
        File::create(target_dir.join("ignored.rs")).unwrap();

        let files = find_source_files(dir.path());
        assert_eq!(files.len(), 2);
        assert!(files.iter().any(|p| p.ends_with("main.rs")));
        assert!(files.iter().any(|p| p.ends_with("lib.rs")));
    }

    #[test]
    fn test_walk_source_files() {
        let dir = tempdir().unwrap();
        File::create(dir.path().join("test.rs")).unwrap();
        File::create(dir.path().join("config.toml")).unwrap();

        let mut count = 0;
        walk_source_files(dir.path(), |_| count += 1);
        assert_eq!(count, 1);
    }
}
