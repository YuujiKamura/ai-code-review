//! Git integration for getting diffs

use std::path::Path;
use std::process::Command;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Get git diff for a file (unstaged changes first, then staged)
pub fn get_git_diff(file_path: &Path) -> Option<String> {
    let file_str = file_path.to_string_lossy();
    let parent = file_path.parent()?;

    // Try unstaged changes first
    let mut cmd = Command::new("git");
    cmd.args(["diff", "--", &file_str]).current_dir(parent);

    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = cmd.output().ok()?;
    if output.status.success() {
        let diff = String::from_utf8_lossy(&output.stdout).to_string();
        if !diff.trim().is_empty() {
            return Some(diff);
        }

        // Try staged changes
        let mut cmd2 = Command::new("git");
        cmd2.args(["diff", "--cached", "--", &file_str])
            .current_dir(parent);

        #[cfg(target_os = "windows")]
        cmd2.creation_flags(CREATE_NO_WINDOW);

        let output2 = cmd2.output().ok()?;
        if output2.status.success() {
            let diff2 = String::from_utf8_lossy(&output2.stdout).to_string();
            if !diff2.trim().is_empty() {
                return Some(diff2);
            }
        }
    }

    None
}

/// Check if a path is inside a git repository
#[allow(dead_code)]
pub fn is_git_repo(path: &Path) -> bool {
    let mut cmd = Command::new("git");
    cmd.args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(path);

    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    cmd.output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Get the git root directory for a path
#[allow(dead_code)]
pub fn get_git_root(path: &Path) -> Option<String> {
    let mut cmd = Command::new("git");
    cmd.args(["rev-parse", "--show-toplevel"]).current_dir(path);

    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = cmd.output().ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

/// Get files that were frequently changed together with the given file
///
/// Looks at the last N commits that touched this file and counts
/// which other files were changed in the same commits.
///
/// # Arguments
/// * `file_path` - The file to analyze
/// * `lookback` - Number of commits to look back
///
/// # Returns
/// A vector of (file_path, co_change_count) tuples, sorted by count descending
pub fn get_cochanged_files(file_path: &Path, lookback: usize) -> Vec<(String, usize)> {
    use std::collections::HashMap;

    let file_str = file_path.to_string_lossy();
    let parent = match file_path.parent() {
        Some(p) => p,
        None => return Vec::new(),
    };

    let target_name = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");

    if target_name.is_empty() {
        return Vec::new();
    }

    // Single git command: get commits with their changed files
    // --full-diff ensures all files in each commit are listed, not just the pathspec match
    let mut cmd = Command::new("git");
    cmd.args([
        "log",
        "--format=",   // suppress commit info, output only file names
        "--name-only",
        "--full-diff",
        "-n",
        &lookback.to_string(),
        "--",
        &file_str,
    ])
    .current_dir(parent);

    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = match cmd.output() {
        Ok(o) if o.status.success() => o,
        _ => return Vec::new(),
    };

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse output: "file1\nfile2\n\nfile3\n..." (blank lines between commits)
    let mut file_counts: HashMap<String, usize> = HashMap::new();

    let target_suffix = format!("/{}", target_name);
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // Skip the target file itself
        let is_target = line == target_name
            || line.ends_with(&target_suffix);
        if !is_target {
            *file_counts.entry(line.to_string()).or_insert(0) += 1;
        }
    }

    // Sort by count descending
    let mut result: Vec<(String, usize)> = file_counts.into_iter().collect();
    result.sort_by(|a, b| b.1.cmp(&a.1));
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_is_git_repo() {
        // Current directory should be a git repo if running tests
        let current = PathBuf::from(".");
        // This may or may not be true depending on where tests are run
        let _ = is_git_repo(&current);
    }

    #[test]
    fn test_get_cochanged_files() {
        // Test that it doesn't panic even if not in a git repo
        let path = PathBuf::from("./Cargo.toml");
        let result = get_cochanged_files(&path, 10);
        // Result may be empty if not in a git repo, that's fine
        let _ = result;
    }
}
