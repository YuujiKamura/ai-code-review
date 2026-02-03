//! Python source code parsing with tree-sitter
//!
//! Uses tree-sitter AST node traversal for reliable import/export extraction.
//! Supports:
//! - `import module` (simple import)
//! - `import module as alias` (aliased import)
//! - `import module1, module2` (multiple imports)
//! - `from module import item1, item2` (from import)
//! - `from module import *` (wildcard import)
//! - `from module import item as alias` (aliased from import)

use tree_sitter::Parser;

use crate::error::{CodeReviewError, Result};
use super::{FileAnalysis, ImportInfo};

/// Analyze Python source code to extract imports and exports
pub(crate) fn analyze_python(source: &str) -> Result<FileAnalysis> {
    let mut parser = Parser::new();
    let language = tree_sitter_python::LANGUAGE;
    parser
        .set_language(&language.into())
        .map_err(|e| CodeReviewError::ParseError(format!("Failed to set Python language: {}", e)))?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| CodeReviewError::ParseError("Failed to parse Python source".to_string()))?;

    let mut imports = Vec::new();
    let mut exports = Vec::new();

    let root = tree.root_node();
    let mut cursor = root.walk();

    for node in root.children(&mut cursor) {
        match node.kind() {
            "import_statement" => {
                // import module or import module as alias
                let mut import_infos = extract_import_statement(node, source);
                imports.append(&mut import_infos);
            }
            "import_from_statement" => {
                // from module import item1, item2
                if let Some(import) = extract_import_from_statement(node, source) {
                    imports.push(import);
                }
            }
            "function_definition" | "class_definition" => {
                if let Some(name) = extract_python_def(node, source) {
                    // Python doesn't have explicit exports; collect all top-level defs
                    if !name.starts_with('_') {
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
        language: "python".to_string(),
    })
}

/// Extract import info from import_statement node
///
/// tree-sitter-python import_statement structure:
/// - import_statement
///   - "import" keyword
///   - dotted_name (module name)
///     - identifier ("os")
///     - "." (for nested modules)
///     - identifier ("path")
///   - aliased_import (optional)
///     - dotted_name
///     - "as"
///     - identifier (alias)
///
/// Returns a Vec because `import a, b, c` creates multiple imports
fn extract_import_statement(node: tree_sitter::Node, source: &str) -> Vec<ImportInfo> {
    let mut imports = Vec::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "dotted_name" => {
                // Simple import: import os.path
                let module_path = extract_dotted_name(child, source);
                imports.push(ImportInfo {
                    module_path,
                    items: Vec::new(),
                });
            }
            "aliased_import" => {
                // Aliased import: import os.path as op
                if let Some(import) = extract_aliased_import(child, source) {
                    imports.push(import);
                }
            }
            _ => {}
        }
    }

    imports
}

/// Extract import info from import_from_statement node
///
/// tree-sitter-python import_from_statement structure:
/// - import_from_statement
///   - "from" keyword
///   - dotted_name or relative_import (module name)
///   - "import" keyword
///   - wildcard_import ("*") or named imports
///     - dotted_name (for simple item)
///     - aliased_import (for item as alias)
fn extract_import_from_statement(node: tree_sitter::Node, source: &str) -> Option<ImportInfo> {
    let mut module_path = String::new();
    let mut items = Vec::new();
    let mut found_import_keyword = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            // Module path (before "import" keyword)
            "dotted_name" if !found_import_keyword => {
                module_path = extract_dotted_name(child, source);
            }
            // Relative import: from . import or from .. import or from .module import
            "relative_import" => {
                module_path = extract_relative_import(child, source);
            }
            "import" => {
                found_import_keyword = true;
            }
            // Wildcard import: from module import *
            "wildcard_import" => {
                items.push("*".to_string());
            }
            // Simple item: from module import item
            "dotted_name" if found_import_keyword => {
                let item = extract_dotted_name(child, source);
                items.push(item);
            }
            // Aliased item: from module import item as alias
            "aliased_import" if found_import_keyword => {
                if let Some(item) = extract_aliased_import_item(child, source) {
                    items.push(item);
                }
            }
            _ => {}
        }
    }

    if module_path.is_empty() {
        return None;
    }

    Some(ImportInfo { module_path, items })
}

/// Extract dotted_name as a string (e.g., "os.path" from dotted_name node)
fn extract_dotted_name(node: tree_sitter::Node, source: &str) -> String {
    let mut parts = Vec::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            if let Ok(text) = child.utf8_text(source.as_bytes()) {
                parts.push(text.to_string());
            }
        }
    }

    parts.join(".")
}

