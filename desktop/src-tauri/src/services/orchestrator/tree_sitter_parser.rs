//! Tree-sitter Based Code Parsing
//!
//! Provides accurate, language-aware symbol extraction using tree-sitter grammars.
//! Supports Python, Rust, TypeScript, JavaScript, Go, and Java.
//! Falls back gracefully when a language is not supported.

use super::analysis_index::{SymbolInfo, SymbolKind};

/// Check whether tree-sitter parsing is available for the given language.
pub fn is_language_supported(language: &str) -> bool {
    matches!(
        language,
        "python" | "rust" | "typescript" | "javascript" | "go" | "java"
    )
}

/// Parse source code and extract symbols using tree-sitter.
///
/// Returns a vector of `SymbolInfo` with rich metadata including parent scope,
/// signatures, doc comments, and line ranges.
///
/// The `max_symbols` parameter limits the number of returned symbols.
pub fn parse_symbols(content: &str, language: &str, max_symbols: usize) -> Vec<SymbolInfo> {
    let lang = match language {
        "python" => tree_sitter_python::LANGUAGE.into(),
        "rust" => tree_sitter_rust::LANGUAGE.into(),
        "typescript" => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        "javascript" => tree_sitter_typescript::LANGUAGE_TSX.into(), // TSX grammar handles JS well
        "go" => tree_sitter_go::LANGUAGE.into(),
        "java" => tree_sitter_java::LANGUAGE.into(),
        _ => return Vec::new(),
    };

    let mut parser = tree_sitter::Parser::new();
    if parser.set_language(&lang).is_err() {
        return Vec::new();
    }

    let tree = match parser.parse(content, None) {
        Some(t) => t,
        None => return Vec::new(),
    };

    let lines: Vec<&str> = content.lines().collect();
    let mut symbols = Vec::new();

    extract_from_node(
        tree.root_node(),
        content,
        &lines,
        language,
        None,
        &mut symbols,
        max_symbols,
    );

    symbols
}

/// Recursively extract symbols from a tree-sitter node.
fn extract_from_node(
    node: tree_sitter::Node,
    source: &str,
    lines: &[&str],
    language: &str,
    parent_name: Option<&str>,
    symbols: &mut Vec<SymbolInfo>,
    max_symbols: usize,
) {
    if symbols.len() >= max_symbols {
        return;
    }

    let kind = node.kind();

    // Try to extract a symbol from the current node
    if let Some(sym) = try_extract_symbol(node, source, lines, language, parent_name) {
        let new_parent = sym.name.clone();
        symbols.push(sym);

        // For class/struct/enum/interface bodies, recurse with the parent name
        if is_container_node(kind, language) {
            for i in 0..node.child_count() {
                if symbols.len() >= max_symbols {
                    return;
                }
                if let Some(child) = node.child(i) {
                    extract_from_node(
                        child,
                        source,
                        lines,
                        language,
                        Some(&new_parent),
                        symbols,
                        max_symbols,
                    );
                }
            }
            return; // Don't double-recurse
        }
    }

    // Recurse into children
    for i in 0..node.child_count() {
        if symbols.len() >= max_symbols {
            return;
        }
        if let Some(child) = node.child(i) {
            extract_from_node(child, source, lines, language, parent_name, symbols, max_symbols);
        }
    }
}

/// Check if a node kind represents a container (class, struct, etc.) whose children
/// should be tagged with the container as parent.
fn is_container_node(kind: &str, language: &str) -> bool {
    match language {
        "python" => kind == "class_definition",
        "rust" => matches!(kind, "struct_item" | "enum_item" | "trait_item" | "impl_item"),
        "typescript" | "javascript" => matches!(kind, "class_declaration" | "interface_declaration"),
        "go" => false, // Go methods are at file scope via receiver syntax
        "java" => matches!(
            kind,
            "class_declaration" | "interface_declaration" | "enum_declaration"
        ),
        _ => false,
    }
}

