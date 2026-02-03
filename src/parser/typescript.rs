//! TypeScript/JavaScript source code parsing with tree-sitter
//!
//! Uses tree-sitter AST node traversal for reliable import/export extraction.
//! Supports:
//! - `import { A, B } from 'module'` (named imports)
//! - `import A from 'module'` (default import)
//! - `import * as A from 'module'` (namespace import)
//! - `import 'module'` (side-effect import)

use tree_sitter::Parser;

use crate::error::{CodeReviewError, Result};
use super::{FileAnalysis, ImportInfo};

/// Analyze TypeScript/JavaScript source code to extract imports and exports
pub(crate) fn analyze_typescript(source: &str) -> Result<FileAnalysis> {
    let mut parser = Parser::new();
    let language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT;
    parser.set_language(&language.into()).map_err(|e| {
        CodeReviewError::ParseError(format!("Failed to set TypeScript language: {}", e))
    })?;

    let tree = parser.parse(source, None).ok_or_else(|| {
        CodeReviewError::ParseError("Failed to parse TypeScript source".to_string())
    })?;

    let mut imports = Vec::new();
    let mut exports = Vec::new();

    let root = tree.root_node();
    let mut cursor = root.walk();

    for node in root.children(&mut cursor) {
        match node.kind() {
            "import_statement" => {
                if let Some(import) = extract_ts_import(node, source) {
                    imports.push(import);
                }
            }
            "export_statement" => {
                if let Some(name) = extract_ts_export(node, source) {
                    exports.push(name);
                }
            }
            _ => {}
        }
    }

    Ok(FileAnalysis {
        imports,
        exports,
        language: "typescript".to_string(),
    })
}

/// Extract import information from an import_statement node by traversing AST
///
/// tree-sitter-typescript import_statement structure:
/// - import_statement
///   - "import" keyword
///   - import_clause (optional)
///     - identifier (default import)
///     - named_imports
///       - import_specifier
///         - identifier (original name)
///         - "as" (optional)
///         - identifier (alias, optional)
///     - namespace_import
///       - "*"
///       - "as"
///       - identifier
///   - "from" (optional)
///   - string (module path)
fn extract_ts_import(node: tree_sitter::Node, source: &str) -> Option<ImportInfo> {
    let mut module_path = String::new();
    let mut items = Vec::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            // Module path: string literal
            "string" => {
                module_path = extract_string_content(child, source);
            }
            // Import clause containing the imported items
            "import_clause" => {
                extract_import_clause_items(child, source, &mut items);
            }
            _ => {}
        }
    }

    // Handle side-effect imports: import 'module'
    if module_path.is_empty() {
        return None;
    }

    Some(ImportInfo { module_path, items })
}

/// Extract items from import_clause node
fn extract_import_clause_items(node: tree_sitter::Node, source: &str, items: &mut Vec<String>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            // Default import: import Foo from 'module'
            "identifier" => {
                if let Ok(text) = child.utf8_text(source.as_bytes()) {
                    items.push(text.to_string());
                }
            }
            // Named imports: import { A, B } from 'module'
            "named_imports" => {
                extract_named_imports(child, source, items);
            }
            // Namespace import: import * as Foo from 'module'
            "namespace_import" => {
                extract_namespace_import(child, source, items);
            }
            _ => {}
        }
    }
}

/// Extract items from named_imports node: { A, B, C as D }
fn extract_named_imports(node: tree_sitter::Node, source: &str, items: &mut Vec<String>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "import_specifier" {
            if let Some(name) = extract_import_specifier(child, source) {
                items.push(name);
            }
        }
    }
}

/// Extract name from import_specifier node
/// Returns the alias if present, otherwise the original name
fn extract_import_specifier(node: tree_sitter::Node, source: &str) -> Option<String> {
    let mut original_name: Option<String> = None;
    let mut alias: Option<String> = None;
    let mut found_as = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "identifier" => {
                if let Ok(text) = child.utf8_text(source.as_bytes()) {
                    if found_as {
                        alias = Some(text.to_string());
                    } else {
                        original_name = Some(text.to_string());
                    }
                }
            }
            "as" => {
                found_as = true;
            }
            _ => {}
        }
    }

    // Return alias if present, otherwise original name
    alias.or(original_name)
}

/// Extract name from namespace_import node: * as Foo
fn extract_namespace_import(node: tree_sitter::Node, source: &str, items: &mut Vec<String>) {
    let mut cursor = node.walk();
    let mut found_as = false;

    for child in node.children(&mut cursor) {
        match child.kind() {
            "as" => {
                found_as = true;
            }
            "identifier" if found_as => {
                if let Ok(text) = child.utf8_text(source.as_bytes()) {
                    // Store as "* as Name" to indicate namespace import
                    items.push(format!("* as {}", text));
                }
            }
            _ => {}
        }
    }
}

/// Extract string content from a string literal node, removing quotes
fn extract_string_content(node: tree_sitter::Node, source: &str) -> String {
    if let Ok(text) = node.utf8_text(source.as_bytes()) {
        // Remove surrounding quotes (single or double)
        text.trim_matches(|c| c == '\'' || c == '"').to_string()
    } else {
        String::new()
    }
}

