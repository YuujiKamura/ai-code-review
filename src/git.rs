//! Git integration for getting diffs

use std::path::{Path, PathBuf};
use std::process::Command;

/// Create a `Command` for git with the given arguments.
/// On Windows, sets `CREATE_NO_WINDOW` to suppress console popups.
fn new_git_command(args: &[&str]) -> Command {
    let mut cmd = Command::new("git");
    cmd.args(args);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    cmd
}

/// Get git diff for a file (unstaged changes first, then staged)
pub fn get_git_diff(file_path: &Path) -> Option<String> {
    let file_str = file_path.to_string_lossy();
    let parent = file_path.parent()?;

    // Try unstaged changes first
    let mut cmd = new_git_command(&["diff", "--", &file_str]);
    cmd.current_dir(parent);

    let output = cmd.output().ok()?;
    if output.status.success() {
        let diff = String::from_utf8_lossy(&output.stdout).to_string();
        if !diff.trim().is_empty() {
            return Some(diff);
        }

        // Try staged changes
        let mut cmd2 = new_git_command(&["diff", "--cached", "--", &file_str]);
        cmd2.current_dir(parent);

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
    let mut cmd = new_git_command(&["rev-parse", "--is-inside-work-tree"]);
    cmd.current_dir(path);

    cmd.output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Get the git root directory for a path
#[allow(dead_code)]
pub fn get_git_root(path: &Path) -> Option<String> {
    let mut cmd = new_git_command(&["rev-parse", "--show-toplevel"]);
    cmd.current_dir(path);

    let output = cmd.output().ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

/// Get the staged diff (all files) for pre-commit hook support
pub fn get_staged_diff(repo_dir: &Path) -> Option<String> {
    let mut cmd = new_git_command(&["diff", "--cached"]);
    cmd.current_dir(repo_dir);
    let output = cmd.output().ok()?;
    if output.status.success() {
        let s = String::from_utf8_lossy(&output.stdout).to_string();
        if s.trim().is_empty() { None } else { Some(s) }
    } else {
        None
    }
}

/// Get the list of staged files as absolute paths
pub fn get_staged_files(repo_dir: &Path) -> Vec<PathBuf> {
    let mut cmd = new_git_command(&["diff", "--cached", "--name-only"]);
    cmd.current_dir(repo_dir);
    match cmd.output() {
        Ok(o) if o.status.success() => {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .filter(|l| !l.is_empty())
                .map(|l| repo_dir.join(l))
                .collect()
        }
        _ => vec![],
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
    let mut cmd = new_git_command(&[
        "log",
        "--format=",   // suppress commit info, output only file names
        "--name-only",
        "--full-diff",
        "-n",
        &lookback.to_string(),
        "--",
        &file_str,
    ]);
    cmd.current_dir(parent);

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
