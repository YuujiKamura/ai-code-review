//! Rust source code parsing with tree-sitter

use tree_sitter::Parser;

use crate::error::{CodeReviewError, Result};
use super::{FileAnalysis, ImportInfo};

/// Analyze Rust source code to extract imports and exports
pub(crate) fn analyze_rust(source: &str) -> Result<FileAnalysis> {
    let mut parser = Parser::new();
    let language = tree_sitter_rust::LANGUAGE;
    parser
        .set_language(&language.into())
        .map_err(|e| CodeReviewError::ParseError(format!("Failed to set Rust language: {}", e)))?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| CodeReviewError::ParseError("Failed to parse Rust source".to_string()))?;

    let mut imports = Vec::new();
    let mut exports = Vec::new();

    let root = tree.root_node();
    let mut cursor = root.walk();

    // Traverse top-level nodes
    for node in root.children(&mut cursor) {
        match node.kind() {
            "use_declaration" => {
                if let Some(import) = extract_rust_use(node, source) {
                    imports.push(import);
                }
            }
            "function_item" | "struct_item" | "enum_item" | "type_item" | "const_item"
            | "static_item" | "trait_item" | "impl_item" => {
                if is_public(node, source) {
                    if let Some(name) = extract_item_name(node, source) {
                        exports.push(name);
                    }
                }
            }
            _ => {}
        }
    }

    Ok(FileAnalysis {
        imports,
        exports,
        language: "rust".to_string(),
    })
}

fn extract_rust_use(node: tree_sitter::Node, source: &str) -> Option<ImportInfo> {
    // Traverse use_declaration children to find the actual use target
    // Possible children: "use" keyword, use target (various types), ";" semicolon
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            // use path::to::Item;
            "scoped_identifier" => {
                return extract_from_scoped_identifier(child, source);
            }
            // use path::to::{Item1, Item2};
            "scoped_use_list" => {
                return extract_from_scoped_use_list(child, source);
            }
            // use super::*; or use crate::*;
            "use_wildcard" => {
                return extract_from_use_wildcard(child, source);
            }
            // use path::to::Item as Alias;
            "use_as_clause" => {
                return extract_from_use_as_clause(child, source);
            }
            // use module; (simple identifier)
            "identifier" => {
                let name = child.utf8_text(source.as_bytes()).ok()?;
                return Some(ImportInfo {
                    module_path: String::new(),
                    items: vec![name.to_string()],
                });
            }
            // use crate; or use self; or use super;
            "crate" | "self" | "super" => {
                let name = child.utf8_text(source.as_bytes()).ok()?;
                return Some(ImportInfo {
                    module_path: name.to_string(),
                    items: Vec::new(),
                });
            }
            // use {Item1, Item2}; (use_list at top level)
            "use_list" => {
                let items = extract_use_list_items(child, source);
                return Some(ImportInfo {
                    module_path: String::new(),
                    items,
                });
            }
            _ => {}
        }
    }
    None
}

/// Extract import info from scoped_identifier (e.g., std::path::Path)
fn extract_from_scoped_identifier(node: tree_sitter::Node, source: &str) -> Option<ImportInfo> {
    // scoped_identifier contains: path (scoped_identifier or identifier), "::", identifier
    // We want to split into module_path and the final item
    let mut path_parts = Vec::new();
    collect_scoped_identifier_parts(node, source, &mut path_parts);

    if path_parts.is_empty() {
        return None;
    }

    // Last part is the imported item, rest is the module path
    let item = path_parts.pop()?;
    let module_path = path_parts.join("::");

    Some(ImportInfo {
        module_path,
        items: vec![item],
    })
}

/// Recursively collect all parts of a scoped_identifier
fn collect_scoped_identifier_parts(node: tree_sitter::Node, source: &str, parts: &mut Vec<String>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "scoped_identifier" => {
                collect_scoped_identifier_parts(child, source, parts);
            }
            "identifier" | "crate" | "self" | "super" => {
                if let Ok(text) = child.utf8_text(source.as_bytes()) {
                    parts.push(text.to_string());
                }
            }
            _ => {}
        }
    }
}