/// Try to extract a SymbolInfo from a tree-sitter node.
/// Returns None if the node does not represent a symbol we care about.
fn try_extract_symbol(
    node: tree_sitter::Node,
    source: &str,
    lines: &[&str],
    language: &str,
    parent_name: Option<&str>,
) -> Option<SymbolInfo> {
    let kind = node.kind();
    let start_line = node.start_position().row + 1; // 1-based
    let end_line = node.end_position().row + 1;

    match language {
        "python" => extract_python_symbol(node, kind, source, lines, start_line, end_line, parent_name),
        "rust" => extract_rust_symbol(node, kind, source, lines, start_line, end_line, parent_name),
        "typescript" | "javascript" => {
            extract_ts_symbol(node, kind, source, lines, start_line, end_line, parent_name)
        }
        "go" => extract_go_symbol(node, kind, source, lines, start_line, end_line, parent_name),
        "java" => extract_java_symbol(node, kind, source, lines, start_line, end_line, parent_name),
        _ => None,
    }
}

// ============================================================================
// Python
// ============================================================================

fn extract_python_symbol(
    node: tree_sitter::Node,
    kind: &str,
    source: &str,
    lines: &[&str],
    start_line: usize,
    end_line: usize,
    parent_name: Option<&str>,
) -> Option<SymbolInfo> {
    match kind {
        "function_definition" => {
            let name = find_child_text(node, "name", source)?;
            let sig = get_line_text(lines, start_line);
            let doc = get_python_docstring(node, source);
            Some(SymbolInfo {
                name,
                kind: SymbolKind::Function,
                line: start_line,
                parent: parent_name.map(|s| s.to_string()),
                signature: sig,
                doc_comment: doc,
                end_line,
            })
        }
        "class_definition" => {
            let name = find_child_text(node, "name", source)?;
            let sig = get_line_text(lines, start_line);
            let doc = get_python_docstring(node, source);
            Some(SymbolInfo {
                name,
                kind: SymbolKind::Class,
                line: start_line,
                parent: parent_name.map(|s| s.to_string()),
                signature: sig,
                doc_comment: doc,
                end_line,
            })
        }
        _ => None,
    }
}

/// Extract Python docstring from the first expression_statement in a body.
fn get_python_docstring(node: tree_sitter::Node, source: &str) -> Option<String> {
    // Look for a `block` child, then its first `expression_statement` with a `string`
    for i in 0..node.child_count() {
        let child = node.child(i)?;
        if child.kind() == "block" {
            for j in 0..child.child_count() {
                let stmt = child.child(j)?;
                if stmt.kind() == "expression_statement" {
                    for k in 0..stmt.child_count() {
                        let expr = stmt.child(k)?;
                        if expr.kind() == "string" {
                            let text = node_text(expr, source);
                            return Some(clean_docstring(&text));
                        }
                    }
                }
                // Only first statement can be a docstring
                break;
            }
        }
    }
    None
}

// ============================================================================
// Rust
// ============================================================================

