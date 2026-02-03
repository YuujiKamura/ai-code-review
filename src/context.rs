//! Context gathering for enhanced code review
//!
//! This module collects project context information to provide
//! better architectural insights during code review.

use std::fmt::Write as FmtWrite;
use std::path::Path;

use crate::analyzer::find_importers;
use crate::error::Result;
use crate::git::get_cochanged_files;
use crate::modules::{generate_module_tree, get_sibling_files};
use crate::parser::analyze_file;

/// Information about a file that's frequently changed together with the target
#[derive(Debug, Clone)]
pub struct RelatedFile {
    /// Path to the related file
    pub path: String,
    /// Number of times this file was changed together with the target
    pub co_change_count: usize,
}

/// Dependency information for a file
#[derive(Debug, Clone, Default)]
pub struct DependencyInfo {
    /// Modules/files this file imports
    pub imports: Vec<String>,
    /// Files that import this file
    pub imported_by: Vec<String>,
}

/// Complete project context for a file
#[derive(Debug, Clone)]
pub struct ProjectContext {
    /// ASCII tree representation of the module structure
    pub module_tree: String,
    /// Files frequently changed together with this file
    pub related_files: Vec<RelatedFile>,
    /// Import/export dependency information
    pub dependencies: DependencyInfo,
    /// Sibling files (in the same directory)
    pub sibling_files: Vec<String>,
}

impl ProjectContext {
    /// Create an empty context
    pub fn empty() -> Self {
        Self {
            module_tree: String::new(),
            related_files: Vec::new(),
            dependencies: DependencyInfo::default(),
            sibling_files: Vec::new(),
        }
    }

    /// Format context as a prompt-friendly string
    pub fn to_prompt_string(&self) -> String {
        let mut output = String::new();

        // Module structure
        if !self.module_tree.is_empty() {
            output.push_str("## プロジェクト構造\n```\n");
            output.push_str(&self.module_tree);
            output.push_str("```\n\n");
        }

        // Related files (co-changed)
        if !self.related_files.is_empty() {
            output.push_str("## 最近一緒に変更されたファイル\n");
            for rf in &self.related_files {
                let _ = writeln!(output, "- {} ({}回)", rf.path, rf.co_change_count);
            }
            output.push('\n');
        }

        // Dependencies
        if !self.dependencies.imports.is_empty() || !self.dependencies.imported_by.is_empty() {
            output.push_str("## 依存関係\n");
            if !self.dependencies.imports.is_empty() {
                output.push_str("このファイルが使用: ");
                output.push_str(&self.dependencies.imports.join(", "));
                output.push('\n');
            }
            if !self.dependencies.imported_by.is_empty() {
                output.push_str("このファイルを使用: ");
                output.push_str(&self.dependencies.imported_by.join(", "));
                output.push('\n');
            }
            output.push('\n');
        }

        // Sibling files
        if !self.sibling_files.is_empty() {
            output.push_str("## 同じディレクトリのファイル\n");
            output.push_str(&self.sibling_files.join(", "));
            output.push_str("\n\n");
        }

        output
    }

    /// Check if the context has any useful information
    pub fn is_empty(&self) -> bool {
        self.module_tree.is_empty()
            && self.related_files.is_empty()
            && self.dependencies.imports.is_empty()
            && self.dependencies.imported_by.is_empty()
            && self.sibling_files.is_empty()
    }
}

/// Gather all context information for a file
///
/// # Arguments
/// * `file_path` - The file being reviewed
/// * `base_path` - The project root (usually the git root or src/ directory)
/// * `lookback` - Number of commits to look back for co-changed files
///
/// # Returns
/// A `ProjectContext` containing all gathered information
pub fn gather_context(file_path: &Path, base_path: &Path, lookback: usize) -> Result<ProjectContext> {
    // Get module tree
    let src_path = if base_path.join("src").exists() {
        base_path.join("src")
    } else {
        base_path.to_path_buf()
    };
    let module_tree = generate_module_tree(&src_path, file_path);

    // Get co-changed files from git history
    let cochanged = get_cochanged_files(file_path, lookback);
    let related_files: Vec<RelatedFile> = cochanged
        .into_iter()
        .take(5) // Limit to top 5
        .map(|(path, count)| RelatedFile {
            path,
            co_change_count: count,
        })
        .collect();

    // Analyze imports/exports
    let mut dependencies = DependencyInfo::default();
    if let Ok(analysis) = analyze_file(file_path) {
        dependencies.imports = analysis
            .imports
            .iter()
            .map(|i| {
                if i.items.is_empty() {
                    i.module_path.clone()
                } else {
                    format!("{}::{{{}}}", i.module_path, i.items.join(", "))
                }
            })
            .collect();
    }

    // Find files that import this file
    dependencies.imported_by = find_importers(file_path, base_path);

    // Get sibling files
    let sibling_files = get_sibling_files(file_path);

    Ok(ProjectContext {
        module_tree,
        related_files,
        dependencies,
        sibling_files,
    })
}

/// Gather context with default settings
pub fn gather_context_default(file_path: &Path, base_path: &Path) -> Result<ProjectContext> {
    gather_context(file_path, base_path, 50) // Default: look back 50 commits
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_empty_context() {
        let ctx = ProjectContext::empty();
        assert!(ctx.is_empty());
        assert!(ctx.to_prompt_string().is_empty() || ctx.to_prompt_string().trim().is_empty());
    }

    #[test]
    fn test_context_to_prompt() {
        let ctx = ProjectContext {
            module_tree: "src/\n└── main.rs".to_string(),
            related_files: vec![RelatedFile {
                path: "lib.rs".to_string(),
                co_change_count: 3,
            }],
            dependencies: DependencyInfo {
                imports: vec!["std::path::Path".to_string()],
                imported_by: vec!["main.rs".to_string()],
            },
            sibling_files: vec!["other.rs".to_string()],
        };

        let prompt = ctx.to_prompt_string();
        assert!(prompt.contains("プロジェクト構造"));
        assert!(prompt.contains("最近一緒に変更されたファイル"));
        assert!(prompt.contains("依存関係"));
        assert!(prompt.contains("同じディレクトリのファイル"));
    }

    #[test]
    fn test_gather_context() {
        // Test with current directory
        let base = PathBuf::from(".");
        let file = PathBuf::from("./Cargo.toml");
        let result = gather_context(&file, &base, 10);
        // Should not error even if not a git repo
        assert!(result.is_ok());
    }
}