/// Extract relative_import path (e.g., "." or ".." or ".module")
fn extract_relative_import(node: tree_sitter::Node, source: &str) -> String {
    let mut prefix = String::new();
    let mut module = String::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "import_prefix" => {
                // Count the dots
                if let Ok(text) = child.utf8_text(source.as_bytes()) {
                    prefix = text.to_string();
                }
            }
            "dotted_name" => {
                module = extract_dotted_name(child, source);
            }
            _ => {}
        }
    }

    format!("{}{}", prefix, module)
}

/// Extract aliased_import for import statement (import module as alias)
/// Returns ImportInfo with the alias as a note
fn extract_aliased_import(node: tree_sitter::Node, source: &str) -> Option<ImportInfo> {
    let mut module_path = String::new();
    let mut alias: Option<String> = None;
    let mut found_as = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "dotted_name" if !found_as => {
                module_path = extract_dotted_name(child, source);
            }
            "as" => {
                found_as = true;
            }
            "identifier" if found_as => {
                if let Ok(text) = child.utf8_text(source.as_bytes()) {
                    alias = Some(text.to_string());
                }
            }
            _ => {}
        }
    }

    if module_path.is_empty() {
        return None;
    }

    // Store the alias in items if present
    let items = if let Some(a) = alias {
        vec![format!("as {}", a)]
    } else {
        Vec::new()
    };

    Some(ImportInfo { module_path, items })
}

/// Extract item name from aliased_import in from-import statement
/// Returns the alias if present, otherwise the original name
fn extract_aliased_import_item(node: tree_sitter::Node, source: &str) -> Option<String> {
    let mut original_name = String::new();
    let mut alias: Option<String> = None;
    let mut found_as = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "dotted_name" if !found_as => {
                original_name = extract_dotted_name(child, source);
            }
            "as" => {
                found_as = true;
            }
            "identifier" if found_as => {
                if let Ok(text) = child.utf8_text(source.as_bytes()) {
                    alias = Some(text.to_string());
                }
            }
            _ => {}
        }
    }

    // Return alias if present, otherwise original name
    Some(alias.unwrap_or(original_name))
}