fn extract_rust_symbol(
    node: tree_sitter::Node,
    kind: &str,
    source: &str,
    lines: &[&str],
    start_line: usize,
    end_line: usize,
    parent_name: Option<&str>,
) -> Option<SymbolInfo> {
    match kind {
        "function_item" => {
            let name = find_child_text(node, "name", source)?;
            let sig = get_line_text(lines, start_line);
            let doc = get_rust_doc_comment(lines, start_line);
            Some(SymbolInfo {
                name,
                kind: SymbolKind::Function,
                line: start_line,
                parent: parent_name.map(|s| s.to_string()),
                signature: sig,
                doc_comment: doc,
                end_line,
            })
        }
        "struct_item" => {
            let name = find_child_text(node, "name", source)?;
            let sig = get_line_text(lines, start_line);
            let doc = get_rust_doc_comment(lines, start_line);
            Some(SymbolInfo {
                name,
                kind: SymbolKind::Struct,
                line: start_line,
                parent: parent_name.map(|s| s.to_string()),
                signature: sig,
                doc_comment: doc,
                end_line,
            })
        }
        "enum_item" => {
            let name = find_child_text(node, "name", source)?;
            let sig = get_line_text(lines, start_line);
            let doc = get_rust_doc_comment(lines, start_line);
            Some(SymbolInfo {
                name,
                kind: SymbolKind::Enum,
                line: start_line,
                parent: parent_name.map(|s| s.to_string()),
                signature: sig,
                doc_comment: doc,
                end_line,
            })
        }
        "trait_item" => {
            let name = find_child_text(node, "name", source)?;
            let sig = get_line_text(lines, start_line);
            let doc = get_rust_doc_comment(lines, start_line);
            Some(SymbolInfo {
                name,
                kind: SymbolKind::Interface,
                line: start_line,
                parent: parent_name.map(|s| s.to_string()),
                signature: sig,
                doc_comment: doc,
                end_line,
            })
        }
        "type_item" => {
            let name = find_child_text(node, "name", source)?;
            let sig = get_line_text(lines, start_line);
            let doc = get_rust_doc_comment(lines, start_line);
            Some(SymbolInfo {
                name,
                kind: SymbolKind::Type,
                line: start_line,
                parent: parent_name.map(|s| s.to_string()),
                signature: sig,
                doc_comment: doc,
                end_line,
            })
        }
        "mod_item" => {
            let name = find_child_text(node, "name", source)?;
            let sig = get_line_text(lines, start_line);
            Some(SymbolInfo {
                name,
                kind: SymbolKind::Module,
                line: start_line,
                parent: parent_name.map(|s| s.to_string()),
                signature: sig,
                doc_comment: None,
                end_line,
            })
        }
        "const_item" | "static_item" => {
            let name = find_child_text(node, "name", source)?;
            let sig = get_line_text(lines, start_line);
            let doc = get_rust_doc_comment(lines, start_line);
            Some(SymbolInfo {
                name,
                kind: SymbolKind::Const,
                line: start_line,
                parent: parent_name.map(|s| s.to_string()),
                signature: sig,
                doc_comment: doc,
                end_line,
            })
        }
        "impl_item" => {
            // For impl blocks, we want the type name as parent for nested items.
            // We don't emit the impl itself as a symbol.
            None
        }
        _ => None,
    }
}

/// Extract Rust doc comments (/// or //!) from lines preceding a definition.
fn get_rust_doc_comment(lines: &[&str], def_line: usize) -> Option<String> {
    if def_line < 2 {
        return None;
    }
    let mut doc_lines = Vec::new();
    let mut line_idx = def_line - 2; // 0-based index of line before definition
    loop {
        if line_idx >= lines.len() {
            break;
        }
        let trimmed = lines[line_idx].trim();
        if let Some(rest) = trimmed.strip_prefix("///") {
            doc_lines.push(rest.trim().to_string());
        } else if let Some(rest) = trimmed.strip_prefix("//!") {
            doc_lines.push(rest.trim().to_string());
        } else {
            break;
        }
        if line_idx == 0 {
            break;
        }
        line_idx -= 1;
    }
    if doc_lines.is_empty() {
        return None;
    }
    doc_lines.reverse();
    Some(doc_lines.join(" ").trim().to_string())
}

// ============================================================================
// TypeScript / JavaScript
// ============================================================================

