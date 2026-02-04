//! Simplified context gathering - let AI do the analysis
//!
//! Philosophy: Gather raw context, let AI analyze.
//! No complex AST parsing - just collect what AI needs to see.

use std::fs;
use std::path::Path;

use crate::git::get_cochanged_files;
use crate::modules::generate_module_tree;
use crate::utils::fs::is_source_file;

/// Raw project context - just strings, no complex parsing
#[derive(Debug, Default)]
pub struct RawContext {
    /// Project structure as ASCII tree
    pub structure: String,
    /// Files that changed together (from git history)
    pub cochanged: Vec<(String, usize)>,
    /// Related file contents (siblings, importers) - raw text
    pub related_files: Vec<(String, String)>,
    /// Requirements/docs if found
    pub docs: Option<String>,
}

impl RawContext {
    pub fn is_empty(&self) -> bool {
        self.structure.is_empty() && self.related_files.is_empty()
    }

    /// Convert to prompt string - just concatenate, let AI parse
    pub fn to_prompt_string(&self) -> String {
        let mut result = String::new();

        // Structure
        if !self.structure.is_empty() {
            result.push_str("## プロジェクト構造\n```\n");
            result.push_str(&self.structure);
            result.push_str("```\n\n");
        }

        // Co-changed files
        if !self.cochanged.is_empty() {
            result.push_str("## 一緒に変更されるファイル\n");
            for (file, count) in &self.cochanged {
                result.push_str(&format!("- {} ({}回)\n", file, count));
            }
            result.push('\n');
        }

        // Related file contents
        if !self.related_files.is_empty() {
            result.push_str("## 関連ファイルの内容\n");
            for (name, content) in &self.related_files {
                result.push_str(&format!("### {}\n```\n", name));
                // Truncate if too long
                if content.len() > 2000 {
                    result.push_str(&content[..2000]);
                    result.push_str("\n... (truncated)");
                } else {
                    result.push_str(content);
                }
                result.push_str("\n```\n\n");
            }
        }

        // Docs
        if let Some(docs) = &self.docs {
            result.push_str("## プロジェクト要件/ドキュメント\n");
            result.push_str(docs);
            result.push('\n');
        }

        result
    }
}

/// Gather raw context - simple file reads, no AST parsing
pub fn gather_raw_context(file_path: &Path, base_path: &Path, max_files: usize) -> RawContext {
    let mut ctx = RawContext::default();

    // 1. Project structure (cheap - just directory listing)
    ctx.structure = generate_module_tree(base_path, file_path);

    // 2. Co-changed files from git (cheap - git commands)
    ctx.cochanged = get_cochanged_files(file_path, 50)
        .into_iter()
        .take(5)
        .collect();

    // 3. Sibling files content (just read, no parse)
    if let Some(parent) = file_path.parent() {
        if let Ok(entries) = fs::read_dir(parent) {
            let mut count = 0;
            for entry in entries.flatten() {
                if count >= max_files {
                    break;
                }
                let path = entry.path();
                if path.is_file() && path != file_path && is_source_file(&path) {
                    if let Ok(content) = fs::read_to_string(&path) {
                        let name = path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default();
                        ctx.related_files.push((name, content));
                        count += 1;
                    }
                }
            }
        }
    }

    // 4. Look for README or docs
    let readme_paths = ["README.md", "README", "docs/README.md"];
    for readme in readme_paths {
        let readme_path = base_path.join(readme);
        if readme_path.exists() {
            if let Ok(content) = fs::read_to_string(&readme_path) {
                ctx.docs = Some(content);
                break;
            }
        }
    }

    ctx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_raw_context_empty() {
        let ctx = RawContext::default();
        assert!(ctx.is_empty());
    }

    #[test]
    fn test_raw_context_to_prompt() {
        let mut ctx = RawContext::default();
        ctx.structure = "src/\n└── main.rs".to_string();
        ctx.related_files.push(("lib.rs".to_string(), "pub fn foo() {}".to_string()));

        let prompt = ctx.to_prompt_string();
        assert!(prompt.contains("プロジェクト構造"));
        assert!(prompt.contains("main.rs"));
        assert!(prompt.contains("lib.rs"));
    }
}