/// Extract import info from scoped_use_list (e.g., std::io::{Read, Write})
fn extract_from_scoped_use_list(node: tree_sitter::Node, source: &str) -> Option<ImportInfo> {
    let mut module_parts = Vec::new();
    let mut items = Vec::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "scoped_identifier" => {
                collect_scoped_identifier_parts(child, source, &mut module_parts);
            }
            "identifier" | "crate" | "self" | "super" => {
                if let Ok(text) = child.utf8_text(source.as_bytes()) {
                    module_parts.push(text.to_string());
                }
            }
            "use_list" => {
                items = extract_use_list_items(child, source);
            }
            _ => {}
        }
    }

    let module_path = module_parts.join("::");
    Some(ImportInfo { module_path, items })
}

/// Extract items from use_list (e.g., {Read, Write, self})
fn extract_use_list_items(node: tree_sitter::Node, source: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        match child.kind() {
            "identifier" | "self" => {
                if let Ok(text) = child.utf8_text(source.as_bytes()) {
                    items.push(text.to_string());
                }
            }
            // Handle nested use_as_clause within use_list: {Item as Alias}
            "use_as_clause" => {
                if let Some(alias) = extract_alias_from_use_as_clause(child, source) {
                    items.push(alias);
                }
            }
            // Handle nested scoped paths within use_list: {sub::Item}
            "scoped_identifier" => {
                // For nested scoped identifiers, get the full path as an item
                if let Ok(text) = child.utf8_text(source.as_bytes()) {
                    items.push(text.to_string());
                }
            }
            _ => {}
        }
    }
    items
}

/// Extract import info from use_wildcard (e.g., super::*)
fn extract_from_use_wildcard(node: tree_sitter::Node, source: &str) -> Option<ImportInfo> {
    let mut path_parts = Vec::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "scoped_identifier" => {
                collect_scoped_identifier_parts(child, source, &mut path_parts);
            }
            "identifier" | "crate" | "self" | "super" => {
                if let Ok(text) = child.utf8_text(source.as_bytes()) {
                    path_parts.push(text.to_string());
                }
            }
            "*" => {
                // Wildcard import
            }
            _ => {}
        }
    }

    let module_path = path_parts.join("::");
    Some(ImportInfo {
        module_path,
        items: vec!["*".to_string()],
    })
}

/// Extract import info from use_as_clause (e.g., path::Item as Alias)
fn extract_from_use_as_clause(node: tree_sitter::Node, source: &str) -> Option<ImportInfo> {
    let mut path_parts = Vec::new();
    let mut alias: Option<String> = None;

    let mut cursor = node.walk();
    let mut found_as = false;

    for child in node.children(&mut cursor) {
        match child.kind() {
            "scoped_identifier" if !found_as => {
                collect_scoped_identifier_parts(child, source, &mut path_parts);
            }
            "identifier" => {
                if found_as {
                    // This is the alias
                    if let Ok(text) = child.utf8_text(source.as_bytes()) {
                        alias = Some(text.to_string());
                    }
                } else {
                    // This is part of the path (simple identifier before 'as')
                    if let Ok(text) = child.utf8_text(source.as_bytes()) {
                        path_parts.push(text.to_string());
                    }
                }
            }
            "crate" | "self" | "super" if !found_as => {
                if let Ok(text) = child.utf8_text(source.as_bytes()) {
                    path_parts.push(text.to_string());
                }
            }
            "as" => {
                found_as = true;
            }
            _ => {}
        }
    }

    // The last part before 'as' is the original item, rest is module_path
    // Use the alias as the imported item name
    if path_parts.is_empty() {
        return None;
    }

    let _original_item = path_parts.pop()?;
    let module_path = path_parts.join("::");
    let imported_name = alias.unwrap_or(_original_item);

    Some(ImportInfo {
        module_path,
        items: vec![imported_name],
    })
}