fn extract_ts_symbol(
    node: tree_sitter::Node,
    kind: &str,
    source: &str,
    lines: &[&str],
    start_line: usize,
    end_line: usize,
    parent_name: Option<&str>,
) -> Option<SymbolInfo> {
    match kind {
        "function_declaration" => {
            let name = find_child_text(node, "name", source)?;
            let sig = get_line_text(lines, start_line);
            let doc = get_jsdoc_comment(lines, start_line);
            Some(SymbolInfo {
                name,
                kind: SymbolKind::Function,
                line: start_line,
                parent: parent_name.map(|s| s.to_string()),
                signature: sig,
                doc_comment: doc,
                end_line,
            })
        }
        "class_declaration" => {
            let name = find_child_text(node, "name", source)?;
            let sig = get_line_text(lines, start_line);
            let doc = get_jsdoc_comment(lines, start_line);
            Some(SymbolInfo {
                name,
                kind: SymbolKind::Class,
                line: start_line,
                parent: parent_name.map(|s| s.to_string()),
                signature: sig,
                doc_comment: doc,
                end_line,
            })
        }
        "interface_declaration" => {
            let name = find_child_text(node, "name", source)?;
            let sig = get_line_text(lines, start_line);
            let doc = get_jsdoc_comment(lines, start_line);
            Some(SymbolInfo {
                name,
                kind: SymbolKind::Interface,
                line: start_line,
                parent: parent_name.map(|s| s.to_string()),
                signature: sig,
                doc_comment: doc,
                end_line,
            })
        }
        "type_alias_declaration" => {
            let name = find_child_text(node, "name", source)?;
            let sig = get_line_text(lines, start_line);
            Some(SymbolInfo {
                name,
                kind: SymbolKind::Type,
                line: start_line,
                parent: parent_name.map(|s| s.to_string()),
                signature: sig,
                doc_comment: None,
                end_line,
            })
        }
        "enum_declaration" => {
            let name = find_child_text(node, "name", source)?;
            let sig = get_line_text(lines, start_line);
            Some(SymbolInfo {
                name,
                kind: SymbolKind::Enum,
                line: start_line,
                parent: parent_name.map(|s| s.to_string()),
                signature: sig,
                doc_comment: None,
                end_line,
            })
        }
        "method_definition" => {
            let name = find_child_text(node, "name", source)?;
            let sig = get_line_text(lines, start_line);
            Some(SymbolInfo {
                name,
                kind: SymbolKind::Function,
                line: start_line,
                parent: parent_name.map(|s| s.to_string()),
                signature: sig,
                doc_comment: None,
                end_line,
            })
        }
        "lexical_declaration" | "variable_declaration" => {
            // export const FOO = ... pattern
            // Look for a variable_declarator child
            for i in 0..node.child_count() {
                if let Some(child) = node.child(i) {
                    if child.kind() == "variable_declarator" {
                        if let Some(name_node) = child.child_by_field_name("name") {
                            let name = node_text(name_node, source);
                            if !name.is_empty() {
                                let sig = get_line_text(lines, start_line);
                                return Some(SymbolInfo {
                                    name,
                                    kind: SymbolKind::Const,
                                    line: start_line,
                                    parent: parent_name.map(|s| s.to_string()),
                                    signature: sig,
                                    doc_comment: None,
                                    end_line,
                                });
                            }
                        }
                    }
                }
            }
            None
        }
        _ => None,
    }
}

/// Extract JSDoc-style comments (/** ... */) from lines preceding a definition.
fn get_jsdoc_comment(lines: &[&str], def_line: usize) -> Option<String> {
    if def_line < 2 {
        return None;
    }
    // Check if the line before the definition ends with */
    let prev_line = lines.get(def_line - 2)?; // 0-based
    let trimmed = prev_line.trim();
    if !trimmed.ends_with("*/") {
        return None;
    }
    // Walk backwards to find the start of the comment
    let mut doc_lines = Vec::new();
    let mut idx = def_line - 2;
    loop {
        if idx >= lines.len() {
            break;
        }
        let line = lines[idx].trim();
        doc_lines.push(line.to_string());
        if line.starts_with("/**") || line.starts_with("/*") {
            break;
        }
        if idx == 0 {
            break;
        }
        idx -= 1;
    }
    if doc_lines.is_empty() {
        return None;
    }
    doc_lines.reverse();
    // Clean up JSDoc markers
    let cleaned: Vec<String> = doc_lines
        .iter()
        .map(|l| {
            l.trim_start_matches("/**")
                .trim_start_matches("/*")
                .trim_end_matches("*/")
                .trim_start_matches('*')
                .trim()
                .to_string()
        })
        .filter(|l| !l.is_empty() && !l.starts_with('@'))
        .collect();
    if cleaned.is_empty() {
        return None;
    }
    Some(cleaned.join(" "))
}

// ============================================================================
// Go
// ============================================================================

