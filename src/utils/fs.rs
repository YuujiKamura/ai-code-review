//! File system utilities
//!
//! Common utilities for traversing directories and identifying source files.

use std::fs;
use std::path::{Path, PathBuf};

/// Source code extensions (without dot)
pub const SOURCE_EXTENSIONS: &[&str] = &[
    "rs", "ts", "tsx", "js", "jsx", "py", "go", "java", "cpp", "c", "h", "hpp", "cs",
    "rb", "swift", "kt",
];

/// Config file extensions (without dot)
pub const CONFIG_EXTENSIONS: &[&str] = &["json", "toml", "yaml", "yml"];

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
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| SOURCE_EXTENSIONS.contains(&ext))
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
    Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|ext| SOURCE_EXTENSIONS.contains(&ext))
        .unwrap_or(false)
}

/// Recursively find all source files matching the given extensions.
///
/// Walks the directory tree starting from `dir`, skipping hidden directories,
/// `target/`, `node_modules/`, and `__pycache__/`.
///
/// # Arguments
/// * `dir` - The root directory to start searching from
/// * `extensions` - A slice of file extensions to match (e.g. `&["rs", "py", "ts"]`).
///   Each extension should be without a leading dot.
///
/// # Returns
/// A vector of paths to matching source files
pub fn walk_source_files(dir: &Path, extensions: &[&str]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    walk_source_files_recursive(dir, extensions, &mut files);
    files
}

fn walk_source_files_recursive(dir: &Path, extensions: &[&str], files: &mut Vec<PathBuf>) {
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
                walk_source_files_recursive(&path, extensions, files);
            }
        } else if path.is_file() {
            let matches = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|ext| extensions.iter().any(|&e| e == ext))
                .unwrap_or(false);
            if matches {
                files.push(path);
            }
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
    fn test_walk_source_files() {
        let dir = tempdir().unwrap();
        File::create(dir.path().join("test.rs")).unwrap();
        File::create(dir.path().join("app.py")).unwrap();
        File::create(dir.path().join("config.toml")).unwrap();

        // Only .rs files
        let rs_files = walk_source_files(dir.path(), &["rs"]);
        assert_eq!(rs_files.len(), 1);
        assert!(rs_files[0].ends_with("test.rs"));

        // .rs and .py files
        let multi_files = walk_source_files(dir.path(), &["rs", "py"]);
        assert_eq!(multi_files.len(), 2);

        // No matching extension
        let no_files = walk_source_files(dir.path(), &["go"]);
        assert!(no_files.is_empty());
    }

    #[test]
    fn test_walk_source_files_skips_dirs() {
        let dir = tempdir().unwrap();
        let target_dir = dir.path().join("target");
        std::fs::create_dir(&target_dir).unwrap();
        File::create(target_dir.join("ignored.rs")).unwrap();

        let hidden_dir = dir.path().join(".hidden");
        std::fs::create_dir(&hidden_dir).unwrap();
        File::create(hidden_dir.join("secret.rs")).unwrap();

        let src_dir = dir.path().join("src");
        std::fs::create_dir(&src_dir).unwrap();
        File::create(src_dir.join("main.rs")).unwrap();

        let files = walk_source_files(dir.path(), &["rs"]);
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("main.rs"));
    }
}
