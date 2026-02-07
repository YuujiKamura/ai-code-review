//! Cross-project shared code discovery
//!
//! Scans two project directories and identifies potential shared/duplicated code.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::parser::analyze_file;
use crate::utils::fs::walk_source_files;

/// Source file extensions to scan
const SCAN_EXTENSIONS: &[&str] = &[
    "rs", "ts", "tsx", "js", "jsx", "py", "go", "java", "cpp", "c", "h", "hpp",
    "json", "toml", "yaml", "yml",
];

/// A candidate pair of files/symbols that may be shared between two projects
#[derive(Debug, Clone)]
pub struct SharedCandidate {
    /// Category of sharing opportunity
    pub kind: SharedKind,
    /// Path in project A (relative to project root)
    pub path_a: String,
    /// Path in project B (relative to project root)
    pub path_b: String,
    /// Brief description of what's shared
    pub description: String,
    /// Similarity score 0.0-1.0 (1.0 = identical)
    pub similarity: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SharedKind {
    /// Same filename exists in both projects
    SameFileName,
    /// Same exported symbol name in both projects
    SameExport,
    /// Same constant or type name appears in both
    SameConstant,
    /// Files have similar content (high line overlap)
    SimilarContent,
}

impl std::fmt::Display for SharedKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SharedKind::SameFileName => write!(f, "同名ファイル"),
            SharedKind::SameExport => write!(f, "同名エクスポート"),
            SharedKind::SameConstant => write!(f, "同名定数/型"),
            SharedKind::SimilarContent => write!(f, "類似コンテンツ"),
        }
    }
}

/// Result of cross-project shared code analysis
#[derive(Debug)]
pub struct SharedReport {
    pub project_a: String,
    pub project_b: String,
    pub candidates: Vec<SharedCandidate>,
    pub files_scanned_a: usize,
    pub files_scanned_b: usize,
}

impl SharedReport {
    /// Format as a prompt-friendly string for AI analysis
    pub fn to_prompt_string(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "## Cross-Project Shared Code Analysis\n\nProject A: {} ({} files)\nProject B: {} ({} files)\n\n",
            self.project_a, self.files_scanned_a, self.project_b, self.files_scanned_b
        ));

        if self.candidates.is_empty() {
            out.push_str("共有候補は見つかりませんでした。\n");
            return out;
        }

        out.push_str(&format!("### 共有候補: {} 件\n\n", self.candidates.len()));

        for (i, c) in self.candidates.iter().enumerate() {
            out.push_str(&format!(
                "{}. [{}] (類似度: {:.0}%)\n   A: {}\n   B: {}\n   {}\n\n",
                i + 1,
                c.kind,
                c.similarity * 100.0,
                c.path_a,
                c.path_b,
                c.description,
            ));
        }

        out
    }
}