fn extract_go_symbol(
    node: tree_sitter::Node,
    kind: &str,
    source: &str,
    lines: &[&str],
    start_line: usize,
    end_line: usize,
    parent_name: Option<&str>,
) -> Option<SymbolInfo> {
    match kind {
        "function_declaration" => {
            let name = find_child_text(node, "name", source)?;
            let sig = get_line_text(lines, start_line);
            let doc = get_go_doc_comment(lines, start_line);
            Some(SymbolInfo {
                name,
                kind: SymbolKind::Function,
                line: start_line,
                parent: parent_name.map(|s| s.to_string()),
                signature: sig,
                doc_comment: doc,
                end_line,
            })
        }
        "method_declaration" => {
            let name = find_child_text(node, "name", source)?;
            // Extract the receiver type as parent
            let receiver = find_go_receiver(node, source);
            let sig = get_line_text(lines, start_line);
            let doc = get_go_doc_comment(lines, start_line);
            Some(SymbolInfo {
                name,
                kind: SymbolKind::Function,
                line: start_line,
                parent: receiver.or_else(|| parent_name.map(|s| s.to_string())),
                signature: sig,
                doc_comment: doc,
                end_line,
            })
        }
        "type_declaration" => {
            // Go type declarations can contain struct, interface, or alias
            for i in 0..node.child_count() {
                if let Some(child) = node.child(i) {
                    if child.kind() == "type_spec" {
                        let name = find_child_text(child, "name", source)?;
                        let sig = get_line_text(lines, start_line);
                        let doc = get_go_doc_comment(lines, start_line);

                        // Determine the kind by looking at the type
                        let sym_kind = determine_go_type_kind(child);

                        return Some(SymbolInfo {
                            name,
                            kind: sym_kind,
                            line: start_line,
                            parent: parent_name.map(|s| s.to_string()),
                            signature: sig,
                            doc_comment: doc,
                            end_line,
                        });
                    }
                }
            }
            None
        }
        _ => None,
    }
}

/// Determine the SymbolKind for a Go type_spec node.
fn determine_go_type_kind(type_spec: tree_sitter::Node) -> SymbolKind {
    for i in 0..type_spec.child_count() {
        if let Some(child) = type_spec.child(i) {
            match child.kind() {
                "struct_type" => return SymbolKind::Struct,
                "interface_type" => return SymbolKind::Interface,
                _ => {}
            }
        }
    }
    SymbolKind::Type
}

/// Extract the receiver type from a Go method declaration.
fn find_go_receiver(node: tree_sitter::Node, source: &str) -> Option<String> {
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if child.kind() == "parameter_list" {
                // The receiver is the first parameter_list before the method name
                let text = node_text(child, source);
                // Extract type name: (s *Server) -> Server, (s Server) -> Server
                let cleaned = text
                    .trim_start_matches('(')
                    .trim_end_matches(')');
                // Split by whitespace, take the last part, remove * prefix
                if let Some(type_part) = cleaned.split_whitespace().last() {
                    return Some(type_part.trim_start_matches('*').to_string());
                }
                break; // Only first param list is receiver
            }
        }
    }
    None
}

/// Extract Go doc comments (// lines preceding a definition).
fn get_go_doc_comment(lines: &[&str], def_line: usize) -> Option<String> {
    if def_line < 2 {
        return None;
    }
    let mut doc_lines = Vec::new();
    let mut idx = def_line - 2; // 0-based
    loop {
        if idx >= lines.len() {
            break;
        }
        let trimmed = lines[idx].trim();
        if let Some(rest) = trimmed.strip_prefix("//") {
            doc_lines.push(rest.trim().to_string());
        } else {
            break;
        }
        if idx == 0 {
            break;
        }
        idx -= 1;
    }
    if doc_lines.is_empty() {
        return None;
    }
    doc_lines.reverse();
    Some(doc_lines.join(" "))
}

// ============================================================================
// Java
// ============================================================================

