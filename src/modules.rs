//! Module structure analysis - generates ASCII tree from directory structure

use std::fs;
use std::path::Path;

use crate::utils::fs::{is_source_filename, should_skip_dir};

/// Generate an ASCII tree representation of the module structure
///
/// # Arguments
/// * `base_path` - The root directory to start from (usually src/)
/// * `target_file` - The file being reviewed (will be marked with "← HERE")
///
/// # Returns
/// An ASCII tree string like:
/// ```text
/// src/
/// ├── lib.rs
/// ├── reviewer.rs ← HERE
/// ├── prompt.rs
/// └── git.rs
/// ```
pub fn generate_module_tree(base_path: &Path, target_file: &Path) -> String {
    let mut result = String::new();

    // Get the directory name for the header
    let dir_name = base_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(".");

    result.push_str(dir_name);
    result.push_str("/\n");

    // Collect and sort entries
    let mut entries: Vec<_> = match fs::read_dir(base_path) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                let name = e.file_name();
                let name_str = name.to_string_lossy();
                // Skip hidden files and common build directories
                !should_skip_dir(&name_str)
            })
            .collect(),
        Err(_) => return result,
    };

    // Sort: directories first, then files, alphabetically
    entries.sort_by(|a, b| {
        let a_is_dir = a.path().is_dir();
        let b_is_dir = b.path().is_dir();
        match (a_is_dir, b_is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.file_name().cmp(&b.file_name()),
        }
    });

    let total = entries.len();
    for (idx, entry) in entries.iter().enumerate() {
        let is_last = idx == total - 1;
        let prefix = if is_last { "└── " } else { "├── " };

        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        result.push_str(prefix);
        result.push_str(&name_str);

        // Mark the target file
        if path == target_file {
            result.push_str(" ← HERE");
        }

        // If it's a directory, add a trailing slash
        if path.is_dir() {
            result.push('/');
        }

        result.push('\n');

        // Recurse into directories (limited depth)
        if path.is_dir() {
            let child_prefix = if is_last { "    " } else { "│   " };
            let subtree = generate_subtree(&path, target_file, child_prefix, 1);
            result.push_str(&subtree);
        }
    }

    result
}

/// Generate subtree with proper indentation (limited to 2 levels deep)
fn generate_subtree(dir: &Path, target_file: &Path, prefix: &str, depth: usize) -> String {
    if depth > 2 {
        return String::new();
    }

    let mut result = String::new();

    let mut entries: Vec<_> = match fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                let name = e.file_name();
                let name_str = name.to_string_lossy();
                !should_skip_dir(&name_str)
            })
            .collect(),
        Err(_) => return result,
    };

    // Sort entries
    entries.sort_by(|a, b| {
        let a_is_dir = a.path().is_dir();
        let b_is_dir = b.path().is_dir();
        match (a_is_dir, b_is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.file_name().cmp(&b.file_name()),
        }
    });

    let total = entries.len();
    for (idx, entry) in entries.iter().enumerate() {
        let is_last = idx == total - 1;
        let connector = if is_last { "└── " } else { "├── " };

        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        result.push_str(prefix);
        result.push_str(connector);
        result.push_str(&name_str);

        if path == target_file {
            result.push_str(" ← HERE");
        }

        if path.is_dir() {
            result.push('/');
        }

        result.push('\n');

        // Recurse
        if path.is_dir() {
            let child_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });
            let subtree = generate_subtree(&path, target_file, &child_prefix, depth + 1);
            result.push_str(&subtree);
        }
    }

    result
}

/// Get a compact list of sibling files (files in the same directory)
pub fn get_sibling_files(file_path: &Path) -> Vec<String> {
    let parent = match file_path.parent() {
        Some(p) => p,
        None => return Vec::new(),
    };

    let mut siblings = Vec::new();

    if let Ok(entries) = fs::read_dir(parent) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path != file_path {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    // Only include source files
                    if is_source_filename(name) {
                        siblings.push(name.to_string());
                    }
                }
            }
        }
    }

    siblings.sort();
    siblings
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_generate_module_tree() {
        // Just test that it doesn't panic on a valid path
        let path = PathBuf::from(".");
        let target = PathBuf::from("./Cargo.toml");
        let tree = generate_module_tree(&path, &target);
        assert!(!tree.is_empty());
    }
}
