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
}