fn extract_java_symbol(
    node: tree_sitter::Node,
    kind: &str,
    source: &str,
    lines: &[&str],
    start_line: usize,
    end_line: usize,
    parent_name: Option<&str>,
) -> Option<SymbolInfo> {
    match kind {
        "class_declaration" => {
            let name = find_child_text(node, "name", source)?;
            let sig = get_line_text(lines, start_line);
            let doc = get_jsdoc_comment(lines, start_line); // Java uses same style
            Some(SymbolInfo {
                name,
                kind: SymbolKind::Class,
                line: start_line,
                parent: parent_name.map(|s| s.to_string()),
                signature: sig,
                doc_comment: doc,
                end_line,
            })
        }
        "interface_declaration" => {
            let name = find_child_text(node, "name", source)?;
            let sig = get_line_text(lines, start_line);
            let doc = get_jsdoc_comment(lines, start_line);
            Some(SymbolInfo {
                name,
                kind: SymbolKind::Interface,
                line: start_line,
                parent: parent_name.map(|s| s.to_string()),
                signature: sig,
                doc_comment: doc,
                end_line,
            })
        }
        "enum_declaration" => {
            let name = find_child_text(node, "name", source)?;
            let sig = get_line_text(lines, start_line);
            Some(SymbolInfo {
                name,
                kind: SymbolKind::Enum,
                line: start_line,
                parent: parent_name.map(|s| s.to_string()),
                signature: sig,
                doc_comment: None,
                end_line,
            })
        }
        "method_declaration" => {
            let name = find_child_text(node, "name", source)?;
            let sig = get_line_text(lines, start_line);
            Some(SymbolInfo {
                name,
                kind: SymbolKind::Function,
                line: start_line,
                parent: parent_name.map(|s| s.to_string()),
                signature: sig,
                doc_comment: None,
                end_line,
            })
        }
        "constructor_declaration" => {
            let name = find_child_text(node, "name", source)?;
            let sig = get_line_text(lines, start_line);
            Some(SymbolInfo {
                name,
                kind: SymbolKind::Function,
                line: start_line,
                parent: parent_name.map(|s| s.to_string()),
                signature: sig,
                doc_comment: None,
                end_line,
            })
        }
        _ => None,
    }
}

// ============================================================================
// Utility functions
// ============================================================================

/// Get the text content of a tree-sitter node.
fn node_text(node: tree_sitter::Node, source: &str) -> String {
    source[node.byte_range()].to_string()
}