/// Find shared code candidates between two project directories
pub fn find_shared_candidates(path_a: &Path, path_b: &Path) -> SharedReport {
    let files_a = walk_source_files(path_a, SCAN_EXTENSIONS);
    let files_b = walk_source_files(path_b, SCAN_EXTENSIONS);

    let mut candidates = Vec::new();

    // 1. Same-name files
    find_same_name_files(path_a, path_b, &files_a, &files_b, &mut candidates);

    // 2. Same exports (via AST parsing)
    find_same_exports(path_a, path_b, &files_a, &files_b, &mut candidates);

    // 3. Cross-language identifier matching (snake_case ↔ camelCase)
    find_cross_language_symbols(path_a, path_b, &files_a, &files_b, &mut candidates);

    // Sort by similarity descending
    candidates.sort_by(|a, b| {
        b.similarity
            .partial_cmp(&a.similarity)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Deduplicate: if same pair appears in multiple categories, keep highest similarity
    dedup_candidates(&mut candidates);

    SharedReport {
        project_a: path_a.display().to_string(),
        project_b: path_b.display().to_string(),
        candidates,
        files_scanned_a: files_a.len(),
        files_scanned_b: files_b.len(),
    }
}

/// Find files with the same name in both projects, then compare content similarity
fn find_same_name_files(
    root_a: &Path,
    root_b: &Path,
    files_a: &[PathBuf],
    files_b: &[PathBuf],
    candidates: &mut Vec<SharedCandidate>,
) {
    // Build map: filename -> list of paths for project B
    let mut name_map_b: HashMap<String, Vec<&PathBuf>> = HashMap::new();
    for fb in files_b {
        if let Some(name) = fb.file_name().and_then(|n| n.to_str()) {
            name_map_b.entry(name.to_string()).or_default().push(fb);
        }
    }

    for fa in files_a {
        let name = match fa.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };

        if let Some(matches) = name_map_b.get(&name) {
            for fb in matches {
                let similarity = content_similarity(fa, fb);
                let rel_a = fa.strip_prefix(root_a).unwrap_or(fa).display().to_string();
                let rel_b = fb
                    .strip_prefix(root_b)
                    .unwrap_or(fb)
                    .display()
                    .to_string();

                // Skip if same relative path and identical (probably copied intentionally)
                // But still report since user wants to find these
                let desc = if similarity > 0.95 {
                    format!("同名ファイル「{}」がほぼ同一内容で存在", name)
                } else if similarity > 0.3 {
                    format!(
                        "同名ファイル「{}」が異なる内容で存在（分岐コピーの可能性）",
                        name
                    )
                } else {
                    format!("同名ファイル「{}」（内容は大きく異なる）", name)
                };

                candidates.push(SharedCandidate {
                    kind: SharedKind::SameFileName,
                    path_a: rel_a,
                    path_b: rel_b,
                    description: desc,
                    similarity,
                });
            }
        }
    }
}

/// Find same exported symbols across projects
fn find_same_exports(
    root_a: &Path,
    root_b: &Path,
    files_a: &[PathBuf],
    files_b: &[PathBuf],
    candidates: &mut Vec<SharedCandidate>,
) {
    // Parse exports from both projects
    let exports_a = collect_exports(root_a, files_a);
    let exports_b = collect_exports(root_b, files_b);

    // Find symbols that appear in both projects
    for (symbol, locs_a) in &exports_a {
        if let Some(locs_b) = exports_b.get(symbol) {
            // Skip very common names that are likely coincidental
            if is_common_symbol(symbol) {
                continue;
            }

            for loc_a in locs_a {
                for loc_b in locs_b {
                    candidates.push(SharedCandidate {
                        kind: SharedKind::SameExport,
                        path_a: loc_a.clone(),
                        path_b: loc_b.clone(),
                        description: format!(
                            "同名シンボル「{}」が両プロジェクトでエクスポート",
                            symbol
                        ),
                        similarity: 0.7, // Base similarity for same-name exports
                    });
                }
            }
        }
    }
}

/// Collect all exported symbols from a set of files
/// Returns: symbol_name -> Vec<relative_path>
fn collect_exports(root: &Path, files: &[PathBuf]) -> HashMap<String, Vec<String>> {
    let mut exports: HashMap<String, Vec<String>> = HashMap::new();

    for file in files {
        if let Ok(analysis) = analyze_file(file) {
            let rel = file
                .strip_prefix(root)
                .unwrap_or(file)
                .display()
                .to_string();
            for export in &analysis.exports {
                exports.entry(export.clone()).or_default().push(rel.clone());
            }
        }
    }

    exports
}

/// Normalize identifier to a canonical form for cross-language matching.
/// Converts snake_case, camelCase, PascalCase, SCREAMING_SNAKE_CASE to lowercase words.
fn normalize_identifier(name: &str) -> String {
    let mut words = Vec::new();
    let mut current = String::new();

    for ch in name.chars() {
        if ch == '_' || ch == '-' {
            if !current.is_empty() {
                words.push(current.to_lowercase());
                current.clear();
            }
        } else if ch.is_uppercase() && !current.is_empty() && !current.chars().last().unwrap_or('A').is_uppercase() {
            // camelCase boundary: lowercase followed by uppercase
            words.push(current.to_lowercase());
            current.clear();
            current.push(ch);
        } else {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        words.push(current.to_lowercase());
    }
    words.join("_")
}

/// Extract identifiers from source code using simple pattern matching.
/// Returns a set of (normalized_name, original_name) pairs.
fn extract_identifiers(content: &str) -> Vec<(String, String)> {
    let mut identifiers = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        // Skip comments
        if trimmed.starts_with("//") || trimmed.starts_with('#') || trimmed.starts_with("/*") || trimmed.starts_with('*') {
            continue;
        }

        // Extract UPPER_CASE constants (e.g., TRUCK_SPECS, DEFAULT_BED_AREA)
        for word in trimmed.split(|c: char| !c.is_alphanumeric() && c != '_') {
            if word.len() >= 4
                && word.chars().all(|c| c.is_uppercase() || c.is_ascii_digit() || c == '_')
                && word.contains('_')
            {
                let norm = normalize_identifier(word);
                identifiers.push((norm, word.to_string()));
            }
        }

        // Extract camelCase/PascalCase identifiers (4+ chars, mixed case)
        for word in trimmed.split(|c: char| !c.is_alphanumeric() && c != '_') {
            if word.len() >= 4
                && word.chars().any(|c| c.is_lowercase())
                && word.chars().any(|c| c.is_uppercase())
            {
                let norm = normalize_identifier(word);
                identifiers.push((norm, word.to_string()));
            }
        }
    }

    identifiers
}

/// Find shared identifiers across languages (snake_case in Rust ↔ camelCase in TS)
fn find_cross_language_symbols(
    root_a: &Path,
    root_b: &Path,
    files_a: &[PathBuf],
    files_b: &[PathBuf],
    candidates: &mut Vec<SharedCandidate>,
) {
    // Collect normalized identifiers from each project
    // normalized_name -> Vec<(original_name, relative_path)>
    let ids_a = collect_normalized_identifiers(root_a, files_a);
    let ids_b = collect_normalized_identifiers(root_b, files_b);

    for (norm, locs_a) in &ids_a {
        if let Some(locs_b) = ids_b.get(norm) {
            // Skip very short normalized names
            if norm.len() < 6 {
                continue;
            }
            // Skip common patterns
            if is_common_normalized(norm) {
                continue;
            }

            // Only report first pair per normalized name to avoid explosion
            let (orig_a, path_a) = &locs_a[0];
            let (orig_b, path_b) = &locs_b[0];

            // If they're exactly the same name, skip (already caught by SameExport)
            if orig_a == orig_b {
                continue;
            }

            candidates.push(SharedCandidate {
                kind: SharedKind::SameConstant,
                path_a: path_a.clone(),
                path_b: path_b.clone(),
                description: format!(
                    "Cross-language同名: 「{}」(A) ↔ 「{}」(B) [正規化: {}]",
                    orig_a, orig_b, norm
                ),
                similarity: 0.6,
            });
        }
    }
}

fn collect_normalized_identifiers(
    root: &Path,
    files: &[PathBuf],
) -> HashMap<String, Vec<(String, String)>> {
    let mut result: HashMap<String, Vec<(String, String)>> = HashMap::new();

    for file in files {
        // Skip JSON/config files for identifier extraction
        let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("");
        if matches!(ext, "json" | "toml" | "yaml" | "yml") {
            continue;
        }

        let content = match std::fs::read_to_string(file) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let rel = file.strip_prefix(root).unwrap_or(file).display().to_string();
        let ids = extract_identifiers(&content);

        for (norm, orig) in ids {
            result.entry(norm).or_default().push((orig, rel.clone()));
        }
    }

    // Deduplicate per normalized name
    for locs in result.values_mut() {
        locs.sort_by(|a, b| a.0.cmp(&b.0));
        locs.dedup_by(|a, b| a.0 == b.0);
    }

    result
}

fn is_common_normalized(norm: &str) -> bool {
    const COMMON_NORMS: &[&str] = &[
        "get_value", "set_value", "to_string", "from_string",
        "file_path", "file_name", "base_path", "is_empty",
        "new_error", "parse_error", "read_file", "write_file",
    ];
    COMMON_NORMS.contains(&norm)
}

/// Check if a symbol name is too common to be meaningful
fn is_common_symbol(name: &str) -> bool {
    const COMMON: &[&str] = &[
        "main", "new", "default", "init", "run", "start", "stop", "get", "set", "test", "setup",
        "teardown", "build", "create", "delete", "update", "Default", "Display", "Debug", "Clone",
        "Error", "Result", "App", "Config", "Options", "Settings", "Context", "State",
    ];
    COMMON.contains(&name)
}

/// Calculate content similarity between two files (0.0-1.0)
/// Uses line-based Jaccard similarity
fn content_similarity(path_a: &Path, path_b: &Path) -> f64 {
    let content_a = match std::fs::read_to_string(path_a) {
        Ok(c) => c,
        Err(_) => return 0.0,
    };
    let content_b = match std::fs::read_to_string(path_b) {
        Ok(c) => c,
        Err(_) => return 0.0,
    };

    // Normalize: trim lines, skip empty/comment-only lines
    let lines_a: std::collections::HashSet<String> = content_a
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty() && !l.starts_with("//") && !l.starts_with('#'))
        .collect();

    let lines_b: std::collections::HashSet<String> = content_b
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty() && !l.starts_with("//") && !l.starts_with('#'))
        .collect();

    if lines_a.is_empty() && lines_b.is_empty() {
        return 1.0;
    }
    if lines_a.is_empty() || lines_b.is_empty() {
        return 0.0;
    }

    let intersection = lines_a.intersection(&lines_b).count();
    let union = lines_a.union(&lines_b).count();

    if union == 0 {
        0.0
    } else {
        intersection as f64 / union as f64
    }
}

