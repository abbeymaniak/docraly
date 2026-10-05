use std::path::Path;
use tree_sitter::{Node, Parser};

use super::LanguageAdapter;
use crate::discovery::Language;
use crate::model::{Class, FileModel, Function, Import, Method, Parameter, SourceLocation, Type};

pub struct PhpAdapter;

impl PhpAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl LanguageAdapter for PhpAdapter {
    fn language(&self) -> Language {
        Language::Php
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["php"]
    }

    fn parse_file(
        &self,
        path: &Path,
        relative_path: &Path,
        source: &str,
    ) -> Result<FileModel, String> {
        let tree = parse_php(source)?;
        let root = tree.root_node();

        let namespace = extract_namespace(root, source);
        let imports = extract_imports(root, source);
        let functions = find_functions(root, source);
        let mut classes = find_classes(root, source);

        // Assign file namespace to all enclosed classes
        for class in &mut classes {
            if class.namespace.is_none() {
                class.namespace = namespace.clone();
            }
        }

        let mut file_model = FileModel::new(path.to_path_buf(), relative_path.to_path_buf());
        file_model.namespace = namespace;
        file_model.imports = imports;
        file_model.functions = functions;
        file_model.classes = classes;

        Ok(file_model)
    }
}

pub fn parse_php(source: &str) -> Result<tree_sitter::Tree, String> {
    let mut parser = Parser::new();
    let language = tree_sitter_php::LANGUAGE_PHP.into();

    parser
        .set_language(&language)
        .map_err(|error| format!("Failed to load PHP parser: {error}"))?;

    parser
        .parse(source, None)
        .ok_or_else(|| "Failed to parse PHP source".to_string())
}

fn node_location(node: Node) -> SourceLocation {
    SourceLocation {
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        start_byte: node.start_byte(),
        end_byte: node.end_byte(),
    }
}

fn child_text(node: Node, source: &str) -> String {
    node.utf8_text(source.as_bytes())
        .unwrap_or("")
        .to_string()
}

#[allow(dead_code)]
pub fn extract_namespace(root: Node, source: &str) -> Option<String> {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "namespace_definition" {
            let mut sub_cursor = child.walk();
            for sub in child.named_children(&mut sub_cursor) {
                if sub.kind() == "namespace_name" || sub.kind() == "name" {
                    let text = child_text(sub, source).trim().to_string();
                    if !text.is_empty() {
                        return Some(text);
                    }
                }
            }
        }
    }
    None
}

#[allow(dead_code)]
pub fn extract_imports(root: Node, source: &str) -> Vec<Import> {
    let mut imports = Vec::new();
    let mut cursor = root.walk();

    for child in root.children(&mut cursor) {
        if child.kind() == "namespace_use_declaration" {
            let mut sub_cursor = child.walk();
            for sub in child.named_children(&mut sub_cursor) {
                if sub.kind() == "namespace_use_clause" {
                    let mut clause_cursor = sub.walk();
                    let named_parts: Vec<_> = sub.named_children(&mut clause_cursor).collect();

                    if let Some(first) = named_parts.first() {
                        let path = child_text(*first, source).trim().to_string();
                        let alias = if named_parts.len() > 1 {
                            Some(child_text(named_parts[1], source).trim().to_string())
                        } else {
                            None
                        };

                        if !path.is_empty() {
                            imports.push(Import { path, alias });
                        }
                    }
                }
            }
        }
    }

    imports
}

pub fn find_functions(node: Node, source: &str) -> Vec<Function> {
    let mut functions = Vec::new();
    collect_functions(node, source, &mut functions);
    functions
}

fn collect_functions(node: Node, source: &str, functions: &mut Vec<Function>) {
    if node.kind() == "function_definition" {
        if let Some(function) = extract_function(node, source) {
            functions.push(function);
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_functions(child, source, functions);
    }
}

fn extract_function(node: Node, source: &str) -> Option<Function> {
    let mut name = String::new();
    let mut parameters = Vec::new();
    let mut return_type = None;
    let location = Some(node_location(node));

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        match child.kind() {
            "name" => {
                name = child_text(child, source);
            }
            "formal_parameters" => {
                parameters = extract_parameters(child, source);
            }
            "primitive_type" | "named_type" | "optional_type" | "type" => {
                return_type = extract_type(child, source);
            }
            _ => {}
        }
    }

    if name.is_empty() {
        return None;
    }

    Some(Function {
        name,
        parameters,
        return_type,
        location,
    })
}

fn extract_parameters(node: Node, source: &str) -> Vec<Parameter> {
    let mut parameters = Vec::new();
    let mut cursor = node.walk();

    for child in node.named_children(&mut cursor) {
        if child.kind() == "simple_parameter" {
            parameters.push(extract_parameter(child, source));
        }
    }

    parameters
}