/// Find a named child by its field name and return its text.
fn find_child_text(node: tree_sitter::Node, field_name: &str, source: &str) -> Option<String> {
    let child = node.child_by_field_name(field_name)?;
    let text = node_text(child, source);
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

/// Get the text of a specific line (1-based), trimmed.
fn get_line_text(lines: &[&str], line_number: usize) -> Option<String> {
    if line_number == 0 || line_number > lines.len() {
        return None;
    }
    let text = lines[line_number - 1].trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

/// Clean up a Python docstring by removing quotes and extra whitespace.
fn clean_docstring(text: &str) -> String {
    let trimmed = text.trim();
    // Remove triple-quote delimiters
    let inner = trimmed
        .strip_prefix("\"\"\"")
        .or_else(|| trimmed.strip_prefix("'''"))
        .unwrap_or(trimmed);
    let inner = inner
        .strip_suffix("\"\"\"")
        .or_else(|| inner.strip_suffix("'''"))
        .unwrap_or(inner);
    // Collapse whitespace
    inner
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_python_symbols() {
        let src = r#"
def greet(name):
    """Say hello."""
    return f"Hello {name}"

class MyService:
    """A service class."""

    def __init__(self):
        pass

    def run(self):
        pass

def helper():
    pass
"#;
        let symbols = parse_symbols(src, "python", 30);
        assert!(symbols.len() >= 4, "Expected at least 4 symbols, got {}", symbols.len());

        let greet = symbols.iter().find(|s| s.name == "greet").expect("greet");
        assert_eq!(greet.kind, SymbolKind::Function);
        assert!(greet.parent.is_none());
        assert!(greet.doc_comment.is_some(), "greet should have docstring");

        let svc = symbols.iter().find(|s| s.name == "MyService").expect("MyService");
        assert_eq!(svc.kind, SymbolKind::Class);
        assert!(svc.doc_comment.is_some(), "MyService should have docstring");

        let init = symbols.iter().find(|s| s.name == "__init__").expect("__init__");
        assert_eq!(init.kind, SymbolKind::Function);
        assert_eq!(init.parent.as_deref(), Some("MyService"));

        let run = symbols.iter().find(|s| s.name == "run").expect("run");
        assert_eq!(run.parent.as_deref(), Some("MyService"));
    }

    #[test]
    fn test_rust_symbols() {
        let src = r#"
use std::io;

/// Main entry point
pub fn main() {
    println!("hello");
}

fn helper_fn(x: i32) -> i32 {
    x + 1
}

/// Configuration struct
pub struct Config {
    name: String,
}

pub enum Status {
    Active,
    Inactive,
}

pub trait Processor {
    fn process(&self);
}

pub type Result<T> = std::result::Result<T, Error>;

pub mod utils;
"#;
        let symbols = parse_symbols(src, "rust", 30);

        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"main"), "should find main. Got: {:?}", names);
        assert!(names.contains(&"helper_fn"), "should find helper_fn");
        assert!(names.contains(&"Config"), "should find Config");
        assert!(names.contains(&"Status"), "should find Status");
        assert!(names.contains(&"Processor"), "should find Processor");
        assert!(names.contains(&"Result"), "should find Result type alias");
        assert!(names.contains(&"utils"), "should find mod utils");

        let main_sym = symbols.iter().find(|s| s.name == "main").unwrap();
        assert_eq!(main_sym.kind, SymbolKind::Function);
        assert!(main_sym.doc_comment.is_some(), "main should have doc comment");

        let config_sym = symbols.iter().find(|s| s.name == "Config").unwrap();
        assert_eq!(config_sym.kind, SymbolKind::Struct);
        assert!(config_sym.doc_comment.is_some(), "Config should have doc comment");

        let status_sym = symbols.iter().find(|s| s.name == "Status").unwrap();
        assert_eq!(status_sym.kind, SymbolKind::Enum);

        let processor_sym = symbols.iter().find(|s| s.name == "Processor").unwrap();
        assert_eq!(processor_sym.kind, SymbolKind::Interface);
    }

    #[test]
    fn test_typescript_symbols() {
        let src = r#"
import { useState } from 'react';

export function createApp(config: AppConfig) {
    return new App(config);
}

export class AppService {
    constructor() {}

    process(data: string): void {}
}

export interface Config {
    name: string;
    port: number;
}

export type Status = 'active' | 'inactive';

export enum Direction {
    Up,
    Down,
}

export const MAX_RETRIES = 5;
"#;
        let symbols = parse_symbols(src, "typescript", 30);

        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"createApp"), "should find createApp. Got: {:?}", names);
        assert!(names.contains(&"AppService"), "should find AppService");
        assert!(names.contains(&"Config"), "should find Config");
        assert!(names.contains(&"Status"), "should find Status");
        assert!(names.contains(&"Direction"), "should find Direction");
        assert!(names.contains(&"MAX_RETRIES"), "should find MAX_RETRIES");

        let create_app = symbols.iter().find(|s| s.name == "createApp").unwrap();
        assert_eq!(create_app.kind, SymbolKind::Function);

        let config = symbols.iter().find(|s| s.name == "Config").unwrap();
        assert_eq!(config.kind, SymbolKind::Interface);

        let direction = symbols.iter().find(|s| s.name == "Direction").unwrap();
        assert_eq!(direction.kind, SymbolKind::Enum);
    }

    #[test]
    fn test_go_symbols() {
        let src = r#"
package main

import "fmt"

// main is the entry point.
func main() {
    fmt.Println("hello")
}

// Start starts the server.
func (s *Server) Start() error {
    return nil
}

type Config struct {
    Name string
    Port int
}

type Handler interface {
    Handle(req Request) Response
}

type StringSlice []string
"#;
        let symbols = parse_symbols(src, "go", 30);

        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"main"), "should find main. Got: {:?}", names);
        assert!(names.contains(&"Start"), "should find Start");
        assert!(names.contains(&"Config"), "should find Config");
        assert!(names.contains(&"Handler"), "should find Handler");
        assert!(names.contains(&"StringSlice"), "should find StringSlice");

        let start_sym = symbols.iter().find(|s| s.name == "Start").unwrap();
        assert_eq!(start_sym.kind, SymbolKind::Function);
        assert_eq!(start_sym.parent.as_deref(), Some("Server"));
        assert!(start_sym.doc_comment.is_some());

        let config = symbols.iter().find(|s| s.name == "Config").unwrap();
        assert_eq!(config.kind, SymbolKind::Struct);

        let handler = symbols.iter().find(|s| s.name == "Handler").unwrap();
        assert_eq!(handler.kind, SymbolKind::Interface);
    }

    #[test]
    fn test_java_symbols() {
        let src = r#"
package com.example;

import java.util.List;

public class UserService {
    private final UserRepository repo;

    public UserService(UserRepository repo) {
        this.repo = repo;
    }

    public User findById(String id) {
        return repo.findById(id);
    }
}

public interface UserRepository {
    User findById(String id);
}

public enum Status {
    ACTIVE,
    INACTIVE;
}
"#;
        let symbols = parse_symbols(src, "java", 30);

        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"UserService"), "should find UserService. Got: {:?}", names);
        assert!(names.contains(&"UserRepository"), "should find UserRepository");
        assert!(names.contains(&"Status"), "should find Status");

        let svc = symbols.iter().find(|s| s.name == "UserService").unwrap();
        assert_eq!(svc.kind, SymbolKind::Class);

        let repo = symbols.iter().find(|s| s.name == "UserRepository").unwrap();
        assert_eq!(repo.kind, SymbolKind::Interface);

        let status = symbols.iter().find(|s| s.name == "Status").unwrap();
        assert_eq!(status.kind, SymbolKind::Enum);
    }

    #[test]
    fn test_max_symbols_limit() {
        let mut src = String::new();
        for i in 0..50 {
            src.push_str(&format!("def func_{}():\n    pass\n\n", i));
        }
        let symbols = parse_symbols(&src, "python", 5);
        assert_eq!(symbols.len(), 5);
    }

    #[test]
    fn test_unsupported_language() {
        let symbols = parse_symbols("some content", "haskell", 30);
        assert!(symbols.is_empty());
    }

    #[test]
    fn test_empty_content() {
        let symbols = parse_symbols("", "python", 30);
        assert!(symbols.is_empty());
    }

    #[test]
    fn test_is_language_supported() {
        assert!(is_language_supported("python"));
        assert!(is_language_supported("rust"));
        assert!(is_language_supported("typescript"));
        assert!(is_language_supported("javascript"));
        assert!(is_language_supported("go"));
        assert!(is_language_supported("java"));
        assert!(!is_language_supported("haskell"));
        assert!(!is_language_supported("config"));
        assert!(!is_language_supported("other"));
    }

    #[test]
    fn test_end_lines_are_populated() {
        let src = r#"
def greet(name):
    return f"Hello {name}"

def farewell(name):
    return f"Goodbye {name}"
"#;
        let symbols = parse_symbols(src, "python", 30);
        assert_eq!(symbols.len(), 2);
        for sym in &symbols {
            assert!(sym.end_line > sym.line, "end_line should be > start line for {}", sym.name);
        }
    }

    #[test]
    fn test_rust_impl_methods_have_parent() {
        let src = r#"
pub struct MyStruct {
    value: i32,
}

impl MyStruct {
    pub fn new(value: i32) -> Self {
        Self { value }
    }

    pub fn get_value(&self) -> i32 {
        self.value
    }
}
"#;
        let symbols = parse_symbols(src, "rust", 30);
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"MyStruct"), "should find MyStruct. Got: {:?}", names);
        assert!(names.contains(&"new"), "should find new method. Got: {:?}", names);
        assert!(names.contains(&"get_value"), "should find get_value method. Got: {:?}", names);

        let new_fn = symbols.iter().find(|s| s.name == "new").unwrap();
        assert_eq!(new_fn.kind, SymbolKind::Function);
        // The impl block should set parent context
        assert!(new_fn.parent.is_some(), "impl method should have parent");
    }

    #[test]
    fn test_signatures_are_populated() {
        let src = r#"
pub fn process_data(items: &[Item], config: &Config) -> Result<Vec<Output>> {
    Ok(vec![])
}
"#;
        let symbols = parse_symbols(src, "rust", 30);
        assert_eq!(symbols.len(), 1);
        assert!(
            symbols[0].signature.is_some(),
            "signature should be populated"
        );
        let sig = symbols[0].signature.as_ref().unwrap();
        assert!(sig.contains("process_data"), "signature should contain function name");
    }
}
