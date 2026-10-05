use crate::model::{Class, Function, Method, Parameter, Type};
use tree_sitter::{Node, Parser};

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

fn child_text(node: Node, source: &str) -> String {
    node.utf8_text(source.as_bytes())
        .unwrap_or("")
        .to_string()
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
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_functions_and_nullable_types() {
        let code = r#"<?php
function getUser(?int $id, string $role): ?string {
    return "User: " . $id;
}
"#;
        let tree = parse_php(code).unwrap();
        let functions = find_functions(tree.root_node(), code);
        assert_eq!(functions.len(), 1);

        let func = &functions[0];
        assert_eq!(func.name, "getUser");
        assert_eq!(func.parameters.len(), 2);

        assert_eq!(func.parameters[0].name, "$id");
        assert_eq!(
            func.parameters[0].type_name,
            Some(Type {
                name: "int".to_string(),
                nullable: true,
            })
        );

        assert_eq!(func.parameters[1].name, "$role");
        assert_eq!(
            func.parameters[1].type_name,
            Some(Type {
                name: "string".to_string(),
                nullable: false,
            })
        );

        assert_eq!(
            func.return_type,
            Some(Type {
                name: "string".to_string(),
                nullable: true,
            })
        );
    }

    #[test]
    fn test_parse_class_and_methods() {
        let code = r#"<?php
class UserController {
    public function show(?int $id): ?User {
        return null;
    }

    public function index(): void {
    }
}
"#;
        let tree = parse_php(code).unwrap();
        let classes = find_classes(tree.root_node(), code);
        assert_eq!(classes.len(), 1);

        let class = &classes[0];
        assert_eq!(class.name, "UserController");
        assert_eq!(class.methods.len(), 2);

        let show = &class.methods[0];
        assert_eq!(show.name, "show");
        assert_eq!(show.parameters.len(), 1);
        assert_eq!(show.parameters[0].name, "$id");
        assert_eq!(
            show.parameters[0].type_name,
            Some(Type {
                name: "int".to_string(),
                nullable: true,
            })
        );
        assert_eq!(
            show.return_type,
            Some(Type {
                name: "User".to_string(),
                nullable: true,
            })
        );

        let index = &class.methods[1];
        assert_eq!(index.name, "index");
        assert_eq!(
            index.return_type,
            Some(Type {
                name: "void".to_string(),
                nullable: false,
            })
        );
    }
}