/// Extract alias from use_as_clause within a use_list
fn extract_alias_from_use_as_clause(node: tree_sitter::Node, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    let mut found_as = false;

    for child in node.children(&mut cursor) {
        match child.kind() {
            "as" => {
                found_as = true;
            }
            "identifier" if found_as => {
                return child.utf8_text(source.as_bytes()).ok().map(String::from);
            }
            _ => {}
        }
    }

    // If no alias found, return the original identifier
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            return child.utf8_text(source.as_bytes()).ok().map(String::from);
        }
    }
    None
}

fn is_public(node: tree_sitter::Node, _source: &str) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "visibility_modifier" {
            return true;
        }
    }
    false
}

fn extract_item_name(node: tree_sitter::Node, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" || child.kind() == "type_identifier" {
            return child.utf8_text(source.as_bytes()).ok().map(String::from);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyze_rust() {
        let source = r#"
use std::path::Path;
use crate::error::{Error, Result};

pub fn hello() {}
pub struct Foo;
fn private() {}
"#;
        let result = analyze_rust(source).unwrap();
        assert_eq!(result.language, "rust");
        assert!(!result.imports.is_empty());
        assert!(result.exports.contains(&"hello".to_string()));
        assert!(result.exports.contains(&"Foo".to_string()));
        assert!(!result.exports.contains(&"private".to_string()));
    }

    #[test]
    fn test_extract_rust_use_scoped_identifier() {
        // Test: use std::path::Path;
        let source = "use std::path::Path;";
        let result = analyze_rust(source).unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].module_path, "std::path");
        assert_eq!(result.imports[0].items, vec!["Path"]);
    }

    #[test]
    fn test_extract_rust_use_scoped_use_list() {
        // Test: use crate::error::{Error, Result};
        let source = "use crate::error::{Error, Result};";
        let result = analyze_rust(source).unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].module_path, "crate::error");
        assert_eq!(result.imports[0].items, vec!["Error", "Result"]);
    }

    #[test]
    fn test_extract_rust_use_wildcard() {
        // Test: use super::*;
        let source = "use super::*;";
        let result = analyze_rust(source).unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].module_path, "super");
        assert_eq!(result.imports[0].items, vec!["*"]);
    }

    #[test]
    fn test_extract_rust_use_as_clause() {
        // Test: use crate::foo::bar as baz;
        let source = "use crate::foo::bar as baz;";
        let result = analyze_rust(source).unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].module_path, "crate::foo");
        assert_eq!(result.imports[0].items, vec!["baz"]);
    }

    #[test]
    fn test_extract_rust_use_with_self() {
        // Test: use std::io::{self, Read, Write};
        let source = "use std::io::{self, Read, Write};";
        let result = analyze_rust(source).unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].module_path, "std::io");
        assert!(result.imports[0].items.contains(&"self".to_string()));
        assert!(result.imports[0].items.contains(&"Read".to_string()));
        assert!(result.imports[0].items.contains(&"Write".to_string()));
    }

    #[test]
    fn test_extract_rust_use_collections() {
        // Test: use std::collections::{HashMap, HashSet};
        let source = "use std::collections::{HashMap, HashSet};";
        let result = analyze_rust(source).unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].module_path, "std::collections");
        assert_eq!(result.imports[0].items, vec!["HashMap", "HashSet"]);
    }

    #[test]
    fn test_analyze_rust_attributed_pub_items() {
        let source = r#"
#[derive(Debug, Clone)]
pub struct Attributed;

#[inline]
pub fn attributed_fn() {}

/// Doc comment
pub enum DocEnum { A, B }

fn private() {}
"#;
        let result = analyze_rust(source).unwrap();
        assert!(result.exports.contains(&"Attributed".to_string()));
        assert!(result.exports.contains(&"attributed_fn".to_string()));
        assert!(result.exports.contains(&"DocEnum".to_string()));
        assert!(!result.exports.contains(&"private".to_string()));
    }
}