/// Remove duplicate candidates (same file pair, keep highest similarity)
fn dedup_candidates(candidates: &mut Vec<SharedCandidate>) {
    let mut seen: HashMap<(String, String), usize> = HashMap::new();
    let mut to_remove = Vec::new();

    for (i, c) in candidates.iter().enumerate() {
        let key = (c.path_a.clone(), c.path_b.clone());
        if let Some(&prev_idx) = seen.get(&key) {
            // Keep the one with higher similarity
            if candidates[prev_idx].similarity >= c.similarity {
                to_remove.push(i);
            } else {
                to_remove.push(prev_idx);
                seen.insert(key, i);
            }
        } else {
            seen.insert(key, i);
        }
    }

    // Remove in reverse order to preserve indices
    to_remove.sort_unstable();
    to_remove.dedup();
    for idx in to_remove.into_iter().rev() {
        candidates.remove(idx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_content_similarity_identical() {
        let dir = tempdir().unwrap();
        let fa = dir.path().join("a.rs");
        let fb = dir.path().join("b.rs");
        std::fs::write(&fa, "fn main() {\n    println!(\"hello\");\n}").unwrap();
        std::fs::write(&fb, "fn main() {\n    println!(\"hello\");\n}").unwrap();
        assert!((content_similarity(&fa, &fb) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_content_similarity_different() {
        let dir = tempdir().unwrap();
        let fa = dir.path().join("a.rs");
        let fb = dir.path().join("b.rs");
        std::fs::write(&fa, "fn foo() { 1 }").unwrap();
        std::fs::write(&fb, "fn bar() { 2 }").unwrap();
        assert!(content_similarity(&fa, &fb) < 0.5);
    }

    #[test]
    fn test_content_similarity_missing_file() {
        let dir = tempdir().unwrap();
        let fa = dir.path().join("exists.rs");
        let fb = dir.path().join("missing.rs");
        std::fs::write(&fa, "content").unwrap();
        assert!((content_similarity(&fa, &fb)).abs() < f64::EPSILON);
    }

    #[test]
    fn test_is_common_symbol() {
        assert!(is_common_symbol("main"));
        assert!(is_common_symbol("Default"));
        assert!(!is_common_symbol("calculateTonnage"));
        assert!(!is_common_symbol("TRUCK_SPECS"));
    }

    #[test]
    fn test_find_shared_candidates_empty() {
        let dir_a = tempdir().unwrap();
        let dir_b = tempdir().unwrap();
        let report = find_shared_candidates(dir_a.path(), dir_b.path());
        assert!(report.candidates.is_empty());
        assert_eq!(report.files_scanned_a, 0);
        assert_eq!(report.files_scanned_b, 0);
    }

    #[test]
    fn test_find_shared_same_name_files() {
        let dir_a = tempdir().unwrap();
        let dir_b = tempdir().unwrap();

        std::fs::write(dir_a.path().join("config.rs"), "const X: i32 = 1;").unwrap();
        std::fs::write(dir_b.path().join("config.rs"), "const X: i32 = 1;").unwrap();

        let report = find_shared_candidates(dir_a.path(), dir_b.path());
        assert!(!report.candidates.is_empty());
        assert!(report
            .candidates
            .iter()
            .any(|c| c.kind == SharedKind::SameFileName));
    }

    #[test]
    fn test_dedup_candidates() {
        let mut candidates = vec![
            SharedCandidate {
                kind: SharedKind::SameFileName,
                path_a: "a.rs".to_string(),
                path_b: "b.rs".to_string(),
                description: "test1".to_string(),
                similarity: 0.5,
            },
            SharedCandidate {
                kind: SharedKind::SameExport,
                path_a: "a.rs".to_string(),
                path_b: "b.rs".to_string(),
                description: "test2".to_string(),
                similarity: 0.8,
            },
        ];
        dedup_candidates(&mut candidates);
        assert_eq!(candidates.len(), 1);
        assert!((candidates[0].similarity - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn test_normalize_identifier_snake_case() {
        assert_eq!(normalize_identifier("TRUCK_SPECS"), "truck_specs");
        assert_eq!(normalize_identifier("DEFAULT_BED_AREA"), "default_bed_area");
    }

    #[test]
    fn test_normalize_identifier_camel_case() {
        assert_eq!(normalize_identifier("truckSpecs"), "truck_specs");
        assert_eq!(normalize_identifier("defaultBedArea"), "default_bed_area");
    }

    #[test]
    fn test_normalize_identifier_pascal_case() {
        assert_eq!(normalize_identifier("TruckSpecs"), "truck_specs");
    }

    #[test]
    fn test_extract_identifiers() {
        let code = "const TRUCK_SPECS = {};\nlet fillRatioZ = 0.5;";
        let ids = extract_identifiers(code);
        let norms: Vec<&str> = ids.iter().map(|(n, _)| n.as_str()).collect();
        assert!(norms.contains(&"truck_specs"));
        assert!(norms.contains(&"fill_ratio_z"));
    }

    #[test]
    fn test_cross_language_detection() {
        let dir_a = tempdir().unwrap();
        let dir_b = tempdir().unwrap();

        // Rust style
        std::fs::write(
            dir_a.path().join("specs.rs"),
            "pub const TRUCK_SPECS: &str = \"test\";\npub const MATERIAL_DENSITIES: f64 = 2.5;",
        ).unwrap();

        // TS style
        std::fs::write(
            dir_b.path().join("specs.ts"),
            "export const truckSpecs = {};\nexport const materialDensities = {};",
        ).unwrap();

        let report = find_shared_candidates(dir_a.path(), dir_b.path());
        assert!(report.candidates.iter().any(|c| c.kind == SharedKind::SameConstant));
    }

    #[test]
    fn test_shared_report_to_prompt_string() {
        let report = SharedReport {
            project_a: "/path/a".to_string(),
            project_b: "/path/b".to_string(),
            candidates: vec![SharedCandidate {
                kind: SharedKind::SameFileName,
                path_a: "config.rs".to_string(),
                path_b: "config.rs".to_string(),
                description: "同名ファイル".to_string(),
                similarity: 0.95,
            }],
            files_scanned_a: 10,
            files_scanned_b: 8,
        };
        let prompt = report.to_prompt_string();
        assert!(prompt.contains("config.rs"));
        assert!(prompt.contains("95%"));
    }
}