fn extract_python_def(node: tree_sitter::Node, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            return child.utf8_text(source.as_bytes()).ok().map(String::from);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyze_python_imports() {
        let source = r#"
import os
from typing import List, Optional
from pathlib import Path

def hello():
    pass

class MyClass:
    pass

def _private():
    pass
"#;
        let result = analyze_python(source).unwrap();
        assert_eq!(result.language, "python");
        assert!(!result.imports.is_empty());

        // Check os import
        let os_import = result.imports.iter().find(|i| i.module_path == "os");
        assert!(os_import.is_some());

        // Check typing import
        let typing_import = result.imports.iter().find(|i| i.module_path == "typing");
        assert!(typing_import.is_some());
        let typing_import = typing_import.unwrap();
        assert!(typing_import.items.contains(&"List".to_string()));
        assert!(typing_import.items.contains(&"Optional".to_string()));
    }

    #[test]
    fn test_analyze_python_exports() {
        let source = r#"
def hello():
    pass

class MyClass:
    pass

def _private():
    pass
"#;
        let result = analyze_python(source).unwrap();
        assert!(result.exports.contains(&"hello".to_string()));
        assert!(result.exports.contains(&"MyClass".to_string()));
        // Private functions (starting with _) should not be exported
        assert!(!result.exports.contains(&"_private".to_string()));
    }

    #[test]
    fn test_analyze_python_from_import() {
        let source = "from collections import defaultdict, Counter";
        let result = analyze_python(source).unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].module_path, "collections");
        assert!(result.imports[0].items.contains(&"defaultdict".to_string()));
        assert!(result.imports[0].items.contains(&"Counter".to_string()));
    }

    #[test]
    fn test_analyze_python_simple_import() {
        let source = "import json";
        let result = analyze_python(source).unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].module_path, "json");
        assert!(result.imports[0].items.is_empty());
    }

    // New tests for AST-based parsing

    #[test]
    fn test_import_dotted_module() {
        // Test: import os.path
        let source = "import os.path";
        let result = analyze_python(source).unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].module_path, "os.path");
        assert!(result.imports[0].items.is_empty());
    }

    #[test]
    fn test_import_with_alias() {
        // Test: import numpy as np
        let source = "import numpy as np";
        let result = analyze_python(source).unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].module_path, "numpy");
        assert_eq!(result.imports[0].items, vec!["as np"]);
    }

    #[test]
    fn test_from_import_with_alias() {
        // Test: from typing import List as L
        let source = "from typing import List as L";
        let result = analyze_python(source).unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].module_path, "typing");
        // Should return the alias
        assert_eq!(result.imports[0].items, vec!["L"]);
    }

    #[test]
    fn test_from_import_wildcard() {
        // Test: from module import *
        let source = "from module import *";
        let result = analyze_python(source).unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].module_path, "module");
        assert_eq!(result.imports[0].items, vec!["*"]);
    }

    #[test]
    fn test_from_import_multiple_items() {
        // Test: from typing import List, Dict, Optional
        let source = "from typing import List, Dict, Optional";
        let result = analyze_python(source).unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].module_path, "typing");
        assert_eq!(result.imports[0].items.len(), 3);
        assert!(result.imports[0].items.contains(&"List".to_string()));
        assert!(result.imports[0].items.contains(&"Dict".to_string()));
        assert!(result.imports[0].items.contains(&"Optional".to_string()));
    }

    #[test]
    fn test_relative_import_single_dot() {
        // Test: from . import module
        let source = "from . import module";
        let result = analyze_python(source).unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].module_path, ".");
        assert_eq!(result.imports[0].items, vec!["module"]);
    }

    #[test]
    fn test_relative_import_double_dot() {
        // Test: from .. import parent_module
        let source = "from .. import parent_module";
        let result = analyze_python(source).unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].module_path, "..");
        assert_eq!(result.imports[0].items, vec!["parent_module"]);
    }

    #[test]
    fn test_relative_import_with_module() {
        // Test: from .sibling import something
        let source = "from .sibling import something";
        let result = analyze_python(source).unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].module_path, ".sibling");
        assert_eq!(result.imports[0].items, vec!["something"]);
    }

    #[test]
    fn test_multiple_imports_in_one_statement() {
        // Test: import os, sys, json
        let source = "import os, sys, json";
        let result = analyze_python(source).unwrap();
        assert_eq!(result.imports.len(), 3);

        let modules: Vec<&str> = result.imports.iter().map(|i| i.module_path.as_str()).collect();
        assert!(modules.contains(&"os"));
        assert!(modules.contains(&"sys"));
        assert!(modules.contains(&"json"));
    }

    #[test]
    fn test_mixed_import_patterns() {
        let source = r#"
import os
import numpy as np
from typing import List, Dict
from collections import defaultdict as dd
from . import local_module
from ..parent import something
from module import *
"#;
        let result = analyze_python(source).unwrap();
        assert_eq!(result.imports.len(), 7);

        // Check os import
        let os_import = result.imports.iter().find(|i| i.module_path == "os");
        assert!(os_import.is_some());
        assert!(os_import.unwrap().items.is_empty());

        // Check numpy alias import
        let np_import = result.imports.iter().find(|i| i.module_path == "numpy");
        assert!(np_import.is_some());
        assert!(np_import.unwrap().items.contains(&"as np".to_string()));

        // Check typing import
        let typing_import = result.imports.iter().find(|i| i.module_path == "typing");
        assert!(typing_import.is_some());
        assert!(typing_import.unwrap().items.contains(&"List".to_string()));
        assert!(typing_import.unwrap().items.contains(&"Dict".to_string()));

        // Check aliased from import
        let collections_import = result.imports.iter().find(|i| i.module_path == "collections");
        assert!(collections_import.is_some());
        // Should have alias
        assert!(collections_import.unwrap().items.contains(&"dd".to_string()));

        // Check relative import
        let relative_import = result.imports.iter().find(|i| i.module_path == ".");
        assert!(relative_import.is_some());

        // Check parent relative import
        let parent_import = result.imports.iter().find(|i| i.module_path == "..parent");
        assert!(parent_import.is_some());

        // Check wildcard import
        let wildcard_import = result.imports.iter().find(|i| i.module_path == "module");
        assert!(wildcard_import.is_some());
        assert!(wildcard_import.unwrap().items.contains(&"*".to_string()));
    }

    #[test]
    fn test_from_import_dotted_module() {
        // Test: from xml.etree import ElementTree
        let source = "from xml.etree import ElementTree";
        let result = analyze_python(source).unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].module_path, "xml.etree");
        assert_eq!(result.imports[0].items, vec!["ElementTree"]);
    }
}