fn extract_parameter(node: Node, source: &str) -> Parameter {
    let mut name = String::new();
    let mut type_name: Option<Type> = None;

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        match child.kind() {
            "variable_name" => {
                name = child_text(child, source);
            }
            "type" | "primitive_type" | "named_type" | "optional_type" => {
                type_name = extract_type(child, source);
            }
            _ => {}
        }
    }

    Parameter { name, type_name }
}

pub fn extract_type(node: Node, source: &str) -> Option<Type> {
    match node.kind() {
        "optional_type" => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                if let Some(inner) = extract_type(child, source) {
                    return Some(Type {
                        name: inner.name,
                        nullable: true,
                    });
                }
            }
            let text = child_text(node, source).trim().to_string();
            Some(Type {
                nullable: true,
                name: text.trim_start_matches('?').trim().to_string(),
            })
        }
        "primitive_type" | "named_type" | "type" => {
            let text = child_text(node, source).trim().to_string();
            if text.is_empty() {
                None
            } else {
                Some(Type {
                    nullable: text.starts_with('?'),
                    name: text.trim_start_matches('?').trim().to_string(),
                })
            }
        }
        _ => None,
    }
}

pub fn find_classes(node: Node, source: &str) -> Vec<Class> {
    let mut classes = Vec::new();
    collect_classes(node, source, &mut classes);
    classes
}

fn collect_classes(node: Node, source: &str, classes: &mut Vec<Class>) {
    if node.kind() == "class_declaration" {
        if let Some(class) = extract_class(node, source) {
            classes.push(class);
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_classes(child, source, classes);
    }
}

fn extract_class(node: Node, source: &str) -> Option<Class> {
    let mut name = String::new();
    let mut methods = Vec::new();
    let location = Some(node_location(node));

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        match child.kind() {
            "name" => {
                name = child_text(child, source);
            }
            "declaration_list" => {
                methods = extract_methods(child, source);
            }
            _ => {}
        }
    }

    if name.is_empty() {
        return None;
    }

    Some(Class {
        name,
        namespace: None,
        methods,
        location,
    })
}

fn extract_methods(node: Node, source: &str) -> Vec<Method> {
    let mut methods = Vec::new();
    let mut cursor = node.walk();

    for child in node.named_children(&mut cursor) {
        if child.kind() == "method_declaration" {
            if let Some(method) = extract_method(child, source) {
                methods.push(method);
            }
        }
    }

    methods
}

fn extract_method(node: Node, source: &str) -> Option<Method> {
    let mut name = String::new();
    let mut parameters = Vec::new();
    let mut return_type = None;
    let location = Some(node_location(node));

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        match child.kind() {
            "name" => {
                name = child_text(child, source);
            }
            "formal_parameters" => {
                parameters = extract_parameters(child, source);
            }
            "primitive_type" | "named_type" | "optional_type" | "type" => {
                return_type = extract_type(child, source);
            }
            _ => {}
        }
    }

    if name.is_empty() {
        return None;
    }

    Some(Method {
        name,
        parameters,
        return_type,
        location,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_php_adapter_parse_file_complete() {
        let code = r#"<?php
namespace App\Http\Controllers;

use App\Models\User;
use App\Services\PaymentService as Payment;

class UserController {
    public function show(?int $id): ?User {
        return null;
    }
}

function helper(string $msg): void {}
"#;
        let adapter = PhpAdapter::new();
        let file_model = adapter
            .parse_file(
                Path::new("app/Http/Controllers/UserController.php"),
                Path::new("app/Http/Controllers/UserController.php"),
                code,
            )
            .unwrap();

        assert_eq!(
            file_model.namespace,
            Some("App\\Http\\Controllers".to_string())
        );
        assert_eq!(file_model.imports.len(), 2);
        assert_eq!(file_model.imports[0].path, "App\\Models\\User");
        assert_eq!(file_model.imports[0].alias, None);
        assert_eq!(file_model.imports[1].path, "App\\Services\\PaymentService");
        assert_eq!(file_model.imports[1].alias, Some("Payment".to_string()));

        assert_eq!(file_model.classes.len(), 1);
        let class = &file_model.classes[0];
        assert_eq!(class.name, "UserController");
        assert_eq!(
            class.namespace,
            Some("App\\Http\\Controllers".to_string())
        );
        assert_eq!(
            class.fully_qualified_name(),
            "App\\Http\\Controllers\\UserController"
        );
        assert!(class.location.is_some());

        assert_eq!(class.methods.len(), 1);
        let method = &class.methods[0];
        assert_eq!(method.name, "show");
        assert_eq!(
            method.return_type,
            Some(Type {
                name: "User".to_string(),
                nullable: true,
            })
        );
        assert!(method.location.is_some());

        assert_eq!(file_model.functions.len(), 1);
        assert_eq!(file_model.functions[0].name, "helper");
    }
}