fn extract_ts_export(node: tree_sitter::Node, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "function_declaration"
            || child.kind() == "class_declaration"
            || child.kind() == "interface_declaration"
        {
            let mut inner_cursor = child.walk();
            for inner_child in child.children(&mut inner_cursor) {
                if inner_child.kind() == "identifier" || inner_child.kind() == "type_identifier" {
                    return inner_child.utf8_text(source.as_bytes()).ok().map(String::from);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyze_typescript_imports() {
        let source = r#"
import { useState, useEffect } from 'react';
import axios from 'axios';

export function MyComponent() {}
"#;
        let result = analyze_typescript(source).unwrap();
        assert_eq!(result.language, "typescript");
        assert!(!result.imports.is_empty());

        // Check first import
        let react_import = result.imports.iter().find(|i| i.module_path == "react");
        assert!(react_import.is_some());
        let react_import = react_import.unwrap();
        assert!(react_import.items.contains(&"useState".to_string()));
        assert!(react_import.items.contains(&"useEffect".to_string()));
    }

    #[test]
    fn test_analyze_typescript_exports() {
        let source = r#"
export function hello() {}
export class MyClass {}
export interface MyInterface {}
"#;
        let result = analyze_typescript(source).unwrap();
        assert!(result.exports.contains(&"hello".to_string()));
        assert!(result.exports.contains(&"MyClass".to_string()));
        assert!(result.exports.contains(&"MyInterface".to_string()));
    }

    #[test]
    fn test_analyze_typescript_single_quotes() {
        let source = "import { foo } from 'bar';";
        let result = analyze_typescript(source).unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].module_path, "bar");
        assert_eq!(result.imports[0].items, vec!["foo"]);
    }

    #[test]
    fn test_analyze_typescript_double_quotes() {
        let source = r#"import { foo } from "bar";"#;
        let result = analyze_typescript(source).unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].module_path, "bar");
        assert_eq!(result.imports[0].items, vec!["foo"]);
    }

    // New tests for AST-based parsing

    #[test]
    fn test_named_imports() {
        // Test: import { A, B, C } from 'module'
        let source = "import { A, B, C } from 'module';";
        let result = analyze_typescript(source).unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].module_path, "module");
        assert_eq!(result.imports[0].items.len(), 3);
        assert!(result.imports[0].items.contains(&"A".to_string()));
        assert!(result.imports[0].items.contains(&"B".to_string()));
        assert!(result.imports[0].items.contains(&"C".to_string()));
    }

    #[test]
    fn test_default_import() {
        // Test: import React from 'react'
        let source = "import React from 'react';";
        let result = analyze_typescript(source).unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].module_path, "react");
        assert_eq!(result.imports[0].items, vec!["React"]);
    }

    #[test]
    fn test_namespace_import() {
        // Test: import * as utils from 'utils'
        let source = "import * as utils from 'utils';";
        let result = analyze_typescript(source).unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].module_path, "utils");
        assert_eq!(result.imports[0].items, vec!["* as utils"]);
    }

    #[test]
    fn test_aliased_import() {
        // Test: import { foo as bar } from 'module'
        let source = "import { foo as bar } from 'module';";
        let result = analyze_typescript(source).unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].module_path, "module");
        // Should return the alias name
        assert_eq!(result.imports[0].items, vec!["bar"]);
    }

    #[test]
    fn test_mixed_imports() {
        // Test: import Default, { named1, named2 as alias } from 'module'
        let source = "import Default, { named1, named2 as alias } from 'module';";
        let result = analyze_typescript(source).unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].module_path, "module");
        assert!(result.imports[0].items.contains(&"Default".to_string()));
        assert!(result.imports[0].items.contains(&"named1".to_string()));
        assert!(result.imports[0].items.contains(&"alias".to_string()));
    }

    #[test]
    fn test_side_effect_import() {
        // Test: import 'module' (side effect only)
        let source = "import 'polyfill';";
        let result = analyze_typescript(source).unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].module_path, "polyfill");
        assert!(result.imports[0].items.is_empty());
    }

    #[test]
    fn test_multiple_imports() {
        let source = r#"
import React from 'react';
import { useState, useEffect } from 'react';
import * as lodash from 'lodash';
import 'styles.css';
"#;
        let result = analyze_typescript(source).unwrap();
        assert_eq!(result.imports.len(), 4);

        // Check React default import
        let react_default = result.imports.iter().find(|i| i.items.contains(&"React".to_string()));
        assert!(react_default.is_some());

        // Check React hooks import
        let react_hooks = result.imports.iter().find(|i| i.items.contains(&"useState".to_string()));
        assert!(react_hooks.is_some());
        assert!(react_hooks.unwrap().items.contains(&"useEffect".to_string()));

        // Check lodash namespace import
        let lodash_import = result.imports.iter().find(|i| i.items.contains(&"* as lodash".to_string()));
        assert!(lodash_import.is_some());

        // Check side-effect import
        let css_import = result.imports.iter().find(|i| i.module_path == "styles.css");
        assert!(css_import.is_some());
        assert!(css_import.unwrap().items.is_empty());
    }

    #[test]
    fn test_scoped_package_import() {
        // Test: import { something } from '@scope/package'
        let source = "import { something } from '@scope/package';";
        let result = analyze_typescript(source).unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].module_path, "@scope/package");
        assert_eq!(result.imports[0].items, vec!["something"]);
    }

    #[test]
    fn test_relative_path_import() {
        // Test: import { Component } from './components/Component'
        let source = "import { Component } from './components/Component';";
        let result = analyze_typescript(source).unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].module_path, "./components/Component");
        assert_eq!(result.imports[0].items, vec!["Component"]);
    }
}
